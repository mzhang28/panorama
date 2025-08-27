use anyhow::Result;
use graphql_parser::query::{Definition, Document, OperationDefinition, Selection};
use sqlx::{QueryBuilder, Row};

use crate::db::Dal;
use serde_json::json;

/// GraphQL request processor.
///
/// This struct holds the parsed GraphQL document, the DAL and any variables
/// provided with the request. It provides small, testable helper methods that
/// perform parts of the overall processing (field collection, SQL construction,
/// execution, mutation handling). The public API remains `process_graphql_request`.
pub struct GraphProcessor {
    pub dal: Dal,
    pub variables: Option<serde_json::Value>,
}

impl GraphProcessor {
    /// Create a new processor which holds `dal` and optional `variables`.
    pub fn new(dal: Dal, variables: Option<serde_json::Value>) -> Self {
        Self { dal, variables }
    }

    /// Resolve a GraphQL literal or variable into a string using `self.variables`.
    ///
    /// Supports string, number and boolean literals as well as variables that
    /// refer to values in the provided `variables` map.
    pub fn resolve_gql_value(&self, val: &graphql_parser::query::Value<String>) -> Option<String> {
        use graphql_parser::query::Value as V;
        match val {
            V::String(s) => Some(s.clone()),
            V::Boolean(b) => Some(b.to_string()),
            V::Variable(var_name) => {
                if let Some(vars) = self.variables.as_ref() {
                    match vars.get(var_name) {
                        Some(serde_json::Value::String(s)) => Some(s.clone()),
                        Some(serde_json::Value::Number(n)) => Some(n.to_string()),
                        Some(serde_json::Value::Bool(b)) => Some(b.to_string()),
                        Some(v) => Some(v.to_string()),
                        None => None,
                    }
                } else {
                    None
                }
            }
            V::Int(i) => Some(format!("{:?}", i)),
            V::Float(f) => Some(format!("{:?}", f)),
            _ => None,
        }
    }

    /// Collect requested node scalar columns and app keys from the parsed document.
    ///
    /// Returns `(node_cols, app_keys)` where `app_keys` are strings like `journal/title`.
    pub fn collect_query_fields(&self, doc: &Document<'_, String>) -> (Vec<String>, Vec<String>) {
        // Collect node scalar columns and app keys using iterator chains
        let mut node_cols: Vec<String> = Vec::new();
        let mut app_keys: Vec<String> = Vec::new();

        for def in doc.definitions.iter() {
            // Normalize to an iterator over top-level selections for this definition
            let top_items = match def {
                Definition::Operation(OperationDefinition::SelectionSet(s)) => {
                    s.items.iter().collect::<Vec<_>>()
                }
                Definition::Operation(OperationDefinition::Query(q)) => {
                    q.selection_set.items.iter().collect::<Vec<_>>()
                }
                _ => Vec::new(),
            };

            top_items.into_iter().filter_map(pred_matches!(Selection::Field(field) => field)).for_each(|field| {
                if field.name == "nodes" {
                    // For each field requested on `nodes`, decide whether it's a node scalar
                    // or an app object and push into the corresponding vec.
                    field.selection_set.items.iter().filter_map(pred_matches!(Selection::Field(f) => f)).for_each(|f| {
                        if f.selection_set.items.is_empty() {
                            node_cols.push(f.name.to_string());
                        } else {
                            let app_name = f.name.clone();
                            f.selection_set.items.iter().filter_map(pred_matches!(Selection::Field(nf) => nf)).for_each(|nf| {
                                app_keys.push(format!("{}/{}", app_name, nf.name));
                            });
                        }
                    });
                }
            });
        }

        (node_cols, app_keys)
    }

    /// Convert a schema key like `app/field` into a safe alias name used in
    /// SQL result column aliases (non-alphanumeric replaced with `_`).
    fn alias_name_for_key(key: &str) -> String {
        let mut alias_key = key.replace("/", "__");
        alias_key = alias_key.replace(|ch: char| !ch.is_alphanumeric() && ch != '_', "_");
        format!("__app_{}", alias_key)
    }

    /// For a logical schema key like `app/field` return the SQL alias and the
    /// split parts `(alias, app, field)`. Returns `None` if the key cannot be
    /// split into two parts.
    fn alias_and_parts_for_key(key: &str) -> Option<(String, String, String)> {
        if let Some((app, field)) = key.split_once('/') {
            Some((Self::alias_name_for_key(key), app.to_string(), field.to_string()))
        } else {
            None
        }
    }

    /// Given a sqlite table and column, look up the corresponding schema `key`
    /// in `_panorama_schema_columns`. If not found, fall back to a
    /// `"table::column"` synthetic key.
    async fn schema_key_for_table_col(&self, table: &str, col: &str) -> Result<String> {
        let mut qb_k = QueryBuilder::new(
            "select key from _panorama_schema_columns where sqlite_table_name = ? and sqlite_column_name = ?",
        );
        let qbk = qb_k.build();
        let rec = qbk
            .bind(table)
            .bind(col)
            .fetch_optional(&self.dal.pool)
            .await?;
        let key: String = if let Some(r) = rec {
            r.get::<String, _>(0)
        } else {
            format!("{}::{}", table, col)
        };
        Ok(key)
    }

    /// Build a SQL `SELECT` statement string and return also the resolved
    /// `app_keys_out` (the original keys used to map result aliases back to
    /// app fields).
    pub async fn build_select_sql(
        &self,
        node_cols: &[String],
        app_keys: &[String],
    ) -> Result<(String, Vec<String>)> {
        // Start building select parts (we'll alias columns to stable names)
        let mut select_parts: Vec<String> = Vec::new();
        // id as "id"
        select_parts.push("nodes.id as id".to_string());
        for col in node_cols.iter() {
            if col.as_str() == "id" {
                continue;
            }
            select_parts.push(format!("nodes.\"{}\" as \"{}\"", col, col));
        }

        // We'll need LEFT JOINs for each distinct sqlite_table_name
        // Query the schema table for app_keys
        if app_keys.is_empty() {
            let sql = format!("select {} from nodes", select_parts.join(", "));
            return Ok((sql, vec![]));
        }

        // fetch mappings
        let mut qb_schema = QueryBuilder::new(
            "select key, sqlite_table_name, sqlite_column_name from _panorama_schema_columns where key in (",
        );
        let mut s = qb_schema.separated(", ");
        for k in app_keys.iter() {
            s.push_bind(k);
        }
        s.push_unseparated(")");
        let q = qb_schema.build();
        let rows = q.fetch_all(&self.dal.pool).await?;

        // Map table -> columns
        use std::collections::HashMap;
        // Fold rows into a map of table -> columns for clearer intent
        let table_cols: HashMap<String, Vec<String>> =
            rows.iter().fold(HashMap::new(), |mut acc, row| {
                let _key: String = row.get::<String, _>(0);
                let table: String = row.get::<String, _>(1);
                let col: String = row.get::<String, _>(2);
                acc.entry(table).or_default().push(col);
                acc
            });

        // Build joins and add selected columns with aliases. For clarity we
        // enumerate tables so aliases are predictable (t0, t1, ...).
        let mut joins: Vec<String> = Vec::new();
        let mut app_keys_out: Vec<String> = Vec::new();

        for (join_idx, (table, cols)) in table_cols.into_iter().enumerate() {
            let alias = format!("t{}", join_idx);

            // For each column in this table produce a select part and resolve
            // the logical schema key (used to map results back to app fields).
            for c in cols.iter() {
                let key = self.schema_key_for_table_col(&table, &c).await?;
                let alias_name = Self::alias_name_for_key(&key);
                select_parts.push(format!("\"{}\".\"{}\" as \"{}\"", alias, c, alias_name));
                app_keys_out.push(key);
            }

            joins.push(format!(
                "left join \"{}\" as \"{}\" on \"{}\".node_id = nodes.id",
                table, alias, alias
            ));
        }

        let sql = format!(
            "select {} from nodes {}",
            select_parts.join(", "),
            joins.join(" ")
        );
        Ok((sql, app_keys_out))
    }

    /// Execute a previously-built `SELECT` and map rows back into the
    /// GraphQL-shaped JSON value for `nodes`.
    pub async fn execute_select(
        &self,
        sql: &str,
        id_filter: Option<String>,
        node_cols: &[String],
        app_keys: &[String],
    ) -> Result<serde_json::Value> {
        let mut sql_owned = sql.to_string();
        let rows = if let Some(id) = id_filter {
            sql_owned.push_str(" where nodes.id = ?");
            sqlx::query(&sql_owned)
                .bind(id)
                .fetch_all(&self.dal.pool)
                .await?
        } else {
            sqlx::query(&sql_owned).fetch_all(&self.dal.pool).await?
        };

        let mut nodes_json: Vec<serde_json::Value> = Vec::new();
        for row in rows.iter() {
            use serde_json::Value as Jv;
            use std::collections::HashMap;
            let mut obj = serde_json::Map::new();

            // id
            let id: Option<String> = row.try_get("id").ok();
            if let Some(v) = id {
                obj.insert("id".to_string(), Jv::String(v));
            }

            // node scalar cols
            for col in node_cols.iter() {
                if col == "id" {
                    continue;
                }
                if let Ok(val_opt) = row.try_get::<Option<String>, _>(col.as_str()) {
                    if let Some(v) = val_opt {
                        obj.insert(col.clone(), Jv::String(v));
                    }
                }
            }

            // app fields
            let mut apps: HashMap<String, serde_json::Map<String, Jv>> = HashMap::new();

            app_keys.iter().filter_map(|key| {
                // compute alias name and parts
                if let Some((alias_name, app, field)) = Self::alias_and_parts_for_key(key) {
                    match row.try_get::<Option<String>, _>(alias_name.as_str()) {
                        Ok(Some(v)) => Some((app, field, v)),
                        _ => None,
                    }
                } else {
                    None
                }
            }).for_each(|(app, field, v)| {
                let entry = apps.entry(app).or_insert_with(|| serde_json::Map::new());
                entry.insert(field, Jv::String(v));
            });

            // insert apps as nested objects
            for (app, map) in apps.into_iter() {
                obj.insert(app, Jv::Object(map));
            }

            nodes_json.push(Jv::Object(obj));
        }

        Ok(json!({ "data": { "nodes": nodes_json } }))
    }

    /// Handle a `Mutation` operation by iterating the top-level fields and
    /// executing supported mutations. Currently supports `setField`.
    pub async fn process_mutation(
        &self,
        m: &graphql_parser::query::Mutation<'_, String>,
    ) -> Result<serde_json::Value> {
        let mut results = vec![];
        // Iterate only over Selection::Field entries for clarity
        for f in m.selection_set.items.iter().filter_map(pred_matches!(Selection::Field(f) => f)) {
            match f.name.as_str() {
                "setField" => {
                    // Extract args succinctly
                    let mut node_id: Option<String> = None;
                    let mut app: Option<String> = None;
                    let mut field: Option<String> = None;
                    let mut value: Option<String> = None;

                    for (arg_name, arg_val) in f.arguments.iter() {
                        let resolved = self.resolve_gql_value(arg_val);
                        match arg_name.as_str() {
                            "nodeId" => node_id = resolved,
                            "app" => app = resolved,
                            "field" => field = resolved,
                            "value" => value = resolved,
                            _ => {}
                        }
                    }

                    let node_id = node_id.ok_or_else(|| anyhow::anyhow!("missing nodeId"))?;
                    let app = app.ok_or_else(|| anyhow::anyhow!("missing app"))?;
                    let field = field.ok_or_else(|| anyhow::anyhow!("missing field"))?;
                    let value = value.ok_or_else(|| anyhow::anyhow!("missing value"))?;

                    let key = format!("{}/{}", app, field);
                    let rec = self.dal.schema_entry(&key).await?;
                    let (table, col) = rec.ok_or_else(|| anyhow::anyhow!("unknown schema key"))?;

                    // Ensure the node exists.
                    let exists: Option<(i64,)> =
                        sqlx::query_as("select 1 from nodes where id = ? limit 1")
                            .bind(&node_id)
                            .fetch_optional(&self.dal.pool)
                            .await?;
                    if exists.is_none() {
                        let insert_node_sql = "insert into nodes (id, type, created_at, updated_at, extra) values (?, ?, datetime('now'), datetime('now'), '{}')";
                        sqlx::query(insert_node_sql)
                            .bind(&node_id)
                            .bind(&app)
                            .execute(&self.dal.pool)
                            .await?;
                    }

                    // Build SQL to upsert the value into the dynamic table
                    let sql = format!(
                        "insert into \"{}\" (node_id, \"{}\") values (?, ?) on conflict(node_id) do update set \"{}\" = excluded.\"{}\"",
                        table, col, col, col
                    );
                    println!("Executing SQL: {}", sql);
                    sqlx::query(&sql)
                        .bind(node_id.clone())
                        .bind(value.clone())
                        .execute(&self.dal.pool)
                        .await?;

                    results.push(json!({"ok": true, "nodeId": node_id, "app": app, "field": field}));
                }
                other => return Err(anyhow::anyhow!("unsupported mutation: {}", other)),
            }
        }
        Ok(json!({ "data": { "mutations": results } }))
    }
}

/// Process a GraphQL request for both queries and simple mutations.
///
/// Supported mutation: `setField(nodeId: "...", app: "journal", field: "title", value: "...")`
pub async fn process_graphql_request(
    dal: Dal,
    query: String,
    variables: Option<serde_json::Value>,
) -> Result<serde_json::Value> {
    // Helper logic moved into `GraphProcessor::resolve_gql_value`.

    // If there's any mutation operation, handle mutations.
    let doc = graphql_parser::parse_query::<String>(&query)?;
    let processor = GraphProcessor::new(dal.clone(), variables);
    for def in doc.definitions.iter() {
        if let Definition::Operation(OperationDefinition::Mutation(m)) = def {
            return processor.process_mutation(m).await;
        }
    }

    // Query path: collect fields, build SQL, execute and map results
    let (node_cols, app_keys) = processor.collect_query_fields(&doc);
    // detect nodes(id: ...) filter using iterator helpers
    let mut id_filter: Option<String> = None;
    'outer: for def in doc.definitions.iter() {
        let top_items = match def {
            Definition::Operation(OperationDefinition::SelectionSet(s)) => s.items.iter().collect::<Vec<_>>(),
            Definition::Operation(OperationDefinition::Query(q)) => q.selection_set.items.iter().collect::<Vec<_>>(),
            _ => Vec::new(),
        };

        for f in top_items.into_iter().filter_map(pred_matches!(Selection::Field(f) => f)) {
                if f.name == "nodes" {
                if let Some(val) = f.arguments.iter().find_map(|(arg_name, arg_val)| {
                    if arg_name == "id" { processor.resolve_gql_value(arg_val) } else { None }
                }) {
                    id_filter = Some(val);
                    break 'outer;
                }
            }
        }
    }

    let (sql, app_keys_out) = processor.build_select_sql(&node_cols, &app_keys).await?;
    processor
        .execute_select(&sql, id_filter, &node_cols, &app_keys_out)
        .await
}

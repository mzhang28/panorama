use anyhow::Result;
use graphql_parser::query::{Definition, OperationDefinition, Selection};
use sqlx::{QueryBuilder, Row, Sqlite};

use crate::db::Dal;
use graphql_parser::query::Value as GqlValue;
use serde_json::json;

/// Translate a simple GraphQL document into a SQL QueryBuilder.
///
/// Supported subset:
/// - Single top-level selection (e.g. `{ nodes { id type journal { title } } }`)
/// - Node scalars from the `nodes` table: `id`, `type`, `created_at`, `updated_at`
/// - App fields as nested objects: `journal { title }` which map to keys like `journal/title` in the schema.
// NOTE: The previous `graphql_query_to_sql_query` translator has been merged
// into `process_graphql_request` below. Translation and execution now live
// together because multiple DB queries (schema lookups + select) are required
// and it's simpler to keep them in one place.

/// Process a GraphQL request for both queries and simple mutations.
///
/// Supported mutation: `setField(nodeId: "...", app: "journal", field: "title", value: "...")`
pub async fn process_graphql_request(
    dal: Dal,
    query: String,
    variables: Option<serde_json::Value>,
) -> Result<serde_json::Value> {
    let doc = graphql_parser::parse_query::<String>(&query)?;

    // Helper to resolve an incoming GraphQL argument value into a string,
    // supporting both string literals and variables (looked up from `variables`).
    fn resolve_gql_value(
        val: &graphql_parser::query::Value<String>,
        variables: Option<&serde_json::Value>,
    ) -> Option<String> {
        use graphql_parser::query::Value as V;
        match val {
            V::String(s) => Some(s.clone()),
            V::Boolean(b) => Some(b.to_string()),
            V::Variable(var_name) => {
                if let Some(vars) = variables {
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
            // Fallbacks for numeric literals: use Debug formatting
            V::Int(i) => Some(format!("{:?}", i)),
            V::Float(f) => Some(format!("{:?}", f)),
            _ => None,
        }
    }

    // If there's any mutation operation, handle mutations.
    for def in doc.definitions.iter() {
        if let Definition::Operation(OperationDefinition::Mutation(m)) = def {
            // Iterate each top-level field (each mutation)
            let mut results = vec![];
            for sel in m.selection_set.items.iter() {
                if let Selection::Field(f) = sel {
                    let name = f.name.as_str();
                    if name == "setField" {
                        // Extract args
                        let mut node_id: Option<String> = None;
                        let mut app: Option<String> = None;
                        let mut field: Option<String> = None;
                        let mut value: Option<String> = None;
                        for (arg_name, arg_val) in f.arguments.iter() {
                            let resolved = resolve_gql_value(arg_val, variables.as_ref());
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
                        let rec = dal.schema_entry(&key).await?;
                        let (table, col) =
                            rec.ok_or_else(|| anyhow::anyhow!("unknown schema key"))?;

                        // Ensure the node exists. If it doesn't, insert a new node with the
                        // app name as its type and current timestamps. This is currently
                        // non-transactional (two separate statements) — TODO: wrap in a
                        // single DB transaction so creation + upsert are atomic.
                        let exists: Option<(i64,)> = sqlx::query_as("select 1 from nodes where id = ? limit 1")
                            .bind(&node_id)
                            .fetch_optional(&dal.pool)
                            .await?;
                        if exists.is_none() {
                            let insert_node_sql = "insert into nodes (id, type, created_at, updated_at, extra) values (?, ?, datetime('now'), datetime('now'), '{}')";
                            sqlx::query(insert_node_sql)
                                .bind(&node_id)
                                .bind(&app)
                                .execute(&dal.pool)
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
                            .execute(&dal.pool)
                            .await?;

                        results.push(
                            json!({"ok": true, "nodeId": node_id, "app": app, "field": field}),
                        );
                    } else {
                        return Err(anyhow::anyhow!("unsupported mutation: {}", name));
                    }
                }
            }
            return Ok(json!({"data": {"mutations": results}}));
        }
    }

    // Otherwise, treat as a query: detect simple node id filter, build SQL, execute it, and map back to GraphQL-shaped JSON
    // Detect nodes(id: "..." or id: $var) argument if present
    let mut id_filter: Option<String> = None;
    for def in doc.definitions.iter() {
        if let Definition::Operation(OperationDefinition::SelectionSet(selection_set)) = def {
            for item in selection_set.items.iter() {
                if let Selection::Field(f) = item {
                    if f.name == "nodes" {
                        for (arg_name, arg_val) in f.arguments.iter() {
                            if arg_name == "id" {
                                if let Some(res) = resolve_gql_value(arg_val, variables.as_ref()) {
                                    id_filter = Some(res);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Build node_cols and app_keys by inspecting the parsed GraphQL document
    let mut node_cols: Vec<String> = vec![];
    let mut app_keys: Vec<String> = vec![]; // keys like "journal/title"

    for def in doc.definitions.iter() {
        match def {
            Definition::Operation(OperationDefinition::SelectionSet(selection_set)) => {
                for item in selection_set.items.iter() {
                    if let Selection::Field(field) = item {
                        if field.name == "nodes" {
                            for sel in field.selection_set.items.iter() {
                                if let Selection::Field(f) = sel {
                                    if f.selection_set.items.is_empty() {
                                        node_cols.push(f.name.to_string());
                                    } else {
                                        let app_name = f.name.clone();
                                        for nested in f.selection_set.items.iter() {
                                            if let Selection::Field(nf) = nested {
                                                let key = format!("{}/{}", app_name, nf.name);
                                                app_keys.push(key);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Definition::Operation(OperationDefinition::Query(q)) => {
                for item in q.selection_set.items.iter() {
                    if let Selection::Field(field) = item {
                        if field.name == "nodes" {
                            for sel in field.selection_set.items.iter() {
                                if let Selection::Field(f) = sel {
                                    if f.selection_set.items.is_empty() {
                                        node_cols.push(f.name.to_string());
                                    } else {
                                        let app_name = f.name.clone();
                                        for nested in f.selection_set.items.iter() {
                                            if let Selection::Field(nf) = nested {
                                                let key = format!("{}/{}", app_name, nf.name);
                                                app_keys.push(key);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

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
    let mut final_sql: String;
    let mut app_keys_out: Vec<String> = Vec::new();
    if !app_keys.is_empty() {
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
        let rows = q.fetch_all(&dal.pool).await?;

        // Map table -> columns
        use std::collections::HashMap;
        let mut table_cols: HashMap<String, Vec<String>> = HashMap::new();
        for row in rows.iter() {
            let _key: String = row.get::<String, _>(0);
            let table: String = row.get::<String, _>(1);
            let col: String = row.get::<String, _>(2);
            table_cols.entry(table).or_default().push(col);
        }

        // Build joins and add selected columns with aliases
        let mut joins: Vec<String> = Vec::new();
        let mut join_idx = 0usize;
        let mut _table_idx = 0usize;
        for (table, cols) in table_cols.into_iter() {
            let alias = format!("t{}", join_idx);
            for c in cols.iter() {
                // fetch key for this table/column
                let mut qb_k = QueryBuilder::new(
                    "select key from _panorama_schema_columns where sqlite_table_name = ? and sqlite_column_name = ?",
                );
                let qbk = qb_k.build();
                let rec = qbk.bind(&table).bind(&c).fetch_optional(&dal.pool).await?;
                let key: String = if let Some(r) = rec {
                    r.get::<String, _>(0)
                } else {
                    format!("{}::{}", table, c)
                };

                // sanitize key for alias
                let mut alias_key = key.replace("/", "__");
                alias_key = alias_key.replace(|ch: char| !ch.is_alphanumeric() && ch != '_', "_");
                let alias_name = format!("__app_{}", alias_key);
                select_parts.push(format!("\"{}\".\"{}\" as \"{}\"", alias, c, alias_name));
                app_keys_out.push(key);
            }
            joins.push(format!(
                "left join \"{}\" as \"{}\" on \"{}\".node_id = nodes.id",
                table, alias, alias
            ));
            join_idx += 1;
            _table_idx += 1;
        }

        final_sql = format!(
            "select {} from nodes {}",
            select_parts.join(", "),
            joins.join(" ")
        );
    } else {
        final_sql = format!("select {} from nodes", select_parts.join(", "));
    }

    let mut sql = final_sql;
    // use node_cols and the resolved app_keys_out for mapping results
    let app_keys = app_keys_out;
    println!("SQL: {}", sql);
    let rows = if let Some(id) = id_filter {
        sql.push_str(" where nodes.id = ?");
        sqlx::query(&sql).bind(id).fetch_all(&dal.pool).await?
    } else {
        sqlx::query(&sql).fetch_all(&dal.pool).await?
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
        // app_keys correspond to aliases we used: __app_<sanitized>
        let mut apps: HashMap<String, serde_json::Map<String, Jv>> = HashMap::new();
        for key in app_keys.iter() {
            let mut alias_key = key.replace("/", "__");
            alias_key = alias_key.replace(|ch: char| !ch.is_alphanumeric() && ch != '_', "_");
            let alias_name = format!("__app_{}", alias_key);
            if let Ok(val_opt) = row.try_get::<Option<String>, _>(alias_name.as_str()) {
                if let Some(v) = val_opt {
                    // split key into app and field
                    if let Some((app, field)) = key.split_once('/') {
                        let entry = apps
                            .entry(app.to_string())
                            .or_insert_with(|| serde_json::Map::new());
                        entry.insert(field.to_string(), Jv::String(v));
                    }
                }
            }
        }

        // insert apps as nested objects
        for (app, map) in apps.into_iter() {
            obj.insert(app, Jv::Object(map));
        }

        nodes_json.push(Jv::Object(obj));
    }

    Ok(json!({"data": {"nodes": nodes_json}}))
}

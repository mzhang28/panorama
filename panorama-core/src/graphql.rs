use anyhow::Result;
use graphql_parser::query::{Definition, OperationDefinition, Selection};
use sqlx::{QueryBuilder, Row, Sqlite};

use crate::db::Dal;

/// Translate a simple GraphQL document into a SQL QueryBuilder.
///
/// Supported subset:
/// - Single top-level selection (e.g. `{ nodes { id type journal { title } } }`)
/// - Node scalars from the `nodes` table: `id`, `type`, `created_at`, `updated_at`
/// - App fields as nested objects: `journal { title }` which map to keys like `journal/title` in the schema.
pub async fn graphql_query_to_sql_query(
    dal: Dal,
    query: String,
) -> Result<QueryBuilder<'static, Sqlite>> {
    let doc = graphql_parser::parse_query::<String>(&query)?;
    // Ensure single selection set
    assert!(
        doc.definitions
            .iter()
            .filter(|def| matches!(
                def,
                Definition::Operation(OperationDefinition::SelectionSet(_))
            ))
            .count()
            == 1,
        "Multiple selection sets are not supported"
    );

    // We'll collect requested node scalar columns and app keys
    let mut node_cols: Vec<String> = vec![];
    let mut app_keys: Vec<String> = vec![]; // keys like "journal/title"

    for def in doc.definitions.into_iter() {
        match def {
            Definition::Operation(OperationDefinition::SelectionSet(selection_set)) => {
                for item in selection_set.items {
                    match item {
                        Selection::Field(field) => {
                            // Top-level field, expect something like `nodes { ... }`
                            // We only support `nodes` top-level for now
                            if field.name == "nodes" {
                                for sel in field.selection_set.items {
                                    match sel {
                                        Selection::Field(f) => {
                                            // If scalar, map to nodes.<name>
                                            if f.selection_set.items.is_empty() {
                                                node_cols.push(f.name.to_string());
                                            } else {
                                                // Nested selection -> app fields
                                                let app_name = f.name;
                                                for nested in f.selection_set.items {
                                                    if let Selection::Field(nf) = nested {
                                                        let key =
                                                            format!("{}/{}", app_name, nf.name);
                                                        app_keys.push(key);
                                                    }
                                                }
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                        Selection::FragmentSpread(_) => todo!(),
                        Selection::InlineFragment(_) => todo!(),
                    }
                }
            }
            Definition::Operation(_) => todo!(),
            Definition::Fragment(_) => todo!(),
        }
    }

    // Start building select parts
    let mut select_parts: Vec<String> = Vec::new();
    select_parts.push("nodes.id".to_string());
    for col in node_cols.iter() {
        if col.as_str() == "id" {
            continue;
        }
        select_parts.push(format!("nodes.{}", col));
    }

    // We'll need LEFT JOINs for each distinct sqlite_table_name
    // Query the schema table for app_keys
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
        for (table, cols) in table_cols.into_iter() {
            let alias = format!("t{}", join_idx);
            for c in cols.iter() {
                select_parts.push(format!("\"{}\".\"{}\"", alias, c));
            }
            joins.push(format!(
                "left join \"{}\" as \"{}\" on \"{}\".node_id = nodes.id",
                table, alias, alias
            ));
            join_idx += 1;
        }

        let sql = format!(
            "select {} from nodes {}",
            select_parts.join(", "),
            joins.join(" ")
        );
        let final_qb = QueryBuilder::new(sql);
        return Ok(final_qb);
    }

    // No app fields, just select from nodes
    let sql = format!("select {} from nodes", select_parts.join(", "));
    Ok(QueryBuilder::new(sql))
}

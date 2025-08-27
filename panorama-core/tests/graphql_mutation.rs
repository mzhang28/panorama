use anyhow::Result;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::Row;
use std::fs;

use panorama_core::{install_default_apps, Dal};

#[tokio::test]
async fn test_set_field_mutation_with_variables() -> Result<()> {
    let db = "test_graphql_mutation.db";
    let _ = fs::remove_file(db);

    let pool = SqlitePoolOptions::new()
        .connect_with(sqlx::sqlite::SqliteConnectOptions::new().filename(db).create_if_missing(true))
        .await?;
    let dal = Dal::new(pool.clone());
    dal.migrate().await?;

    install_default_apps(dal.clone()).await?;

    // Insert a node
    let node_id = "node-123";
    sqlx::query("insert into nodes (id, type, created_at, updated_at) values (?, 'note', datetime('now'), datetime('now'))")
        .bind(node_id)
        .execute(&pool)
        .await?;

    // Send mutation using variables
    let mutation = r#"
        mutation($nodeId: String!, $value: String!) {
            setField(nodeId: $nodeId, app: "journal", field: "title", value: $value)
        }
    "#;

    let vars = serde_json::json!({
        "nodeId": node_id,
        "value": "Hello from variable test"
    });

    let res = panorama_core::process_graphql_request(dal.clone(), mutation.to_string(), Some(vars)).await?;
    // Expect ok
    assert!(res.get("data").is_some());

    // Verify the value was written into the dynamic table
    let rec = dal.schema_entry("journal/title").await?;
    let (table, col) = rec.ok_or_else(|| anyhow::anyhow!("missing schema entry"))?;

    let sql = format!("select \"{}\" from \"{}\" where node_id = ?", col, table);
    let row = sqlx::query(&sql).bind(node_id).fetch_one(&pool).await?;
    let val: Option<String> = row.try_get(0)?;
    assert_eq!(val.unwrap(), "Hello from variable test");

    let _ = fs::remove_file(db);
    Ok(())
}


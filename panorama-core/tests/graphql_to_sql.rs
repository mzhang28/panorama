use anyhow::Result;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::path::Path;

use panorama_core::{install_default_apps, Dal};

#[tokio::test]
async fn test_graphql_to_sql_simple() -> Result<()> {
    let db_path = "test_graphql_to_sql.db";
    if Path::new(db_path).exists() {
        let _ = std::fs::remove_file(db_path);
    }

    let pool_opt = SqliteConnectOptions::new().filename(db_path).create_if_missing(true);
    let pool = SqlitePoolOptions::new().connect_with(pool_opt).await?;
    let dal = Dal::new(pool);

    dal.migrate().await?;
    install_default_apps(dal.clone()).await?;

    let query = "{ nodes { id type journal { title } } }".to_string();
    let qb = panorama_core::graphql_query_to_sql_query(dal.clone(), query).await?;

    // Get the sqlite names for journal/title
    let rec = dal.schema_entry("journal/title").await?;
    let (table, col) = rec.expect("schema entry should exist");

    let sql = qb.sql().to_string();
    assert!(sql.contains(&table), "SQL should reference the journal table");
    assert!(sql.contains(&col), "SQL should reference the journal column");

    let _ = std::fs::remove_file(db_path);
    Ok(())
}

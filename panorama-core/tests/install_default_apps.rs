use anyhow::Result;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::path::Path;

use panorama_core::{install_default_apps, Dal};

#[tokio::test]
async fn test_install_default_apps_idempotent() -> Result<()> {
    // Use a temporary file so migrations work reliably
    let db_path = "test_install_default_apps.db";
    if Path::new(db_path).exists() {
        let _ = std::fs::remove_file(db_path);
    }

    let pool_opt = SqliteConnectOptions::new().filename(db_path).create_if_missing(true);
    let pool = SqlitePoolOptions::new().connect_with(pool_opt).await?;
    let dal = Dal::new(pool);

    // Run migrations and install apps twice
    dal.migrate().await?;
    install_default_apps(dal.clone()).await?;
    install_default_apps(dal.clone()).await?;

    // Check that the key exists only once
    let count = dal.schema_count("journal/title").await?;
    assert_eq!(count, 1, "journal/title should be present exactly once");

    // Cleanup
    let _ = std::fs::remove_file(db_path);

    Ok(())
}

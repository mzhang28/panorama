use std::collections::HashMap;

use anyhow::Result;

use crate::db::Dal;

pub async fn install_default_apps(dal: Dal) -> Result<()> {
    // Desired journal fields
    let mut desired = HashMap::new();
    desired.insert("journal/title".to_string(), "TEXT".to_string());
    // Store the editor contents separately from the title
    desired.insert("journal/content".to_string(), "TEXT".to_string());

    // File app: store file metadata (sha256 and original filename)
    desired.insert("file/sha256".to_string(), "TEXT".to_string());
    desired.insert("file/name".to_string(), "TEXT".to_string());
    desired.insert("file/size".to_string(), "INTEGER".to_string());

    // Determine which keys are missing from _panorama_schema_columns
    let mut missing = HashMap::new();
    for (key, ty) in desired.into_iter() {
        if !dal.has_schema_key(&key).await? {
            missing.insert(key, ty);
        }
    }

    if !missing.is_empty() {
        dal.create_table("journal", missing).await?;
    }

    Ok(())
}

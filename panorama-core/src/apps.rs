use std::collections::HashMap;

use anyhow::Result;

use crate::db::Dal;

pub async fn install_default_apps(dal: Dal) -> Result<()> {
    let mut fields = HashMap::new();
    fields.insert(format!("journal/title"), format!("TEXT"));
    dal.create_table("journal", fields).await?;

    Ok(())
}

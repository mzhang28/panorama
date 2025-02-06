use std::collections::HashMap;

use anyhow::Result;
use serde_json::json;
use sqlx::Row;
use tracing::Level;

use crate::graphql::Context;

pub async fn run_automate_service(ctx: Context) -> Result<()> {
  let schedules = HashMap::<String, String>::new();

  // Fetch all of the existing schedules in the database
  info!("starting automate scheduler...");
  let results = sqlx::query(
    r#"
    select id, created_at, automate_trigger_json from node
    where automate_trigger_json is not null
  "#,
  )
  .fetch_all(&ctx.db)
  .await?
  .into_iter()
  .map(|row| json!({ "id": row.get::<String, _>(0) }))
  .collect::<Vec<_>>();

  event!(Level::INFO, results = ?results, "found");

  // Create our own router

  Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TriggerData {
  #[serde(flatten)]
  trigger_type: TriggerType,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "trigger_type")]
pub enum TriggerType {
  Cron(CronTrigger),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CronTrigger {
  expression: String,
}

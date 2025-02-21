use axum::extract::State;
use sqlx::Row;

use crate::context::Context;

pub async fn run_query(State(ctx): State<Context>) {
  // Get the attributes
  let rows = sqlx::query("pragma table_info(node)")
    .fetch_all(&ctx.db)
    .await
    .unwrap();
  let attributes = rows
    .into_iter()
    .map(|row| row.get("name"))
    .collect::<Vec<String>>();

  info!(attributes=?attributes,"attributes");
}

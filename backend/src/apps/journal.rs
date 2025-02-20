use axum::{
  extract::{Path, State},
  Json,
};
use sqlx::Row;

use crate::context::Context;

#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct GetJournalResponse {
  date: String,
  node_id: String,
  content: String,
}

pub async fn get_journal(
  State(ctx): State<Context>,
  Path(date): Path<String>,
) -> Json<GetJournalResponse> {
  let row = sqlx::query(
    "insert into node (journal_date) values (?)
    on conflict (journal_date) do update set journal_date = EXCLUDED.journal_date
    returning id, content",
  )
  .bind(&date)
  .fetch_one(&ctx.db)
  .await
  .unwrap();

  Json(GetJournalResponse {
    date,
    node_id: row.get(0),
    content: row.get(1),
  })
}

#[derive(Debug, Deserialize)]
pub struct SaveJournalRequest {
  content: String,
}

#[derive(Debug, Serialize)]
pub struct SaveJournalResponse {}

pub async fn save_journal(
  State(ctx): State<Context>,
  Path(date): Path<String>,
  Json(request): Json<SaveJournalRequest>,
) {
  sqlx::query("update node set content = ? where journal_date = ?")
    .bind(&request.content)
    .bind(&date)
    .execute(&ctx.db)
    .await
    .unwrap();
}

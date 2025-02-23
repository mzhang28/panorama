use axum::extract::State;
use axum::{Json, extract::Path};
use sqlx::Row;

use crate::context::Context;

pub async fn get_tags(State(ctx): State<Context>, Path(id): Path<String>) -> Json<Vec<String>> {
  let rows = sqlx::query("SELECT tag FROM tags WHERE node_id = ?")
    .bind(&id)
    .fetch_all(&ctx.db)
    .await
    .unwrap();
  let tags = rows
    .into_iter()
    .map(|row| row.get("tag"))
    .collect::<Vec<String>>();
  Json(tags)
}

#[derive(Deserialize)]
pub struct UpdateTagsPayload {
  #[serde(default)]
  insertions: Vec<String>,
  #[serde(default)]
  deletions: Vec<String>,
}

pub async fn update_tags(
  State(ctx): State<Context>,
  Path(id): Path<String>,
  Json(payload): Json<UpdateTagsPayload>,
) {
  let mut query_builder = sqlx::QueryBuilder::new("INSERT INTO tags (node_id, tag) ");
  query_builder.push_values(&payload.insertions, |mut b, tag| {
    b.push_bind(&id).push_bind(tag);
  });
  query_builder.push(" on conflict (node_id, tag) do nothing");
  query_builder.build().execute(&ctx.db).await.unwrap();

  let mut query_builder =
    sqlx::QueryBuilder::new("DELETE FROM tags WHERE node_id = ? AND tag IN (");
  query_builder.push_bind(&id);
  let mut sep = query_builder.separated(", ");
  for tag in payload.deletions {
    sep.push_bind(tag);
  }
  query_builder.push(")");
  query_builder.build().execute(&ctx.db).await.unwrap();
}

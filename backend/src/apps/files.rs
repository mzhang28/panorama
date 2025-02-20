use axum::{
  extract::{Multipart, State},
  Json,
};
use object_store::{path::Path, PutPayload};
use sha2::{Digest, Sha256};
use sqlx::Row;
use uuid::Uuid;

use crate::context::Context;

#[derive(Debug, Serialize)]
pub struct UploadFileResponse {
  node_id: String,
}

pub async fn upload_file(
  State(ctx): State<Context>,
  mut multipart: Multipart,
) -> Json<UploadFileResponse> {
  let field = multipart.next_field().await.unwrap().unwrap();
  let file_bytes = field.bytes().await.unwrap();

  let mut hasher = Sha256::new();
  hasher.update(&file_bytes);
  let hash = hasher.finalize();
  let hash = format!("{:x}", hash);

  // TODO: streaming upload w/ progress
  let uuid = Uuid::now_v7();
  let upload_path = Path::parse(format!("uploads/{}", uuid.to_string())).unwrap();
  let payload = PutPayload::from_bytes(file_bytes);
  ctx.object_store.put(&upload_path, payload).await.unwrap();

  let blob_path = Path::parse(&hash).unwrap();
  ctx
    .object_store
    .rename(&upload_path, &blob_path)
    .await
    .unwrap();

  let row = sqlx::query("insert into node (blob_hash) values (?) returning id")
    .bind(hash)
    .fetch_one(&ctx.db)
    .await
    .unwrap();

  Json(UploadFileResponse {
    node_id: row.get(0),
  })
}

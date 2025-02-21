use axum::{extract::State, Json};
use futures::StreamExt;
use object_store::{path::Path as ObstorePath, PutPayload};
use serde_json::Value as JsonValue;
use sha2::{Digest, Sha256};
use sqlx::Row;
use uuid::Uuid;

use crate::{context::Context, db::get_config};

#[derive(Debug, Serialize)]
pub struct ConnectorPingResponse {}

pub async fn connector_ping() -> Json<ConnectorPingResponse> {
  Json(ConnectorPingResponse {})
}

#[derive(Debug, Deserialize)]
pub struct ConnectorGetSelectedCollectionRequest {
  #[serde(flatten)]
  json: JsonValue,
}

#[derive(Debug, Serialize)]
pub struct ConnectorGetSelectedCollectionResponse {
  #[serde(rename = "libraryID")]
  library_id: String,
  #[serde(rename = "libraryName")]
  library_name: String,
  id: String,
  name: String,
}

pub async fn connector_get_selected_collection(
  State(ctx): State<Context>,
  Json(req): Json<ConnectorGetSelectedCollectionRequest>,
) -> Json<ConnectorGetSelectedCollectionResponse> {
  let row = sqlx::query("insert into node (panorama_config_key, panorama_config_value)
  values ('zotero_root_collection', UUIDV7_NOW())
  on conflict (panorama_config_key) do update set panorama_config_value = EXCLUDED.panorama_config_value
  returning panorama_config_value").fetch_one(&ctx.db).await.unwrap();
  let root_id: String = row.get(0);

  // ensure that the root collection exists
  // TODO: can i inline this into the above?
  sqlx::query("insert into node (id) values (?) on conflict (id) do nothing")
    .bind(&root_id)
    .execute(&ctx.db)
    .await
    .unwrap();

  debug!(req=?req,"fuck");

  Json(ConnectorGetSelectedCollectionResponse {
    library_id: root_id.clone(),
    library_name: String::from("root"),
    id: root_id,
    name: String::from("root"),
  })
}

#[derive(Debug, Deserialize)]
pub struct ConnectorSaveSnapshotRequest {
  html: String,
  title: String,
  url: String,
  uri: String,
  #[serde(rename = "skipSnapshot")]
  skip_snapshot: bool,
  #[serde(rename = "sessionID")]
  session_id: String,
  referrer: String,
  cookie: Option<String>,
  #[serde(rename = "detailedCookies")]
  detailed_cookies: Option<String>,
  #[serde(rename = "singleFile")]
  single_file: bool,
  pdf: bool,

  #[serde(flatten)]
  json: JsonValue,
}

#[derive(Debug, Serialize)]
pub struct ConnectorSaveSnapshotResponse {
  #[serde(rename = "saveSingleFile")]
  save_single_file: bool,
}

pub async fn connector_save_snapshot(
  State(ctx): State<Context>,
  Json(req): Json<ConnectorSaveSnapshotRequest>,
) -> Json<ConnectorSaveSnapshotResponse> {
  let mut req = req;
  // req.html = String::new();
  debug!(req = ?req, "saveSnapshot");

  // MAN so zotero is just a skin of firefox, so it actually launches a full browser to load the page
  // i ain't doing that (at least in this version)
  // (in future versions i might bundle headless chrome)

  if !req.skip_snapshot && req.single_file && req.pdf {
    // Let's just try to grab the pdf from the fucking url
    // upload it as a blob
    let res = reqwest::get(req.url).await.unwrap();
    let mut stream = res.bytes_stream();
    let mut hasher = Sha256::new();

    // prepare dest
    let uuid = Uuid::now_v7();
    let download_path = ObstorePath::parse(format!("downloads/{}", uuid.to_string())).unwrap();
    let mut file = ctx
      .object_store
      .put_multipart(&download_path)
      .await
      .unwrap();

    while let Some(bytes) = stream.next().await {
      let bytes = bytes.unwrap();
      hasher.update(&bytes);
      let payload = PutPayload::from_bytes(bytes);
      file.put_part(payload).await.unwrap();
    }

    file.complete().await.unwrap();

    let hash = hasher.finalize();
    let hash_hex = format!("{:x}", hash);
    let blob_path = ObstorePath::parse(&hash_hex).unwrap();
    ctx
      .object_store
      .rename(&download_path, &blob_path)
      .await
      .unwrap();

    let row = sqlx::query(
      "insert into node (blob_hash, zotero_parent_collection)
      values (?, ?)
      returning id",
    )
    .bind(&hash_hex)
    .bind("helloge")
    .fetch_one(&ctx.db)
    .await
    .unwrap();
    let id: String = row.get(0);
    info!(id = id, "NEW ID");
  }

  Json(ConnectorSaveSnapshotResponse {
    save_single_file: !req.skip_snapshot && !req.pdf && req.single_file,
  })
}

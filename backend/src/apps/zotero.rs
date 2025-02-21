use axum::{extract::State, Json};
use serde_json::Value as JsonValue;
use sqlx::Row;

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
  Json(req): Json<ConnectorSaveSnapshotRequest>,
) -> Json<ConnectorSaveSnapshotResponse> {
  let mut req = req;
  // req.html = String::new();
  debug!(req = ?req, "saveSnapshot");

  Json(ConnectorSaveSnapshotResponse {
    save_single_file: !req.skip_snapshot && !req.pdf && req.single_file,
  })
}

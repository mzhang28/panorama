#[macro_use]
extern crate serde;
#[macro_use]
extern crate ts_rs;
#[macro_use]
extern crate tracing;

mod apps;
pub mod context;
mod db;
mod node;
pub mod services;
pub mod tag;
pub mod utils;

use std::env;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use axum::extract::{DefaultBodyLimit, MatchedPath, Request};
use axum::response::Response;
use axum::routing::{get, patch, post, put};
use axum::{Json, Router};
use object_store::local::LocalFileSystem;
use serde_json::json;
use sqlx::{migrate, sqlite::SqliteConnectOptions};
use tantivy::Index;
use tantivy::directory::MmapDirectory;
use tantivy::schema::{Schema, TEXT};
use tower_http::trace::TraceLayer;
use tracing::Span;
use utils::get_panorama_state_dir;

use crate::apps::{cal, files, journal, search, wakatime, zotero};
use crate::context::Context;
use crate::db::init_db_options;

pub async fn create_context() -> Result<Context> {
  let state_dir = get_panorama_state_dir();
  std::fs::create_dir_all(&state_dir)?;
  info!(
    state_dir = state_dir.display().to_string(),
    "Using state dir"
  );

  let db = {
    let db_path = state_dir.join("panorama.db");
    let db = init_db_options()
      .connect_with(
        SqliteConnectOptions::new()
          .filename(db_path)
          .create_if_missing(true),
      )
      .await?;
    migrate!().run(&db).await?;
    db
  };

  let object_store = {
    let storage_root = state_dir.join("storage");
    std::fs::create_dir_all(&storage_root)?;
    LocalFileSystem::new_with_prefix(storage_root)?
  };

  let tantivy_index = {
    let tantivy_index_dir = state_dir.join("tantivy");
    std::fs::create_dir_all(&tantivy_index_dir)?;
    let directory = MmapDirectory::open(tantivy_index_dir)?;
    let schema = get_tantivy_schema();
    Index::open_or_create(directory, schema)?
  };

  Ok(Context {
    db: db.clone(),
    object_store: Arc::new(object_store),
    tantivy_index,
  })
}

pub async fn create_web_server(context: Context) -> Result<Router> {
  #[rustfmt::skip]
  let app = Router::new()
    .route("/", get(|| async { Json(json!({"hello": "world!", "version": env!("CARGO_PKG_VERSION")})) }))
    .route("/apps/cal/events", get(cal::query_events))
    .route("/apps/cal/ics_upload", post(cal::ics_upload))
    .route("/apps/file/upload", post(files::upload_file))
    .route("/apps/journal/by_date/{date}", get(journal::get_journal))
    .route("/apps/journal/by_date/{date}", post(journal::save_journal))
    .route("/apps/journal/by_date/{date}/prev", get(journal::get_prev_journal))
    .route("/apps/search", get(search::run_query))
    .route("/apps/wakatime/api/v1/users/current/heartbeats.bulk", post(wakatime::bulk_heartbeats))
    .route("/apps/wakatime/api/v1/users/current/statusbar/today", get(wakatime::statusbar))
    // TODO: REMOVE ---------------v
    .route("/apps/zotero/connector/ping", post(zotero::connector_ping))
    .route("/apps/zotero/connector/saveSnapshot", post(zotero::connector_save_snapshot))
    .route("/apps/zotero/connector/getSelectedCollection", post(zotero::connector_get_selected_collection))
    // TODO: ----------------------^
    .route("/node", put(node::create))
    .route("/node/recent", get(node::recent))
    .route("/node/{id}", get(node::fetch))
    .route("/node/{id}", patch(node::update))
    .route("/node/{id}/tags", get(tag::get_tags))
    .route("/node/{id}/tags", patch(tag::update_tags))
  ;

  let app = app
    .layer(DefaultBodyLimit::max(1024 * 1024 * 1024))
    .layer(
      TraceLayer::new_for_http()
        .make_span_with(|request: &Request<_>| {
          // Log the matched route's path (with placeholders not filled in).
          // Use request.uri() or OriginalUri if you want the real path.
          let matched_path = request
            .extensions()
            .get::<MatchedPath>()
            .map(MatchedPath::as_str);

          info_span!(
              "http_request",
              method = ?request.method(),
              matched_path,
              path = tracing::field::Empty,
              status = tracing::field::Empty,
          )
        })
        .on_request(|request: &Request<_>, span: &Span| {
          span.record("path", request.uri().path_and_query().map(|pq| pq.as_str()));
        })
        .on_response(|res: &Response, latency, span: &Span| {
          span.record("status", res.status().as_u16());
          debug!(latency = ?latency, "request");
        }),
    )
    .with_state(context.clone());

  Ok(app)
}

fn get_tantivy_schema() -> Schema {
  let mut builder = Schema::builder();
  builder.add_text_field("content", TEXT);
  builder.build()
}

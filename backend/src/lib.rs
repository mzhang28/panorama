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
pub mod seed_data;
pub mod services;
pub mod utils;

use std::env;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use axum::extract::{DefaultBodyLimit, MatchedPath, Request};
use axum::response::Response;
use axum::routing::{any, get, post};
use axum::{Extension, Router};
use object_store::local::LocalFileSystem;
use sqlx::{migrate, sqlite::SqliteConnectOptions};
use tower_http::trace::TraceLayer;
use tracing::Span;

use crate::apps::{cal, dataviz, files, journal, search, wakatime, zotero};
use crate::context::Context;
use crate::db::init_db_options;

pub async fn create_context() -> Result<Context> {
  let db_path = PathBuf::from(env::var("DATABASE_PATH").unwrap_or_else(|_| "test.db".to_owned()));

  let db = init_db_options()
    .connect_with(
      SqliteConnectOptions::new()
        .filename(db_path)
        .create_if_missing(true),
    )
    .await?;

  migrate!().run(&db).await?;

  let storage_root = PathBuf::from("./storage");
  std::fs::create_dir_all(&storage_root)?;
  let object_store = LocalFileSystem::new_with_prefix(storage_root)?;

  Ok(Context {
    db: db.clone(),
    object_store: Arc::new(object_store),
  })
}

pub async fn create_web_server(context: Context) -> Result<Router> {
  #[rustfmt::skip]
  let app = Router::new()
    .route("/", get(|| async { "Hello, World!" }))
    .route("/apps/cal/events", get(cal::query_events))
    .route("/apps/cal/ics_upload", post(cal::ics_upload))
    .route("/apps/file/upload", post(files::upload_file))
    .route("/apps/journal/by_date/{date}", get(journal::get_journal))
    .route("/apps/journal/by_date/{date}", post(journal::save_journal))
    .route("/apps/journal/by_date/{date}/prev", get(journal::get_prev_journal))
    .route("/apps/search", get(search::run_query))
    .route("/apps/wakatime/api/v1/users/current/heartbeats.bulk", post(wakatime::bulk_heartbeats))
    .route("/apps/wakatime/api/v1/users/current/statusbar/today", get(wakatime::statusbar))
    .route("/apps/zotero/connector/ping", post(zotero::connector_ping))
    .route("/apps/zotero/connector/saveSnapshot", post(zotero::connector_save_snapshot))
    .route("/apps/zotero/connector/getSelectedCollection", post(zotero::connector_get_selected_collection))
    .route("/node/recent", get(node::recent))
    .route("/node/{id}", get(node::fetch))
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

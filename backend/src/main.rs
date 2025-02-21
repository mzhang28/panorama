#[macro_use]
extern crate serde;
#[macro_use]
extern crate ts_rs;
#[macro_use]
extern crate tracing;

mod apps;
pub mod context;
mod db;
pub mod seed_data;
pub mod services;

use std::{env, path::PathBuf, sync::Arc};

use anyhow::Result;
use apps::{journal, search};
use axum::{
  extract::{MatchedPath, Request},
  routing::{any, get, on, post, MethodFilter},
  Extension, Router,
};
use chrono::Utc;
use db::init_db_options;
use juniper::EmptyMutation;
use juniper_graphql_ws::ConnectionConfig;
use object_store::local::LocalFileSystem;
use rusqlite::functions::FunctionFlags;
use seed_data::ensure_seed_data;
use services::spawn_services;
use sqlx::{
  migrate,
  sqlite::{SqliteConnectOptions, SqlitePoolOptions},
  Row,
};
use tokio::{spawn, sync::mpsc};
use tower_http::trace::TraceLayer;
use tracing::{info_span, Span};
use tracing_subscriber::{fmt::time::uptime, layer::SubscriberExt, util::SubscriberInitExt};
use uuid::Uuid;

use crate::apps::{cal, files, wakatime};
use crate::context::Context;

#[tokio::main]
async fn main() -> Result<()> {
  let format = tracing_subscriber::fmt::format()
    .with_level(true) // don't include levels in formatted output
    .with_target(false) // don't include targets
    .with_thread_ids(false) // include the thread ID of the current thread
    .with_thread_names(false) // include the name of the current thread
    .with_ansi(true)
    .with_timer(uptime())
    .with_source_location(true)
    .pretty(); // use the `Compact` formatting style.

  tracing_subscriber::registry()
    .with(
      tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        // axum logs rejections from built-in extractors with the `axum::rejection`
        // target, at `TRACE` level. `axum::rejection=trace` enables showing those events
        format!(
          "{}=debug,tower_http=debug,axum::rejection=trace",
          env!("CARGO_CRATE_NAME")
        )
        .into()
      }),
    )
    .with(tracing_subscriber::fmt::layer().event_format(format))
    .init();

  let db_path = PathBuf::from(env::var("DATABASE_PATH").unwrap_or_else(|_| "test.db".to_owned()));

  let db = init_db_options()
    .connect_with(
      SqliteConnectOptions::new()
        .filename(db_path)
        .create_if_missing(true),
    )
    .await?;

  migrate!().run(&db).await?;

  ensure_seed_data(&db).await?;

  let storage_root = PathBuf::from("./storage");
  std::fs::create_dir_all(&storage_root)?;
  let object_store = LocalFileSystem::new_with_prefix(storage_root)?;

  let context = Context {
    db: db.clone(),
    object_store: Arc::new(object_store),
  };

  let (workflow_router_tx, workflow_router_rx) = mpsc::unbounded_channel();
  let workflow_router = move |req: Request| async move {
    workflow_router_tx.send(req);
  };

  // Spawn services
  let services_handle = spawn(spawn_services(context.clone()));

  #[rustfmt::skip]
  let app = Router::new()
    .route("/", get(|| async { "Hello, World!" }))
    .route("/workflows", any(workflow_router))
    .route("/apps/search", get(search::run_query))
    .route("/apps/file/upload", post(files::upload_file))
    .route("/apps/cal/ics_upload", post(cal::ics_upload))
    .route("/apps/cal/events", get(cal::query_events))
    .route("/apps/journal/by_date/{date}", get(journal::get_journal))
    .route("/apps/journal/by_date/{date}", post(journal::save_journal))
    .route("/apps/wakatime/api/v1/users/current/statusbar/today", get(wakatime::statusbar))
    .route("/apps/wakatime/api/v1/users/current/heartbeats.bulk", post(wakatime::bulk_heartbeats))
  ;

  let app = app
    .layer(Extension(context.clone()))
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
          )
        })
        .on_request(|request: &Request<_>, span: &Span| {
          // You can use `_span.record("some_other_field", value)` in one of these
          // closures to attach a value to the initially empty field in the info_span
          // created above.
          span.record("path", request.uri().path_and_query().map(|pq| pq.as_str()));
        }),
    )
    .with_state(context.clone());

  let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
  axum::serve(listener, app).await.unwrap();

  Ok(())
}

#[macro_use]
extern crate serde;

mod apps;
mod db;
mod graphql;

use std::{env, path::PathBuf, sync::Arc};

use anyhow::Result;
use apps::{cal, wakatime};
use axum::{
  extract::{MatchedPath, Request, State},
  routing::{get, on, post, MethodFilter},
  Extension, Router,
};
use chrono::Utc;
use juniper::EmptyMutation;
use juniper_graphql_ws::ConnectionConfig;
use rusqlite::functions::FunctionFlags;
use sqlx::{
  migrate,
  sqlite::{SqliteConnectOptions, SqlitePoolOptions},
  Row,
};
use tower_http::trace::TraceLayer;
use tracing::{info_span, Span};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use uuid::Uuid;

use crate::graphql::{Context, Query, Schema, Subscription};

#[tokio::main]
async fn main() -> Result<()> {
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
    .with(tracing_subscriber::fmt::layer())
    .init();

  let db_path = PathBuf::from(env::var("DATABASE_PATH").unwrap_or_else(|_| "test.db".to_owned()));
  let db = SqlitePoolOptions::new()
    .after_connect(|conn, _| {
      Box::pin(async move {
        let mut locked_conn = conn.lock_handle().await?;
        let mut raw_handle = locked_conn.as_raw_handle();

        // SAFETY: We are currently locking the handle
        let rusqlite_handle =
          unsafe { rusqlite::Connection::from_handle_owned(raw_handle.as_mut()) }
            // ok holy shit sqlx::SqliteError can't be constructed so i need to figure out how to handle this error
            .unwrap();

        rusqlite_handle
          .create_scalar_function("NOW_ISO8601", 0, FunctionFlags::SQLITE_UTF8, |_| {
            let now = Utc::now();
            Ok(now.to_rfc3339())
          })
          // same as above
          .unwrap();

        rusqlite_handle
          .create_scalar_function("UUIDV7_NOW", 0, FunctionFlags::SQLITE_UTF8, |_| {
            let id = Uuid::now_v7();
            Ok(id.to_string())
          })
          // same as above
          .unwrap();

        drop(rusqlite_handle);
        drop(locked_conn);
        // conn is now unlocked

        Ok(())
      })
    })
    .connect_with(
      SqliteConnectOptions::new()
        .filename(db_path)
        .create_if_missing(true),
    )
    .await?;

  migrate!().run(&db).await?;

  let context = Context { db: db.clone() };
  let schema = Schema::new(Query, EmptyMutation::new(), Subscription);

  let app = Router::new()
    .route(
      "/graphql",
      on(
        MethodFilter::GET.or(MethodFilter::POST),
        crate::graphql::handler,
      ),
    )
    .route(
      "/subscriptions",
      get(juniper_axum::ws::<Arc<Schema>>(ConnectionConfig::new(
        context.clone(),
      ))),
    )
    .route(
      "/graphiql",
      get(juniper_axum::graphiql("/api/graphql", "/api/subscriptions")),
    )
    .route("/", get(|| async { "Hello, World!" }))
    .route("/apps/cal/ics_upload", post(cal::ics_upload))
    .route(
      "/apps/wakatime/api/v1/users/current/statusbar/today",
      get(wakatime::statusbar),
    )
    .route(
      "/apps/wakatime/api/v1/users/current/heartbeats.bulk",
      post(wakatime::bulk_heartbeats),
    )
    .layer(Extension(Arc::new(schema)))
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

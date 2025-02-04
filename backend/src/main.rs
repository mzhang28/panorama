mod db;
mod graphql;

use std::sync::Arc;

use anyhow::Result;
use axum::{
  extract::State,
  routing::{get, on, post, MethodFilter},
  Extension, Router,
};
use chrono::Utc;
use juniper::EmptyMutation;
use juniper_graphql_ws::ConnectionConfig;
use rusqlite::functions::FunctionFlags;
use sqlx::{
  sqlite::{SqliteConnectOptions, SqlitePoolOptions},
  Row,
};
use uuid::Uuid;

use crate::graphql::{Context, Query, Schema, Subscription};

#[tokio::main]
async fn main() -> Result<()> {
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
          .create_scalar_function("NOW_ISO8601", 0, FunctionFlags::SQLITE_UTF8, |ctx| {
            let now = Utc::now();
            Ok(now.to_rfc3339())
          })
          // same as above
          .unwrap();

        rusqlite_handle
          .create_scalar_function("UUIDV7_NOW", 0, FunctionFlags::SQLITE_UTF8, |ctx| {
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
        .filename("test.db")
        .create_if_missing(true),
    )
    .await?;
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
    .layer(Extension(Arc::new(schema)))
    .layer(Extension(context.clone()))
    .route("/", get(|| async { "Hello, World!" }))
    .route("/asdf", post(asdf))
    .with_state(context.clone());

  let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
  axum::serve(listener, app).await.unwrap();

  Ok(())
}

async fn asdf(State(ctx): State<Context>) -> String {
  let result = sqlx::query("INSERT INTO node DEFAULT VALUES RETURNING id")
    .fetch_one(&ctx.db)
    .await
    .unwrap();
  result.get(0)
}

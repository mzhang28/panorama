#[macro_use]
extern crate anyhow;
#[macro_use]
extern crate serde;
#[macro_use]
extern crate serde_json;
#[macro_use]
extern crate sugars;

mod error;
mod export;
mod journal;
mod migrations;
mod node;
mod query_builder;

use std::fs;

use anyhow::Result;
use axum::{
  http::Method,
  routing::{get, post, put},
  Router,
};
use cozo::DbInstance;
use serde_json::Value;
use tokio::net::TcpListener;
use tower::ServiceBuilder;
use tower_http::cors::{self, CorsLayer};

use crate::{
  export::export,
  journal::get_todays_journal_id,
  migrations::run_migrations,
  node::{create_node, get_node, node_types, search_nodes, update_node},
};

#[derive(Clone)]
pub struct AppState {
  db: DbInstance,
}

#[tokio::main]
async fn main() -> Result<()> {
  let data_dir = dirs::data_dir().unwrap();
  let panorama_dir = data_dir.join("panorama");
  let db_path = panorama_dir.join("db.sqlite");
  fs::create_dir_all(panorama_dir)?;

  let db = DbInstance::new(
    "sqlite",
    db_path.display().to_string(),
    Default::default(),
  )
  .unwrap();

  run_migrations(&db).await?;

  let state = AppState { db };

  let cors = CorsLayer::new()
    .allow_methods([Method::GET, Method::POST, Method::PUT])
    .allow_headers(cors::Any)
    .allow_origin(cors::Any);

  // build our application with a single route
  let app = Router::new()
    .route("/", get(|| async { "Hello, World!" }))
    .route("/export", get(export))
    .route("/node", put(create_node))
    .route("/node/search", get(search_nodes))
    .route("/node/:id", get(get_node))
    .route("/node/:id", post(update_node))
    .route("/node/types", get(node_types))
    .route("/journal/get_todays_journal_id", get(get_todays_journal_id))
    .layer(ServiceBuilder::new().layer(cors))
    .with_state(state);

  let listener = TcpListener::bind("0.0.0.0:5195").await?;
  println!("Listening... {:?}", listener);
  axum::serve(listener, app).await?;

  Ok(())
}

pub fn ensure_ok(s: &str) -> Result<()> {
  let status: Value = serde_json::from_str(&s)?;
  let status = status.as_object().unwrap();
  let ok = status.get("ok").unwrap().as_bool().unwrap_or(false);
  if !ok {
    bail!("shit (error: {s})")
  }
  Ok(())
}

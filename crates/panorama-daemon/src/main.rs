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
mod mail;
mod migrations;
mod node;
mod query_builder;
pub mod state;

use std::fs;

use anyhow::Result;
use axum::{
  http::Method,
  routing::{get, post, put},
  Router,
};
use serde_json::Value;
use tokio::net::TcpListener;
use tower::ServiceBuilder;
use tower_http::cors::{self, CorsLayer};
use utoipa::OpenApi;
use utoipa_scalar::{Scalar, Servable};

use crate::{
  export::export,
  journal::get_todays_journal_id,
  mail::{get_mail, get_mail_config},
  node::{create_node, node_types, search_nodes, update_node},
  state::AppState,
};

#[tokio::main]
async fn main() -> Result<()> {
  #[derive(OpenApi)]
  #[openapi(
    modifiers(),
    nest((path = "/node", api = crate::node::NodeApi)),
  )]
  struct ApiDoc;

  let data_dir = dirs::data_dir().unwrap();
  let panorama_dir = data_dir.join("panorama");
  fs::create_dir_all(&panorama_dir)?;

  let state = AppState::new(&panorama_dir).await?;

  let cors = CorsLayer::new()
    .allow_methods([Method::GET, Method::POST, Method::PUT])
    .allow_headers(cors::Any)
    .allow_origin(cors::Any);

  // build our application with a single route
  let app = Router::new()
    // .merge(
    //   SwaggerUi::new("/swagger-ui")
    //     .url("/api-docs/openapi.json", ApiDoc::openapi()),
    // )
    .merge(Scalar::with_url("/api/docs", ApiDoc::openapi()))
    .route("/", get(|| async { "Hello, World!" }))
    .route("/export", get(export))
    .route("/node", put(create_node))
    .route("/node/search", get(search_nodes))
    // .route("/node/:id", get(get_node))
    // .route("/node/:id", post(update_node))
    .route("/node/types", get(node_types))
    .nest("/node", node::router().with_state(state.clone()))
    .route("/journal/get_todays_journal_id", get(get_todays_journal_id))
    .route("/mail/config", get(get_mail_config))
    .route("/mail", get(get_mail))
    .layer(ServiceBuilder::new().layer(cors))
    .with_state(state.clone());

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
    let display = status.get("display").unwrap().as_str().unwrap();
    bail!("shit (error: {display})")
  }
  Ok(())
}

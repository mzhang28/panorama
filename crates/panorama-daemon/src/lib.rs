#[macro_use]
extern crate anyhow;
#[macro_use]
extern crate serde;
#[macro_use]
extern crate serde_json;
#[macro_use]
extern crate sugars;

mod error;
mod journal;
pub mod mail;
mod node;

use std::fs;

use anyhow::Result;
use axum::{http::Method, routing::get, Router};
use panorama_core::AppState;
use tokio::net::TcpListener;
use tower::ServiceBuilder;
use tower_http::{
  cors::{self, CorsLayer},
  trace::TraceLayer,
};
use utoipa::OpenApi;
use utoipa_scalar::{Scalar, Servable};

pub async fn run() -> Result<()> {
  #[derive(OpenApi)]
  #[openapi(
    modifiers(),
    nest(
      (path = "/journal", api = crate::journal::JournalApi),
      (path = "/node", api = crate::node::NodeApi),
    ),
  )]
  struct ApiDoc;

  let data_dir = dirs::data_dir().unwrap();
  let panorama_dir = data_dir.join("panorama");
  fs::create_dir_all(&panorama_dir)?;

  let state = AppState::new(&panorama_dir).await?;

  state.install_apps_from_search_paths().await?;

  let cors_layer = CorsLayer::new()
    .allow_methods([Method::GET, Method::POST, Method::PUT])
    .allow_headers(cors::Any)
    .allow_origin(cors::Any);

  let trace_layer = TraceLayer::new_for_http();

  // build our application with a single route
  let app = Router::new()
    .merge(Scalar::with_url("/api/docs", ApiDoc::openapi()))
    .route("/", get(|| async { "Hello, World!" }))
    .nest("/node", node::router().with_state(state.clone()))
    .nest("/journal", journal::router().with_state(state.clone()))
    // .route("/mail/config", get(get_mail_config))
    // .route("/mail", get(get_mail))
    .layer(ServiceBuilder::new().layer(cors_layer))
    .layer(ServiceBuilder::new().layer(trace_layer))
    .with_state(state.clone());

  let listener = TcpListener::bind("0.0.0.0:5195").await?;
  println!("Listening... {:?}", listener);
  axum::serve(listener, app).await?;

  Ok(())
}

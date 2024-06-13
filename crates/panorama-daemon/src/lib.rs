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
pub mod mail;
mod node;

use std::fs;

use axum::{http::Method, routing::get, Router};
use miette::{IntoDiagnostic, Result};
use panorama_core::AppState;
use tokio::net::TcpListener;
use tower::ServiceBuilder;
use tower_http::cors::{self, CorsLayer};
use utoipa::OpenApi;
use utoipa_scalar::{Scalar, Servable};

use crate::{
  export::export,
  mail::{get_mail, get_mail_config},
  node::search_nodes,
};

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
  fs::create_dir_all(&panorama_dir).into_diagnostic()?;

  let state = AppState::new(&panorama_dir).await?;

  let cors = CorsLayer::new()
    .allow_methods([Method::GET, Method::POST, Method::PUT])
    .allow_headers(cors::Any)
    .allow_origin(cors::Any);

  // build our application with a single route
  let app = Router::new()
    .merge(Scalar::with_url("/api/docs", ApiDoc::openapi()))
    .route("/", get(|| async { "Hello, World!" }))
    .route("/export", get(export))
    .nest("/node", node::router().with_state(state.clone()))
    .nest("/journal", journal::router().with_state(state.clone()))
    .route("/mail/config", get(get_mail_config))
    .route("/mail", get(get_mail))
    .layer(ServiceBuilder::new().layer(cors))
    .with_state(state.clone());

  let listener = TcpListener::bind("0.0.0.0:5195").await.into_diagnostic()?;
  println!("Listening... {:?}", listener);
  axum::serve(listener, app).await.into_diagnostic()?;

  Ok(())
}

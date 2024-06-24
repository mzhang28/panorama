use axum::{
  routing::{method_routing, MethodFilter},
  Router,
};
use panorama_core::AppState;
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(paths(), components(schemas()))]
pub(super) struct AppsApi;

pub(super) fn router() -> Router<AppState> {
  Router::new()
  // .route("/app/:id/*path", method_routing::any(handler))
}

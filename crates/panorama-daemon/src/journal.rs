use axum::{Router};
use utoipa::OpenApi;

use crate::{AppState};

/// Node API
#[derive(OpenApi)]
#[openapi(paths(), components(schemas()))]
pub(super) struct JournalApi;

pub(super) fn router() -> Router<AppState> {
  Router::new()
  // .route("/get_todays_journal_id", get(get_todays_journal_id))
}

// #[utoipa::path(
//   get,
//   path = "/get_todays_journal_id",
//   responses(
//     (status = 200),
//   ),
// )]
// pub async fn get_todays_journal_id(
//   State(state): State<AppState>,
// ) -> AppResult<Json<Value>> {
//   let node_id = state.get_todays_journal_id().await?;

//   Ok(Json(json!({
//     "node_id": node_id.to_string(),
//   })))
// }

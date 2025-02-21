use axum::extract::State;

use crate::context::Context;

#[derive(Debug, Serialize, Deserialize)]
pub struct DataVizOptions {
  query_string: String,
}

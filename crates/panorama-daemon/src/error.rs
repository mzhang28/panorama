use axum::{
  http::StatusCode,
  response::{IntoResponse, Response},
};


pub type AppResult<T, E = AppError> = std::result::Result<T, E>;

// Make our own error that wraps `anyhow::Error`.
pub struct AppError(miette::Report);

// Tell axum how to convert `AppError` into a response.
impl IntoResponse for AppError {
  fn into_response(self) -> Response {
    eprintln!("Encountered error: {}", self.0);
    (
      StatusCode::INTERNAL_SERVER_ERROR,
      format!("Something went wrong: {}", self.0),
    )
      .into_response()
  }
}

// This enables using `?` on functions that return `Result<_, anyhow::Error>` to turn them into
// `Result<_, AppError>`. That way you don't need to do that manually.
impl<E> From<E> for AppError
where
  E: Into<miette::Report>,
{
  fn from(err: E) -> Self {
    Self(err.into())
  }
}

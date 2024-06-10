use std::{
  fs::{self, File},
  path::PathBuf,
};

use axum::extract::State;
use miette::IntoDiagnostic;

use crate::{error::AppResult, AppState};

// This code is really bad but gives me a quick way to look at all of the data
// in the data at once. Rip this out once there's any Real Security Mechanism.
pub async fn export(State(state): State<AppState>) -> AppResult<()> {
  let export = state.export().await?;

  let base_dir = PathBuf::from("export");
  fs::create_dir_all(&base_dir).into_diagnostic()?;

  let file = File::create(base_dir.join("export.json")).into_diagnostic()?;

  serde_json::to_writer_pretty(file, &export).into_diagnostic()?;

  Ok(())
}

use std::{
  fs,
  path::{Path, PathBuf},
};

use miette::{IntoDiagnostic, Result};

use crate::AppState;

impl AppState {
  pub async fn install_apps_from_search_paths(&self) -> Result<()> {
    let search_paths =
      vec![PathBuf::from("/Users/michael/Projects/panorama/apps")];

    let mut found = Vec::new();

    for path in search_paths {
      let read_dir = fs::read_dir(path).into_diagnostic()?;

      for dir_entry in read_dir {
        let dir_entry = dir_entry.into_diagnostic()?;
        let path = dir_entry.path();

        let manifest_path = path.join("manifest.yml");
        if manifest_path.exists() {
          found.push(path);
        }
      }
    }

    for path in found {
      self.install_app_from_path(path).await?;
    }

    Ok(())
  }

  async fn install_app_from_path(&self, path: impl AsRef<Path>) -> Result<()> {
    let app_path = path.as_ref();
    let manifest_path = app_path.join("manifest.yml");

    // Install tables

    Ok(())
  }
}

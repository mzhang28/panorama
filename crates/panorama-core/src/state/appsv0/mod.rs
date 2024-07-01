pub mod manifest;

use std::{
  collections::HashMap,
  fs::{self, File},
  path::{Path, PathBuf},
};

use anyhow::{Context as _, Result};

use crate::AppState;

use self::manifest::AppManifest;

impl AppState {
  pub async fn install_apps_from_search_paths(&self) -> Result<()> {
    let search_paths = vec![
      PathBuf::from("/Users/michael/Projects/panorama/apps"),
      PathBuf::from("/home/michael/Projects/panorama/apps"),
    ];

    let mut found = Vec::new();

    for path in search_paths {
      if !path.exists() {
        continue;
      }

      let read_dir = fs::read_dir(&path)
        .with_context(|| format!("could not read {}", path.display()))?;

      for dir_entry in read_dir {
        let dir_entry = dir_entry?;
        let path = dir_entry.path();

        let manifest_path = path.join("manifest.yml");
        if manifest_path.exists() {
          found.push(path);
        }
      }
    }

    // let mut all_app_data = HashMap::new();
    // for path in found {
    //   let app_data = self.install_app_from_path(&path).await?;
    //   println!("App data: {:?}", app_data);
    //   all_app_data.insert(
    //     path.display().to_string(),
    //     AppData {
    //       name: "hello".to_string(),
    //     },
    //   );
    // }

    Ok(())
  }

  async fn install_app_from_path(&self, path: impl AsRef<Path>) -> Result<()> {
    let app_path = path.as_ref();
    let manifest_path = app_path.join("manifest.yml");
    let manifest: AppManifest = {
      let file = File::open(&manifest_path)?;
      serde_yaml::from_reader(file).with_context(|| {
        format!(
          "Could not parse config file from {}",
          manifest_path.display()
        )
      })?
    };
    println!("Manifest: {:?}", manifest);

    todo!()
  }
}

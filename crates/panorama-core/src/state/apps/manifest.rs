use std::path::PathBuf;

use schemars::JsonSchema;

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct AppManifest {
  pub name: String,
  pub version: Option<String>,
  pub panorama_version: Option<String>,
  pub description: Option<String>,
  pub installer_path: PathBuf,

  pub endpoints: Vec<AppManifestEndpoint>,
  pub triggers: Vec<AppManifestTriggers>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct AppManifestEndpoint {
  pub url: String,
  pub method: String,
  pub export_name: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct AppManifestTriggers {}

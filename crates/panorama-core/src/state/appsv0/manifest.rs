use std::path::PathBuf;

use schemars::JsonSchema;

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct AppManifest {
  pub name: String,
  pub version: Option<String>,
  pub panorama_version: Option<String>,
  pub description: Option<String>,
  pub module: PathBuf,

  #[serde(default)]
  pub endpoints: Vec<AppManifestEndpoint>,
  #[serde(default)]
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

use std::path::PathBuf;

use schemars::JsonSchema;

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct AppManifest {
  name: String,
  version: Option<String>,
  panorama_version: Option<String>,
  description: Option<String>,
  installer_path: PathBuf,

  endpoints: Vec<AppManifestEndpoint>,
  triggers: Vec<AppManifestTriggers>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct AppManifestEndpoint {
  url: String,
  method: String,
  export_name: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct AppManifestTriggers {}

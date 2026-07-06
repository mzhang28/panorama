use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize)]
pub struct WasmInput {
  pub endpoint: String,
  pub request: WasmHttpRequest,
  #[serde(default)]
  pub nodes: Vec<WasmNode>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WasmHttpRequest {
  pub method: String,
  pub path: String,
  #[serde(default)]
  pub query_params: HashMap<String, String>,
  #[serde(default)]
  pub headers: HashMap<String, String>,
  pub body: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmNode {
  pub id: String,
  #[serde(default)]
  pub fields: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct WasmOutput {
  pub status: u16,
  #[serde(default)]
  pub headers: HashMap<String, String>,
  #[serde(default)]
  pub body: serde_json::Value,
  #[serde(default)]
  pub effects: Vec<serde_json::Value>,
}

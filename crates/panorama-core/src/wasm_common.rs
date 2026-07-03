//! Common types shared between the WASM runtime (server) and WASM plugin entry points.
//! This is a minimal no-std-compatible module that both sides can use.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Input to a WASM plugin handler (sent via stdin)
#[derive(Debug, Serialize, Deserialize)]
pub struct WasmInput {
    pub endpoint: String,
    pub request: WasmHttpRequest,
    /// All nodes pre-loaded from the database
    #[serde(default)]
    pub nodes: Vec<WasmNode>,
}

/// HTTP request data
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

/// A simplified node representation for WASM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmNode {
    pub id: String,
    #[serde(default)]
    pub fields: HashMap<String, WasmFieldValue>,
    pub created_at: String,
    pub updated_at: String,
}

/// Field value for WASM
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum WasmFieldValue {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    DateTime(String),
    NodeRef(String),
    ObjectRef(WasmObjectRef),
    Json(serde_json::Value),
    Array(Vec<WasmFieldValue>),
    Binary(Vec<u8>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmObjectRef {
    pub bucket: String,
    pub key: String,
    pub size: u64,
}

/// Output from a WASM plugin handler (written to stdout)
#[derive(Debug, Serialize, Deserialize)]
pub struct WasmOutput {
    pub status: u16,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(default)]
    pub body: serde_json::Value,
    #[serde(default)]
    pub effects: Vec<WasmEffect>,
}

/// Side effects the WASM module requests the server to perform
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WasmEffect {
    #[serde(rename = "create_node")]
    CreateNode {
        fields: HashMap<String, WasmFieldValue>,
    },
    #[serde(rename = "update_node")]
    UpdateNode {
        id: String,
        fields: HashMap<String, WasmFieldValue>,
    },
    #[serde(rename = "delete_node")]
    DeleteNode { id: String },
}

/// Helper to check if a field exists and matches a string value
pub fn node_has_field_str(node: &WasmNode, key: &str, value: &str) -> bool {
    node.fields.get(key).map_or(false, |v| match v {
        WasmFieldValue::String(s) => s == value,
        _ => false,
    })
}

/// Helper to get a string field from a node
pub fn node_field_str<'a>(node: &'a WasmNode, key: &str) -> Option<&'a str> {
    node.fields.get(key).and_then(|v| match v {
        WasmFieldValue::String(s) => Some(s.as_str()),
        _ => None,
    })
}

/// Helper to create a response with JSON body
pub fn json_response(data: serde_json::Value) -> WasmOutput {
    let mut headers = HashMap::new();
    headers.insert("Content-Type".to_string(), "application/json".to_string());
    WasmOutput {
        status: 200,
        headers,
        body: data,
        effects: vec![],
    }
}

/// Helper to create an error response
pub fn error_response(status: u16, message: &str) -> WasmOutput {
    WasmOutput {
        status,
        headers: HashMap::new(),
        body: serde_json::json!({"error": message}),
        effects: vec![],
    }
}

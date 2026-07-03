use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Read;

#[derive(Debug, Deserialize)]
pub struct WasmInput { pub endpoint: String, pub request: WasmHttpRequest, #[serde(default)] pub nodes: Vec<WasmNode> }
#[derive(Debug, Deserialize)]
pub struct WasmHttpRequest { pub method: String, pub path: String, #[serde(default)] pub query_params: HashMap<String, String>, #[serde(default)] pub headers: HashMap<String, String>, pub body: Option<String> }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmNode { pub id: String, #[serde(default)] pub fields: HashMap<String, serde_json::Value> }
#[derive(Debug, Serialize)]
pub struct WasmOutput { pub status: u16, #[serde(default)] pub headers: HashMap<String, String>, #[serde(default)] pub body: serde_json::Value, #[serde(default)] pub effects: Vec<serde_json::Value> }
pub fn json_ok(data: serde_json::Value) -> WasmOutput { let mut h = HashMap::new(); h.insert("Content-Type".to_string(), "application/json".to_string()); WasmOutput { status: 200, headers: h, body: data, effects: vec![] } }
pub fn not_found() -> WasmOutput { let mut h = HashMap::new(); h.insert("Content-Type".to_string(), "application/json".to_string()); WasmOutput { status: 404, headers: h, body: serde_json::json!({"error":"not found"}), effects: vec![] } }
pub fn create_effect(fields: HashMap<String, serde_json::Value>) -> serde_json::Value { serde_json::json!({"type":"create_node","fields":fields}) }
pub fn field_value(ty: &str, value: impl Into<serde_json::Value>) -> serde_json::Value { serde_json::json!({"type": ty, "value": value.into()}) }

pub fn read_input() -> WasmInput {
    let mut buf = String::new();
    std::io::stdin().read_to_string(&mut buf).ok();
    serde_json::from_str(&buf).unwrap_or_else(|_| WasmInput {
        endpoint: "error".to_string(),
        request: WasmHttpRequest { method: "GET".to_string(), path: "/".to_string(), query_params: HashMap::new(), headers: HashMap::new(), body: Some(buf) },
        nodes: vec![],
    })
}

pub fn write_output(output: &WasmOutput) {
    println!("{}", serde_json::to_string(output).unwrap_or_default());
}

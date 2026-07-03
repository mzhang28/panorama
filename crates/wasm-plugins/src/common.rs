use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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

pub fn read_input() -> (WasmInput, String) {
    let args: Vec<String> = std::env::args().collect();
    let in_path = args.get(1).cloned().unwrap_or_else(|| "input.json".to_string());
    let out_path = args.get(2).cloned().unwrap_or_else(|| "output.json".to_string());
    // Try multiple paths since WASI mounts the work dir at /
    let content = std::fs::read_to_string(&in_path)
        .or_else(|_| std::fs::read_to_string(format!("/{}", in_path)))
        .or_else(|_| std::fs::read_to_string("/input.json"))
        .unwrap_or_else(|_| "{}".to_string());
    let input: WasmInput = serde_json::from_str(&content).unwrap_or_else(|e| {
        let err = format!("{{\"error\":\"parse: {}\",\"raw\":\"{}\"}}", e, &content[..content.len().min(200)]);
        serde_json::from_str(&err).unwrap()
    });
    (input, out_path)
}

pub fn write_output(output: &WasmOutput, out_path: &str) {
    let json = serde_json::to_string(output).unwrap_or_default();
    std::fs::write(out_path, &json).or_else(|_| std::fs::write(format!("/{}", out_path), &json)).ok();
}

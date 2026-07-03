use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Read;

// ── Host function imports ───────────────────────────────────────────────────
#[link(wasm_import_module = "env")]
extern "C" {
    fn host_query(query_ptr: i32, query_len: i32, result_ptr: i32) -> i32;
    fn host_create_node(fields_ptr: i32, fields_len: i32);
    fn host_delete_node(id_ptr: i32, id_len: i32);
}

// ── Types ───────────────────────────────────────────────────────────────────
#[derive(Debug, Deserialize, Clone)]
pub struct WasmInput { pub endpoint: String, pub request: WasmHttpRequest }

#[derive(Debug, Deserialize, Clone)]
pub struct WasmHttpRequest {
    pub method: String,
    pub path: String,
    #[serde(default)]
    pub query_params: HashMap<String, String>,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct WasmOutput {
    pub status: u16,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(default)]
    pub body: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmNode {
    pub id: String,
    #[serde(default)]
    pub fields: HashMap<String, serde_json::Value>,
}

// ── I/O via WASI stdin/stdout ───────────────────────────────────────────────
pub fn read_input() -> WasmInput {
    let mut buf = String::new();
    std::io::stdin().read_to_string(&mut buf).ok();
    serde_json::from_str(&buf).unwrap_or_else(|_| WasmInput {
        endpoint: "error".to_string(),
        request: WasmHttpRequest { method: "GET".to_string(), path: "/".to_string(), query_params: HashMap::new(), headers: HashMap::new(), body: Some(buf) },
    })
}

pub fn write_output(output: &WasmOutput) {
    println!("{}", serde_json::to_string(output).unwrap_or_default());
}

// ── Helpers ─────────────────────────────────────────────────────────────────
pub fn json_ok(data: serde_json::Value) -> WasmOutput {
    let mut h = HashMap::new(); h.insert("Content-Type".to_string(), "application/json".to_string());
    WasmOutput { status: 200, headers: h, body: data }
}

pub fn not_found() -> WasmOutput {
    let mut h = HashMap::new(); h.insert("Content-Type".to_string(), "application/json".to_string());
    WasmOutput { status: 404, headers: h, body: serde_json::json!({"error":"not found"}) }
}

pub fn field_value(ty: &str, value: impl Into<serde_json::Value>) -> serde_json::Value {
    serde_json::json!({"type": ty, "value": value.into()})
}

pub fn row_to_wasm_node(row: &serde_json::Value) -> Option<WasmNode> {
    let obj = row.as_object()?;
    let id = obj.get("id")?.as_str()?.to_string();
    let fields_val = obj.get("fields_json")?;
    let fields: HashMap<String, serde_json::Value> = match fields_val {
        serde_json::Value::String(s) => serde_json::from_str(s).unwrap_or_default(),
        serde_json::Value::Object(_) => serde_json::from_value(fields_val.clone()).unwrap_or_default(),
        _ => HashMap::new(),
    };
    Some(WasmNode { id, fields })
}

pub fn fstr(n: &WasmNode, k: &str) -> Option<String> {
    n.fields.get(k)?.get("value")?.as_str().map(|s| s.to_string())
}

pub fn ff64(n: &WasmNode, k: &str) -> Option<f64> {
    n.fields.get(k)?.get("value")?.as_f64()
}

// ── Host function wrappers ──────────────────────────────────────────────────

/// Run a Panorama Query Language query and get JSON rows back.
pub fn host_run_query(qs: &str) -> Vec<serde_json::Value> {
    unsafe {
        let qb = qs.as_bytes();
        let mut buf = vec![0u8; 65536];
        let result_len = host_query(qb.as_ptr() as i32, qb.len() as i32, buf.as_mut_ptr() as i32);
        if result_len <= 0 {
            return Vec::new();
        }
        let len = (result_len as usize).min(buf.len());
        serde_json::from_slice(&buf[..len]).unwrap_or_default()
    }
}

/// Create a node on the host.
pub fn host_create(fields: &HashMap<String, serde_json::Value>) {
    unsafe {
        let json = serde_json::to_string(fields).unwrap_or_default();
        host_create_node(json.as_ptr() as i32, json.len() as i32);
    }
}

/// Delete a node by ID.
pub fn host_delete(id: &str) {
    unsafe {
        host_delete_node(id.as_ptr() as i32, id.len() as i32);
    }
}

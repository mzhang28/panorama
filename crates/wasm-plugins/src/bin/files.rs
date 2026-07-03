mod common { include!("../common.rs"); }
use common::*;
use std::collections::HashMap;
fn main() { let (input, out_path) = read_input(); let output = handle(&input); write_output(&output, &out_path); }
fn handle(input: &WasmInput) -> WasmOutput {
    match (input.request.method.as_str(), input.endpoint.as_str()) {
        ("POST", "upload") => { let filename = input.request.query_params.get("filename").cloned().unwrap_or_else(|| "unnamed".to_string()); let mime = input.request.query_params.get("mime_type").cloned().unwrap_or_else(|| "application/octet-stream".to_string()); let body_len = input.request.body.as_ref().map(|b| b.len()).unwrap_or(0); let mut fields = HashMap::new(); fields.insert("system:node_title".into(), field_value("String", filename)); fields.insert("files:object_ref".into(), serde_json::json!({"type":"ObjectRef","value":{"bucket":"files","key":"test","size":body_len}})); fields.insert("files:file_size".into(), field_value("Integer", body_len as i64)); fields.insert("files:mime_type".into(), field_value("String", mime)); let mut out = json_ok(serde_json::json!({"uploaded":true,"size":body_len})); out.effects.push(create_effect(fields)); out }
        ("GET", "files") => { let files: Vec<&WasmNode> = input.nodes.iter().filter(|n| n.fields.contains_key("files:object_ref")).collect(); json_ok(serde_json::json!(files)) }
        (_, ep) if ep.starts_with("files/") => { let id = &ep["files/".len()..]; match input.request.method.as_str() { "GET" => { let file = input.nodes.iter().find(|n| n.id == id); if let Some(f) = file { let mut h = HashMap::new(); h.insert("Content-Type".into(), "application/octet-stream".to_string()); let filename = f.fields.get("system:node_title").and_then(|v| v["value"].as_str()).unwrap_or("file"); h.insert("Content-Disposition".into(), format!("attachment; filename=\"{}\"", filename)); WasmOutput { status: 200, headers: h, body: serde_json::json!("binary-data"), effects: vec![] } } else { not_found() } } "DELETE" => { let mut out = json_ok(serde_json::json!({"deleted":true})); out.effects.push(serde_json::json!({"type":"delete_node","id":id})); out.status = 204; out } _ => not_found() } }
        _ => not_found()
    }
}

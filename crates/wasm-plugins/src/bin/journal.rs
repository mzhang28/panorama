mod common { include!("../common.rs"); }
use common::*;
use std::collections::HashMap;
fn main() { let input = read_input(); let output = handle(&input); write_output(&output); }
fn handle(input: &WasmInput) -> WasmOutput {
    match (input.request.method.as_str(), input.endpoint.as_str()) {
        ("POST", "entries") => {
            let body: serde_json::Value = input.request.body.as_deref().and_then(|b| serde_json::from_str(b).ok()).unwrap_or_default();
            let mut fields = HashMap::new();
            fields.insert("system:node_title".into(), field_value("String", body["title"].as_str().unwrap_or("Untitled")));
            fields.insert("system:node_time".into(), field_value("DateTime", "2025-01-01T00:00:00Z"));
            fields.insert("journal:content".into(), field_value("String", body["content"].as_str().unwrap_or("")));
            if let Some(m) = body["mood"].as_str() { fields.insert("journal:mood".into(), field_value("String", m)); }
            let mut out = json_ok(serde_json::json!({"created":true})); out.effects.push(create_effect(fields)); out
        }
        ("GET", "entries") => {
            let mut entries: Vec<&WasmNode> = input.nodes.iter().filter(|n| n.fields.contains_key("journal:content")).collect();
            entries.sort_by(|a,b| { let ta = a.fields.get("system:node_time").and_then(|v| v["value"].as_str()).unwrap_or(""); let tb = b.fields.get("system:node_time").and_then(|v| v["value"].as_str()).unwrap_or(""); tb.cmp(ta) });
            json_ok(serde_json::json!(entries))
        }
        _ => not_found()
    }
}

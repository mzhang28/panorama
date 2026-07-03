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
            host_create(&fields);
            json_ok(serde_json::json!({"created":true}))
        }
        ("GET", "entries") => {
            let rows = host_run_query("MATCH (n) IN space(\"default\") WHERE HAS_FIELD(n, \"journal\", \"content\") RETURN n ORDER BY n.system.node_time DESC LIMIT 100");
            let nodes: Vec<WasmNode> = rows.iter().filter_map(row_to_wasm_node).collect();
            json_ok(serde_json::json!(nodes))
        }
        _ => not_found()
    }
}

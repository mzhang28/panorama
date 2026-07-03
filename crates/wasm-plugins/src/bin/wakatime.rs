mod common { include!("../common.rs"); }
use common::*;
use std::collections::HashMap;
fn main() { let input = read_input(); let output = handle(&input); write_output(&output); }
fn handle(input: &WasmInput) -> WasmOutput {
    match (input.request.method.as_str(), input.endpoint.as_str()) {
        ("POST", "heartbeat") | ("POST", "heartbeats") => {
            let body: serde_json::Value = input.request.body.as_deref().and_then(|b| serde_json::from_str(b).ok()).unwrap_or_default();
            let hbs: Vec<&serde_json::Value> = if body.is_array() { body.as_array().unwrap().iter().collect() } else { vec![&body] };
            let mut out = json_ok(serde_json::json!({"status":"ok","created":0}));
            for hb in &hbs {
                let mut fields = HashMap::new();
                fields.insert("system:node_time".into(), field_value("DateTime", "2025-01-01T00:00:00Z"));
                if let Some(v) = hb["entity"].as_str() { fields.insert("wakatime:entity".into(), field_value("String", v)); }
                if let Some(v) = hb["project"].as_str() { fields.insert("wakatime:project".into(), field_value("String", v)); }
                if let Some(v) = hb["language"].as_str() { fields.insert("wakatime:language".into(), field_value("String", v)); }
                if let Some(v) = hb["duration"].as_f64() { fields.insert("wakatime:duration".into(), field_value("Float", v)); }
                out.effects.push(create_effect(fields));
            }
            out.body = serde_json::json!({"status":"ok","created":hbs.len()}); out
        }
        _ => not_found()
    }
}

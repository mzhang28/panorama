mod common { include!("../common.rs"); }
use common::*;
use std::collections::HashMap;

fn main() { let input = read_input(); let output = handle(&input); write_output(&output); }

fn handle(input: &WasmInput) -> WasmOutput {
    match (input.request.method.as_str(), input.endpoint.as_str()) {
        ("POST", "trips") => {
            let body: serde_json::Value = input.request.body.as_deref().and_then(|b| serde_json::from_str(b).ok()).unwrap_or_default();
            let mut fields = HashMap::new();
            fields.insert("system:node_title".into(), field_value("String", body["title"].as_str().unwrap_or("Untitled")));
            if let Some(d) = body["start_date"].as_str() { fields.insert("trips:start_date".into(), field_value("DateTime", d)); }
            if let Some(d) = body["end_date"].as_str() { fields.insert("trips:end_date".into(), field_value("DateTime", d)); }
            host_create(&fields);
            json_ok(serde_json::json!({"created":true}))
        }
        ("GET", "trips") => {
            let rows = host_run_query("MATCH (n) IN space(\"default\") WHERE HAS_FIELD(n, \"trips\", \"start_date\") RETURN n");
            let nodes: Vec<WasmNode> = rows.iter().filter_map(row_to_wasm_node).collect();
            json_ok(serde_json::json!(nodes))
        }
        ("POST", "events") => {
            let body: serde_json::Value = input.request.body.as_deref().and_then(|b| serde_json::from_str(b).ok()).unwrap_or_default();
            let mut fields = HashMap::new();
            fields.insert("system:node_title".into(), field_value("String", body["title"].as_str().unwrap_or("Untitled")));
            if let Some(v) = body["start_time"].as_str() { fields.insert("system:node_start_time".into(), field_value("DateTime", v)); }
            if let Some(v) = body["trip_id"].as_str() { fields.insert("trips:trip_id".into(), field_value("String", v)); }
            if let Some(v) = body["latitude"].as_f64() { fields.insert("trips:latitude".into(), field_value("Float", v)); }
            if let Some(v) = body["longitude"].as_f64() { fields.insert("trips:longitude".into(), field_value("Float", v)); }
            if let Some(v) = body["location_name"].as_str() { fields.insert("trips:location_name".into(), field_value("String", v)); }
            host_create(&fields);
            json_ok(serde_json::json!({"created":true}))
        }
        ("GET", "events") => {
            let tid = input.request.query_params.get("trip_id");
            let rows = host_run_query("MATCH (n) IN space(\"default\") WHERE HAS_FIELD(n, \"trips\", \"trip_id\") RETURN n");
            let mut events: Vec<WasmNode> = rows.iter().filter_map(row_to_wasm_node).collect();
            if let Some(t) = tid {
                events.retain(|n| fstr(n, "trips:trip_id").as_deref() == Some(t.as_str()));
            }
            json_ok(serde_json::json!(events))
        }
        ("GET", "events/map") => {
            let rows = host_run_query("MATCH (n) IN space(\"default\") WHERE HAS_FIELD(n, \"trips\", \"latitude\") RETURN n");
            let nodes: Vec<WasmNode> = rows.iter().filter_map(row_to_wasm_node).collect();
            let map: Vec<serde_json::Value> = nodes.iter()
                .filter(|n| n.fields.contains_key("trips:latitude") && n.fields.contains_key("trips:longitude"))
                .map(|n| serde_json::json!({
                    "id": n.id,
                    "title": fstr(n, "system:node_title").unwrap_or_default(),
                    "latitude": ff64(n, "trips:latitude").unwrap_or(0.0),
                    "longitude": ff64(n, "trips:longitude").unwrap_or(0.0),
                    "location": fstr(n, "trips:location_name").unwrap_or_default()
                }))
                .collect();
            json_ok(serde_json::json!(map))
        }
        _ => not_found()
    }
}

mod common { include!("../common.rs"); }
use common::*;
use std::collections::HashMap;

fn main() { let input = read_input(); let output = handle(&input); write_output(&output); }

fn handle(input: &WasmInput) -> WasmOutput {
    match (input.request.method.as_str(), input.endpoint.as_str()) {
        ("POST", "upload") => {
            let filename = input.request.query_params.get("filename").cloned().unwrap_or_else(|| "unnamed".to_string());
            let mime = input.request.query_params.get("mime_type").cloned().unwrap_or_else(|| "application/octet-stream".to_string());
            let body_len = input.request.body.as_ref().map(|b| b.len()).unwrap_or(0);
            let mut fields = HashMap::new();
            fields.insert("system:node_title".into(), field_value("String", filename));
            fields.insert("files:object_ref".into(), serde_json::json!({"type":"Json","value":{"bucket":"files","key":"test"}}));
            fields.insert("files:file_size".into(), field_value("Integer", body_len as i64));
            fields.insert("files:mime_type".into(), field_value("String", mime));
            host_create(&fields);
            json_ok(serde_json::json!({"uploaded":true,"size":body_len}))
        }
        ("GET", "files") => {
            let rows = host_run_query("MATCH (n) IN space(\"default\") WHERE HAS_FIELD(n, \"files\", \"object_ref\") RETURN n ORDER BY n.system.updated_at DESC");
            let nodes: Vec<WasmNode> = rows.iter().filter_map(row_to_wasm_node).collect();
            json_ok(serde_json::json!(nodes))
        }
        (_, ep) if ep.starts_with("files/") => {
            let id = &ep["files/".len()..];
            match input.request.method.as_str() {
                "GET" => {
                    let rows = host_run_query("MATCH (n) IN space(\"default\") WHERE HAS_FIELD(n, \"files\", \"object_ref\") RETURN n");
                    let nodes: Vec<WasmNode> = rows.iter().filter_map(row_to_wasm_node).collect();
                    let file = nodes.iter().find(|n| n.id == id);
                    if let Some(f) = file {
                        let mut h = HashMap::new();
                        h.insert("Content-Type".to_string(), "application/octet-stream".to_string());
                        let filename = fstr(f, "system:node_title").unwrap_or_else(|| "file".to_string());
                        h.insert("Content-Disposition".to_string(), format!("attachment; filename=\"{}\"", filename));
                        WasmOutput { status: 200, headers: h, body: serde_json::json!("binary-data") }
                    } else {
                        not_found()
                    }
                }
                "DELETE" => {
                    host_delete(id);
                    WasmOutput { status: 204, headers: HashMap::new(), body: serde_json::json!({"deleted":true}) }
                }
                _ => not_found()
            }
        }
        _ => not_found()
    }
}

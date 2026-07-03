//! WASM runtime for .panoapp plugin execution.
//! Uses the `wasmtime` CLI as a subprocess with file-based I/O via WASI.
//!
//! Protocol:
//!   1. Server creates a temp directory with input.json
//!   2. Runs: wasmtime run --dir=<workdir> plugin.wasm -- input.json output.json
//!   3. WASM module reads input.json, writes output.json
//!   4. Server reads output.json, executes effects, returns HTTP response

use std::collections::HashMap;
use std::io::Write;
use std::process::Command;

use bytes::Bytes;
use panorama_core::plugin::{HttpRequest, HttpResponse, PluginError};
use panorama_core::types::{FieldValue, Node};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::storage::NodeStorage;
use crate::object_store::ObjectStorage;

#[derive(Debug, Serialize)]
struct WasmInput {
    endpoint: String,
    request: WasmHttpRequest,
    nodes: Vec<WasmNodeData>,
}

#[derive(Debug, Serialize)]
struct WasmHttpRequest {
    method: String,
    path: String,
    #[serde(default)]
    query_params: HashMap<String, String>,
    #[serde(default)]
    headers: HashMap<String, String>,
    body: Option<String>,
}

#[derive(Debug, Serialize)]
struct WasmNodeData {
    id: String,
    fields: HashMap<String, serde_json::Value>,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Deserialize)]
struct WasmOutput {
    status: u16,
    #[serde(default)]
    headers: HashMap<String, String>,
    #[serde(default)]
    body: serde_json::Value,
    #[serde(default)]
    effects: Vec<WasmEffect>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum WasmEffect {
    #[serde(rename = "create_node")]
    CreateNode { fields: HashMap<String, FieldValue> },
    #[serde(rename = "update_node")]
    UpdateNode { id: String, fields: HashMap<String, FieldValue> },
    #[serde(rename = "delete_node")]
    DeleteNode { id: String },
    #[serde(rename = "put_object")]
    PutObject { bucket: String, key: String, data_base64: String, mime_type: String },
}

pub fn execute_wasm_handler(
    wasm_bytes: &[u8],
    endpoint: &str,
    request: &HttpRequest,
    storage: &NodeStorage,
    object_storage: &ObjectStorage,
) -> Result<HttpResponse, PluginError> {
    // ── Build input ──
    let nodes: Vec<_> = storage.query(&HashMap::new(), None, None);
    let wasm_nodes: Vec<WasmNodeData> = nodes.iter().map(|n| {
        let fields: HashMap<_, _> = n.fields.iter()
            .map(|(k, v)| (k.clone(), serde_json::to_value(v).unwrap_or_default()))
            .collect();
        WasmNodeData { id: n.id.to_string(), fields, created_at: n.created_at.to_rfc3339(), updated_at: n.updated_at.to_rfc3339() }
    }).collect();

    let input = WasmInput {
        endpoint: endpoint.to_string(),
        request: WasmHttpRequest {
            method: request.method.clone(), path: request.path.clone(),
            query_params: request.query_params.clone(), headers: request.headers.clone(),
            body: request.body.as_ref().map(|b| String::from_utf8_lossy(b).to_string()),
        },
        nodes: wasm_nodes,
    };

    let input_json = serde_json::to_vec(&input).map_err(|e| PluginError::internal(format!("input: {}", e)))?;

    // ── Set up temp dir ──
    let work_dir = std::env::temp_dir().join(format!("pano-wasm-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&work_dir).map_err(|e| PluginError::internal(format!("dir: {}", e)))?;

    // Write the WASM module to a temp file
    let wasm_path = work_dir.join("plugin.wasm");
    std::fs::write(&wasm_path, wasm_bytes).map_err(|e| PluginError::internal(format!("wasm write: {}", e)))?;

    // ── Run wasmtime CLI with stdin pipe ──
    let mut child = Command::new("wasmtime")
        .arg("run")
        .arg(wasm_path.to_str().unwrap_or("plugin.wasm"))
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| PluginError::internal(format!("wasmtime spawn: {}", e)))?;

    // Write input JSON to stdin
    use std::io::Write;
    if let Some(ref mut stdin) = child.stdin {
        stdin.write_all(&input_json).ok();
    }
    // stdin is dropped here (closed) so WASM gets EOF

    let output = child.wait_with_output()
        .map_err(|e| PluginError::internal(format!("wasmtime wait: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        std::fs::remove_dir_all(&work_dir).ok();
        return Err(PluginError::internal(format!("WASM error: {}", stderr)));
    }

    // ── Parse stdout as JSON ──
    let output_str = String::from_utf8_lossy(&output.stdout);
    let wasm_output: WasmOutput = serde_json::from_str(&output_str).map_err(|e| {
        PluginError::internal(format!("WASM stdout parse: {}. Raw: {}", e, &output_str[..output_str.len().min(300)]))
    })?;

    std::fs::remove_dir_all(&work_dir).ok();

    // ── Execute effects ──
    for effect in &wasm_output.effects {
        match effect {
            WasmEffect::CreateNode { fields } => {
                let mut node = Node::new(Uuid::nil());
                for (k, v) in fields { node.set_field(k, v.clone()); }
                storage.create(node).map_err(|e| PluginError::internal(e))?;
            }
            WasmEffect::UpdateNode { id, fields } => {
                if let Ok(uid) = Uuid::parse_str(id) {
                    storage.update(&uid, fields.clone()).map_err(|e| PluginError::internal(e))?;
                }
            }
            WasmEffect::DeleteNode { id } => {
                if let Ok(uid) = Uuid::parse_str(id) {
                    storage.delete(&uid).map_err(|e| PluginError::internal(e))?;
                }
            }
            WasmEffect::PutObject { bucket, key, data_base64, mime_type } => {
                use base64::Engine;
                let data = base64::engine::general_purpose::STANDARD
                    .decode(data_base64).map_err(|e| PluginError::internal(e.to_string()))?;
                object_storage.put(bucket, key, &data, mime_type).map_err(|e| PluginError::internal(e))?;
            }
        }
    }

    Ok(HttpResponse { status: wasm_output.status, headers: wasm_output.headers, body: Bytes::from(serde_json::to_string(&wasm_output.body).unwrap_or_default()) })
}

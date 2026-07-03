//! WASM runtime — wasmtime + WASI preview1 + host functions.

use std::collections::HashMap;

use bytes::Bytes;
use panorama_core::plugin::{HttpRequest, HttpResponse, PluginError};
use uuid::Uuid;

use crate::storage::NodeStorage;

type WasiCtx = wasmtime_wasi::preview1::WasiP1Ctx;

pub async fn execute_wasm_handler(
    wasm_bytes: &[u8],
    endpoint: &str,
    request: &HttpRequest,
    storage: &NodeStorage,
    _object_storage: &crate::object_store::ObjectStorage,
) -> Result<HttpResponse, PluginError> {
    let mut config = wasmtime::Config::new();
    config.async_support(true);
    let engine = wasmtime::Engine::new(&config)
        .map_err(|e| PluginError::internal(format!("wasm engine: {}", e)))?;

    let module = wasmtime::Module::from_binary(&engine, wasm_bytes)
        .map_err(|e| PluginError::internal(format!("wasm compile: {}", e)))?;

    // Build WASI context with stdin from request JSON
    let input_json = serde_json::to_vec(&serde_json::json!({
        "endpoint": endpoint,
        "request": {
            "method": &request.method, "path": &request.path,
            "query_params": &request.query_params, "headers": &request.headers,
            "body": request.body.as_ref().map(|b| String::from_utf8_lossy(b).to_string()),
        },
    })).map_err(|e| PluginError::internal(format!("json: {}", e)))?;

    let stdout_pipe = wasmtime_wasi::pipe::MemoryOutputPipe::new(65536);
    let stdin_pipe = wasmtime_wasi::pipe::MemoryInputPipe::new(Bytes::from(input_json));

    let mut builder = wasmtime_wasi::WasiCtxBuilder::new();
    builder.stdin(stdin_pipe);
    builder.stdout(stdout_pipe.clone());
    let wasi_ctx = builder.build_p1();

    let mut store = wasmtime::Store::new(&engine, wasi_ctx);
    let mut linker = wasmtime::Linker::new(&engine);

    wasmtime_wasi::preview1::wasi_snapshot_preview1::add_to_linker(
        &mut linker,
        |cx: &mut WasiCtx| cx,
    ).map_err(|e| PluginError::internal(format!("wasi: {}", e)))?;

    // ── Host functions ──────────────────────────────────────────────────

    let s = storage.clone();
    linker.func_wrap("env", "host_query",
        move |mut caller: wasmtime::Caller<'_, WasiCtx>, q_ptr: i32, q_len: i32, r_ptr: i32| -> i32 {
            let mem = match caller.get_export("memory").and_then(|e| e.into_memory()) {
                Some(m) => m,
                None => return 0,
            };
            let data = mem.data(&caller);
            let q_start = q_ptr as usize;
            let q_end = q_start.saturating_add(q_len as usize);
            if q_end > data.len() {
                return 0;
            }
            let qs = match std::str::from_utf8(&data[q_start..q_end]) {
                Ok(s) => s,
                Err(_) => return 0,
            };

            let rows = match s.query_lang(qs) {
                Ok(r) => r,
                Err(_) => return 0,
            };

            let json = serde_json::to_vec(&rows).unwrap_or_default();

            let mem = match caller.get_export("memory").and_then(|e| e.into_memory()) {
                Some(m) => m,
                None => return 0,
            };
            let data_mut = mem.data_mut(&mut caller);
            let r_start = r_ptr as usize;
            if r_start >= data_mut.len() {
                return 0;
            }
            let write_len = json.len().min(data_mut.len() - r_start);
            data_mut[r_start..r_start + write_len].copy_from_slice(&json[..write_len]);
            write_len as i32
        }
    ).map_err(|e| PluginError::internal(format!("link host_query: {}", e)))?;

    let s2 = storage.clone();
    linker.func_wrap("env", "host_create_node",
        move |mut caller: wasmtime::Caller<'_, WasiCtx>, ptr: i32, len: i32, r_ptr: i32| -> i32 {
            let mem = match caller.get_export("memory").and_then(|e| e.into_memory()) {
                Some(m) => m,
                None => return 0,
            };
            let data = mem.data(&caller);
            let start = ptr as usize;
            let end = start.saturating_add(len as usize);
            if end > data.len() { return 0; }
            let json = match std::str::from_utf8(&data[start..end]) {
                Ok(s) => s,
                Err(_) => return 0,
            };
            let id = if let Ok(fields) = serde_json::from_str::<HashMap<String, panorama_core::types::FieldValue>>(json) {
                let mut node = panorama_core::types::Node::new(Uuid::nil());
                for (k, v) in fields { node.set_field(&k, v); }
                let node_id = node.id;
                match s2.create(node) {
                    Ok(_) => node_id.to_string(),
                    Err(_) => return 0,
                }
            } else {
                return 0;
            };
            let id_bytes = id.as_bytes();
            let data_mut = mem.data_mut(&mut caller);
            let r_start = r_ptr as usize;
            if r_start >= data_mut.len() { return 0; }
            let write_len = id_bytes.len().min(data_mut.len() - r_start);
            data_mut[r_start..r_start + write_len].copy_from_slice(&id_bytes[..write_len]);
            write_len as i32
        }
    ).map_err(|e| PluginError::internal(format!("link host_create_node: {}", e)))?;

    let s3 = storage.clone();
    linker.func_wrap("env", "host_delete_node",
        move |mut caller: wasmtime::Caller<'_, WasiCtx>, ptr: i32, len: i32| {
            let mem = match caller.get_export("memory").and_then(|e| e.into_memory()) {
                Some(m) => m,
                None => return,
            };
            let data = mem.data(&caller);
            let start = ptr as usize;
            let end = start.saturating_add(len as usize);
            if end > data.len() { return; }
            let id_str = match std::str::from_utf8(&data[start..end]) {
                Ok(s) => s,
                Err(_) => return,
            };
            if let Ok(id) = Uuid::parse_str(id_str) { let _ = s3.delete(&id); }
        }
    ).map_err(|e| PluginError::internal(format!("link host_delete_node: {}", e)))?;

    let s4 = storage.clone();
    linker.func_wrap("env", "host_update_node",
        move |mut caller: wasmtime::Caller<'_, WasiCtx>, id_ptr: i32, id_len: i32, f_ptr: i32, f_len: i32| {
            let mem = match caller.get_export("memory").and_then(|e| e.into_memory()) {
                Some(m) => m,
                None => return,
            };
            let data = mem.data(&caller);
            let id_start = id_ptr as usize;
            let id_end = id_start.saturating_add(id_len as usize);
            if id_end > data.len() { return; }
            let id_str = match std::str::from_utf8(&data[id_start..id_end]) {
                Ok(s) => s,
                Err(_) => return,
            };
            let id = match Uuid::parse_str(id_str) { Ok(id) => id, Err(_) => return };
            let f_start = f_ptr as usize;
            let f_end = f_start.saturating_add(f_len as usize);
            if f_end > data.len() { return; }
            let json = match std::str::from_utf8(&data[f_start..f_end]) {
                Ok(s) => s,
                Err(_) => return,
            };
            if let Ok(fields) = serde_json::from_str::<HashMap<String, panorama_core::types::FieldValue>>(json) {
                let _ = s4.update(&id, fields);
            }
        }
    ).map_err(|e| PluginError::internal(format!("link host_update_node: {}", e)))?;

    let s5 = storage.clone();
    linker.func_wrap("env", "host_get_node",
        move |mut caller: wasmtime::Caller<'_, WasiCtx>, id_ptr: i32, id_len: i32, r_ptr: i32| -> i32 {
            let mem = match caller.get_export("memory").and_then(|e| e.into_memory()) {
                Some(m) => m,
                None => return 0,
            };
            let data = mem.data(&caller);
            let id_start = id_ptr as usize;
            let id_end = id_start.saturating_add(id_len as usize);
            if id_end > data.len() { return 0; }
            let id_str = match std::str::from_utf8(&data[id_start..id_end]) {
                Ok(s) => s,
                Err(_) => return 0,
            };
            let id = match Uuid::parse_str(id_str) { Ok(id) => id, Err(_) => return 0 };
            let node = match s5.get(&id) {
                Some(n) => n,
                None => return 0,
            };
            let json = serde_json::to_vec(&node).unwrap_or_default();
            let data_mut = mem.data_mut(&mut caller);
            let r_start = r_ptr as usize;
            if r_start >= data_mut.len() { return 0; }
            let write_len = json.len().min(data_mut.len() - r_start);
            data_mut[r_start..r_start + write_len].copy_from_slice(&json[..write_len]);
            write_len as i32
        }
    ).map_err(|e| PluginError::internal(format!("link host_get_node: {}", e)))?;

    // ── Instantiate & run ───────────────────────────────────────────────

    let instance = linker.instantiate_async(&mut store, &module).await
        .map_err(|e| PluginError::internal(format!("instantiate: {}", e)))?;

    let start = instance.get_typed_func::<(), ()>(&mut store, "_start")
        .map_err(|_| PluginError::internal("no _start export".into()))?;
    start.call_async(&mut store, ()).await
        .map_err(|e| PluginError::internal(format!("trap: {}", e)))?;

    // ── Read stdout (status + body JSON) ────────────────────────────────

    let output_bytes = stdout_pipe.contents();
    let output_str = String::from_utf8_lossy(&output_bytes);
    let out: serde_json::Value = serde_json::from_str(output_str.trim())
        .map_err(|e| PluginError::internal(format!("stdout: {} (raw: {})", e, &output_str[..output_str.len().min(200)])))?;

    let mut headers = HashMap::new();
    headers.insert("Content-Type".into(), "application/json".into());
    if let Some(h_obj) = out.get("headers").and_then(|h| h.as_object()) {
        for (k, v) in h_obj {
            if let Some(v_str) = v.as_str() {
                headers.insert(k.clone(), v_str.to_string());
            }
        }
    }

    let body_bytes = match out.get("body") {
        Some(serde_json::Value::String(s)) => Bytes::from(s.clone().into_bytes()),
        Some(val) => Bytes::from(serde_json::to_vec(val).unwrap_or_default()),
        None => Bytes::new(),
    };

    Ok(HttpResponse {
        status: out["status"].as_u64().unwrap_or(200) as u16,
        headers,
        body: body_bytes,
    })
}

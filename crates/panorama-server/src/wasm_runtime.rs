//! WASM runtime — wasmtime + WASI preview1 + PluginContext host functions.
//!
//! Host functions exposed to WASM plugins mirror the `PluginContext` trait.
//! They go through `RuntimeContext`, which enforces capability checks,
//! schema validation, and meta-table invariants — exactly the same path
//! native plugins take.  No backdoors.

use std::collections::HashMap;
use std::sync::Arc;

use bytes::Bytes;
use panorama_core::capabilities::CapabilityGrants;
use panorama_core::plugin::{HttpRequest, HttpResponse, LogLevel, PluginContext, PluginError};
use uuid::Uuid;

use crate::plugin_runtime::RuntimeContext;
use crate::storage::NodeStorage;
use crate::schema_registry::SchemaRegistry;
use crate::object_store::ObjectStorage;

type WasiCtx = wasmtime_wasi::preview1::WasiP1Ctx;

pub async fn execute_wasm_handler(
    engine: &wasmtime::Engine,
    module: &wasmtime::Module,
    endpoint: &str,
    request: &HttpRequest,
    plugin_id: &str,
    capabilities: &CapabilityGrants,
    storage: &NodeStorage,
    schema_registry: &SchemaRegistry,
    object_storage: &ObjectStorage,
) -> Result<HttpResponse, PluginError> {

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

    let mut store = wasmtime::Store::new(engine, wasi_ctx);
    let mut linker = wasmtime::Linker::new(engine);

    wasmtime_wasi::preview1::wasi_snapshot_preview1::add_to_linker(
        &mut linker,
        |cx: &mut WasiCtx| cx,
    ).map_err(|e| PluginError::internal(format!("wasi: {}", e)))?;

    // ── Build RuntimeContext (shared by all host functions) ──────────────

    let ctx = Arc::new(RuntimeContext::new(
        plugin_id,
        storage.clone(),
        schema_registry.clone(),
        object_storage.clone(),
        capabilities.clone(),
    ));

    // ── host_ctx_create_nodes ──────────────────────────────────────────

    let c1 = ctx.clone();
    linker.func_wrap("env", "host_ctx_create_nodes",
        move |mut caller: wasmtime::Caller<'_, WasiCtx>, f_ptr: i32, f_len: i32, r_ptr: i32| -> i32 {
            let mem = match caller.get_export("memory").and_then(|e| e.into_memory()) {
                Some(m) => m, None => return 0,
            };
            let data = mem.data(&caller);
            let start = f_ptr as usize;
            let end = start.saturating_add(f_len as usize);
            if end > data.len() { return 0; }

            let json = match std::str::from_utf8(&data[start..end]) {
                Ok(s) => s, Err(_) => return 0,
            };

            let nodes: Vec<panorama_core::types::Node> = if let Ok(ns) = serde_json::from_str(json) {
                ns
            } else if let Ok(fields) = serde_json::from_str::<HashMap<String, panorama_core::types::FieldValue>>(json) {
                let mut n = panorama_core::types::Node::new(Uuid::nil());
                for (k, v) in fields { n.set_field(&k, v); }
                vec![n]
            } else {
                return 0;
            };

            let result = pollster::block_on(c1.as_ref().create_nodes(nodes));
            let out_bytes = match result {
                Ok(ns) => serde_json::to_vec(&ns).unwrap_or_default(),
                Err(e) => serde_json::to_vec(&serde_json::json!({"error": e.message})).unwrap_or_default(),
            };

            let data_mut = mem.data_mut(&mut caller);
            let r_start = r_ptr as usize;
            if r_start >= data_mut.len() { return 0; }
            let wl = out_bytes.len().min(data_mut.len() - r_start);
            data_mut[r_start..r_start + wl].copy_from_slice(&out_bytes[..wl]);
            wl as i32
        }
    ).map_err(|e| PluginError::internal(format!("link host_ctx_create_nodes: {}", e)))?;

    // ── host_ctx_create_node ───────────────────────────────────────────

    let c1_single = ctx.clone();
    linker.func_wrap("env", "host_ctx_create_node",
        move |mut caller: wasmtime::Caller<'_, WasiCtx>, f_ptr: i32, f_len: i32, r_ptr: i32| -> i32 {
            let mem = match caller.get_export("memory").and_then(|e| e.into_memory()) {
                Some(m) => m, None => return 0,
            };
            let data = mem.data(&caller);
            let start = f_ptr as usize;
            let end = start.saturating_add(f_len as usize);
            if end > data.len() { return 0; }

            let json = match std::str::from_utf8(&data[start..end]) {
                Ok(s) => s, Err(_) => return 0,
            };
            let fields: HashMap<String, panorama_core::types::FieldValue> = match serde_json::from_str(json) {
                Ok(f) => f, Err(_) => return 0,
            };

            let mut node = panorama_core::types::Node::new(Uuid::nil());
            for (k, v) in fields { node.set_field(&k, v); }

            let result = pollster::block_on(c1_single.as_ref().create_nodes(vec![node]));
            let out_bytes = match result {
                Ok(mut ns) if !ns.is_empty() => serde_json::to_vec(&ns.remove(0)).unwrap_or_default(),
                Ok(_) => serde_json::to_vec(&serde_json::json!({"error": "No node created"})).unwrap_or_default(),
                Err(e) => serde_json::to_vec(&serde_json::json!({"error": e.message})).unwrap_or_default(),
            };

            let data_mut = mem.data_mut(&mut caller);
            let r_start = r_ptr as usize;
            if r_start >= data_mut.len() { return 0; }
            let wl = out_bytes.len().min(data_mut.len() - r_start);
            data_mut[r_start..r_start + wl].copy_from_slice(&out_bytes[..wl]);
            wl as i32
        }
    ).map_err(|e| PluginError::internal(format!("link host_ctx_create_node: {}", e)))?;

    // ── host_ctx_get_node ──────────────────────────────────────────────

    let c2 = ctx.clone();
    linker.func_wrap("env", "host_ctx_get_node",
        move |mut caller: wasmtime::Caller<'_, WasiCtx>, id_ptr: i32, id_len: i32, r_ptr: i32| -> i32 {
            let mem = match caller.get_export("memory").and_then(|e| e.into_memory()) {
                Some(m) => m, None => return 0,
            };
            let data = mem.data(&caller);
            let start = id_ptr as usize;
            let end = start.saturating_add(id_len as usize);
            if end > data.len() { return 0; }

            let id_str = match std::str::from_utf8(&data[start..end]) {
                Ok(s) => s, Err(_) => return 0,
            };
            let id = match Uuid::parse_str(id_str) {
                Ok(id) => id, Err(_) => return 0,
            };

            let result = pollster::block_on(c2.as_ref().get_node(id));
            let out_bytes = match result {
                Ok(Some(n)) => serde_json::to_vec(&n).unwrap_or_default(),
                Ok(None) => serde_json::to_vec(&serde_json::Value::Null).unwrap_or_default(),
                Err(e) => serde_json::to_vec(&serde_json::json!({"error": e.message})).unwrap_or_default(),
            };

            let data_mut = mem.data_mut(&mut caller);
            let r_start = r_ptr as usize;
            if r_start >= data_mut.len() { return 0; }
            let wl = out_bytes.len().min(data_mut.len() - r_start);
            data_mut[r_start..r_start + wl].copy_from_slice(&out_bytes[..wl]);
            wl as i32
        }
    ).map_err(|e| PluginError::internal(format!("link host_ctx_get_node: {}", e)))?;

    // ── host_ctx_update_node ───────────────────────────────────────────

    let c3 = ctx.clone();
    linker.func_wrap("env", "host_ctx_update_node",
        move |mut caller: wasmtime::Caller<'_, WasiCtx>, id_ptr: i32, id_len: i32, f_ptr: i32, f_len: i32, r_ptr: i32| -> i32 {
            let mem = match caller.get_export("memory").and_then(|e| e.into_memory()) {
                Some(m) => m, None => return 0,
            };
            let data = mem.data(&caller);
            // Read id
            let id_start = id_ptr as usize;
            let id_end = id_start.saturating_add(id_len as usize);
            if id_end > data.len() { return 0; }
            let id_str = match std::str::from_utf8(&data[id_start..id_end]) {
                Ok(s) => s, Err(_) => return 0,
            };
            let id = match Uuid::parse_str(id_str) { Ok(id) => id, Err(_) => return 0 };
            // Read fields JSON
            let f_start = f_ptr as usize;
            let f_end = f_start.saturating_add(f_len as usize);
            if f_end > data.len() { return 0; }
            let f_json = match std::str::from_utf8(&data[f_start..f_end]) {
                Ok(s) => s, Err(_) => return 0,
            };
            let fields: HashMap<String, panorama_core::types::FieldValue> = match serde_json::from_str(f_json) {
                Ok(f) => f, Err(_) => return 0,
            };

            let result = pollster::block_on(c3.as_ref().update_node(id, fields));
            let out_bytes = match result {
                Ok(n) => serde_json::to_vec(&n).unwrap_or_default(),
                Err(e) => serde_json::to_vec(&serde_json::json!({"error": e.message})).unwrap_or_default(),
            };

            let data_mut = mem.data_mut(&mut caller);
            let r_start = r_ptr as usize;
            if r_start >= data_mut.len() { return 0; }
            let wl = out_bytes.len().min(data_mut.len() - r_start);
            data_mut[r_start..r_start + wl].copy_from_slice(&out_bytes[..wl]);
            wl as i32
        }
    ).map_err(|e| PluginError::internal(format!("link host_ctx_update_node: {}", e)))?;

    // ── host_ctx_delete_node ───────────────────────────────────────────

    let c4 = ctx.clone();
    linker.func_wrap("env", "host_ctx_delete_node",
        move |mut caller: wasmtime::Caller<'_, WasiCtx>, id_ptr: i32, id_len: i32| {
            let mem = match caller.get_export("memory").and_then(|e| e.into_memory()) {
                Some(m) => m, None => return,
            };
            let data = mem.data(&caller);
            let start = id_ptr as usize;
            let end = start.saturating_add(id_len as usize);
            if end > data.len() { return; }
            let id_str = match std::str::from_utf8(&data[start..end]) {
                Ok(s) => s, Err(_) => return,
            };
            if let Ok(id) = Uuid::parse_str(id_str) {
                let _ = pollster::block_on(c4.as_ref().delete_node(id));
            }
        }
    ).map_err(|e| PluginError::internal(format!("link host_ctx_delete_node: {}", e)))?;

    // ── host_ctx_query ─────────────────────────────────────────────────

    let c5 = ctx.clone();
    linker.func_wrap("env", "host_ctx_query",
        move |mut caller: wasmtime::Caller<'_, WasiCtx>, q_ptr: i32, q_len: i32, r_ptr: i32| -> i32 {
            let mem = match caller.get_export("memory").and_then(|e| e.into_memory()) {
                Some(m) => m, None => return 0,
            };
            let data = mem.data(&caller);
            let start = q_ptr as usize;
            let end = start.saturating_add(q_len as usize);
            if end > data.len() { return 0; }
            let qs = match std::str::from_utf8(&data[start..end]) {
                Ok(s) => s, Err(_) => return 0,
            };

            let result = pollster::block_on(c5.as_ref().query(qs));
            let rows = match result {
                Ok(r) => r,
                Err(_) => vec![],
            };
            let json = serde_json::to_vec(&rows).unwrap_or_default();

            let data_mut = mem.data_mut(&mut caller);
            let r_start = r_ptr as usize;
            if r_start >= data_mut.len() { return 0; }
            let wl = json.len().min(data_mut.len() - r_start);
            data_mut[r_start..r_start + wl].copy_from_slice(&json[..wl]);
            wl as i32
        }
    ).map_err(|e| PluginError::internal(format!("link host_ctx_query: {}", e)))?;

    // ── host_ctx_log ───────────────────────────────────────────────────

    let c6 = ctx.clone();
    linker.func_wrap("env", "host_ctx_log",
        move |mut caller: wasmtime::Caller<'_, WasiCtx>, msg_ptr: i32, msg_len: i32| {
            let mem = match caller.get_export("memory").and_then(|e| e.into_memory()) {
                Some(m) => m, None => return,
            };
            let data = mem.data(&caller);
            let start = msg_ptr as usize;
            let end = start.saturating_add(msg_len as usize);
            if end > data.len() { return; }
            if let Ok(msg) = std::str::from_utf8(&data[start..end]) {
                let _ = pollster::block_on(c6.as_ref().log(LogLevel::Info, msg));
            }
        }
    ).map_err(|e| PluginError::internal(format!("link host_ctx_log: {}", e)))?;

    // ── Instantiate & run ───────────────────────────────────────────────

    let instance = linker.instantiate_async(&mut store, module).await
        .map_err(|e| PluginError::internal(format!("instantiate: {}", e)))?;

    let start = instance.get_typed_func::<(), ()>(&mut store, "_start")
        .map_err(|_| PluginError::internal("no _start export".into()))?;
    start.call_async(&mut store, ()).await
        .map_err(|e| PluginError::internal(format!("trap: {}", e)))?;

    // ── Read stdout ────────────────────────────────────────────────────

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

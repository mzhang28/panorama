//! WASM runtime — wasmtime + WASI preview1 + PluginContext host functions.
//!
//! Host functions exposed to WASM plugins mirror the `PluginContext` trait.
//! They go through `RuntimeContext`, which enforces capability checks,
//! schema validation, and meta-table invariants — exactly the same path
//! native plugins take.  No backdoors.
//!
//! ## Linker caching (§1.1)
//!
//! `create_prelinked_instance` builds a `wasmtime::Linker`, registers all 6
//! host functions, and returns an `InstancePre`.  The caller caches this
//! per-plugin so `execute_wasm_handler` skips linking entirely — only the
//! per-request WASI stdin/stdout store is built fresh.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use bytes::Bytes;
use panorama_core::capabilities::CapabilityGrants;
use panorama_core::plugin::{HttpRequest, HttpResponse, LogLevel, PluginContext, PluginError};
use tracing::Span;
use uuid::Uuid;
use wasmtime_wasi::{HostOutputStream, StreamError, Subscribe};

use crate::backtrace::{BacktraceStore, CapturedContext, SpanSnapshot};
use crate::object_store::ObjectStorage;
use crate::plugin_runtime::RuntimeContext;
use crate::schema_registry::SchemaRegistry;
use crate::storage::NodeStorage;

pub(crate) type WasiCtx = wasmtime_wasi::preview1::WasiP1Ctx;

// ── Unbounded output pipe ──────────────────────────────────────────────────

/// A `HostOutputStream` with no fixed capacity — grows a `Vec<u8>` without limit.
#[derive(Debug, Clone)]
struct UnboundedOutputPipe {
  buffer: Arc<Mutex<Vec<u8>>>,
}

impl UnboundedOutputPipe {
  fn new() -> Self {
    Self {
      buffer: Arc::new(Mutex::new(Vec::new())),
    }
  }

  fn contents(&self) -> Vec<u8> {
    self.buffer.lock().unwrap().clone()
  }
}

#[async_trait::async_trait]
impl HostOutputStream for UnboundedOutputPipe {
  fn write(&mut self, bytes: Bytes) -> Result<(), StreamError> {
    self.buffer.lock().unwrap().extend_from_slice(&bytes);
    Ok(())
  }
  fn flush(&mut self) -> Result<(), StreamError> {
    Ok(())
  }
  fn check_write(&mut self) -> Result<usize, StreamError> {
    Ok(usize::MAX)
  }
}

#[async_trait::async_trait]
impl Subscribe for UnboundedOutputPipe {
  async fn ready(&mut self) {}
}

impl wasmtime_wasi::StdoutStream for UnboundedOutputPipe {
  fn stream(&self) -> Box<dyn HostOutputStream> {
    Box::new(self.clone())
  }

  fn isatty(&self) -> bool {
    false
  }
}

/// Build a pre-linked `InstancePre` for a WASM plugin.
///
/// Creates the linker, registers WASI + all 6 host functions, and returns
/// an `InstancePre` that can be instantiated nearly instantly per-request.
/// The returned `InstancePre` is safe to clone and share across threads.
///
/// This should be called **once** at plugin load time, not per-request.
pub fn create_prelinked_instance(
  engine: &wasmtime::Engine,
  module: &wasmtime::Module,
  plugin_id: &str,
  capabilities: &CapabilityGrants,
  storage: &NodeStorage,
  schema_registry: &SchemaRegistry,
  object_storage: &ObjectStorage,
  backtrace_store: Arc<BacktraceStore>,
) -> Result<wasmtime::InstancePre<WasiCtx>, PluginError> {
  let mut linker = wasmtime::Linker::new(engine);

  // ── WASI ──────────────────────────────────────────────────────────────
  wasmtime_wasi::preview1::wasi_snapshot_preview1::add_to_linker(
    &mut linker,
    |cx: &mut WasiCtx| cx,
  )
  .map_err(|e| PluginError::internal(format!("wasi: {}", e)))?;

  // ── RuntimeContext (shared by all host functions) ─────────────────────
  let mut ctx = RuntimeContext::new(
    plugin_id,
    storage.clone(),
    schema_registry.clone(),
    object_storage.clone(),
    capabilities.clone(),
  );
  ctx.backtrace_store = backtrace_store;
  let ctx = Arc::new(ctx);

  // ── host_ctx_create_nodes ──────────────────────────────────────────
  let c1 = ctx.clone();
  linker
    .func_wrap(
      "env",
      "host_ctx_create_nodes",
      move |mut caller: wasmtime::Caller<'_, WasiCtx>, f_ptr: i32, f_len: i32, r_ptr: i32| -> i32 {
        let mem = match caller.get_export("memory").and_then(|e| e.into_memory()) {
          Some(m) => m,
          None => return 0,
        };
        let data = mem.data(&caller);
        let start = f_ptr as usize;
        let end = start.saturating_add(f_len as usize);
        if end > data.len() {
          return 0;
        }

        let json = match std::str::from_utf8(&data[start..end]) {
          Ok(s) => s,
          Err(e) => {
            error!("[host_ctx_create_nodes] UTF-8 decode failed: {}", e);
            return 0;
          }
        };

        let nodes: Vec<panorama_core::types::Node> = if let Ok(ns) = serde_json::from_str(json) {
          ns
        } else if let Ok(fields) =
          serde_json::from_str::<HashMap<String, panorama_core::types::FieldValue>>(json)
        {
          let mut n = panorama_core::types::Node::new(Uuid::nil());
          for (k, v) in fields {
            n.set_field(&k, v);
          }
          vec![n]
        } else {
          error!(
            "[host_ctx_create_nodes] JSON parse failed: {}",
            &json[..json.len().min(500)]
          );
          return 0;
        };

        let result = pollster::block_on(c1.as_ref().create_nodes(nodes));
        let out_bytes = match result {
          Ok(ns) => serde_json::to_vec(&ns).unwrap_or_default(),
          Err(e) => serde_json::to_vec(&serde_json::json!({"error": &e})).unwrap_or_default(),
        };

        let data_mut = mem.data_mut(&mut caller);
        let r_start = r_ptr as usize;
        if r_start >= data_mut.len() {
          return 0;
        }
        let wl = out_bytes.len().min(data_mut.len() - r_start);
        data_mut[r_start..r_start + wl].copy_from_slice(&out_bytes[..wl]);
        wl as i32
      },
    )
    .map_err(|e| PluginError::internal(format!("link host_ctx_create_nodes: {}", e)))?;

  // ── host_ctx_create_node ───────────────────────────────────────────
  let c1_single = ctx.clone();
  linker
    .func_wrap(
      "env",
      "host_ctx_create_node",
      move |mut caller: wasmtime::Caller<'_, WasiCtx>, f_ptr: i32, f_len: i32, r_ptr: i32| -> i32 {
        let mem = match caller.get_export("memory").and_then(|e| e.into_memory()) {
          Some(m) => m,
          None => return 0,
        };
        let data = mem.data(&caller);
        let start = f_ptr as usize;
        let end = start.saturating_add(f_len as usize);
        if end > data.len() {
          return 0;
        }

        let json = match std::str::from_utf8(&data[start..end]) {
          Ok(s) => s,
          Err(e) => {
            error!("[host_ctx_create_node] UTF-8 decode failed: {}", e);
            return 0;
          }
        };
        let fields: HashMap<String, panorama_core::types::FieldValue> =
          match serde_json::from_str(json) {
            Ok(f) => f,
            Err(e) => {
              error!(
                "[host_ctx_create_node] JSON parse failed: {} (input: {})",
                e,
                &json[..json.len().min(500)]
              );
              return 0;
            }
          };

        let mut node = panorama_core::types::Node::new(Uuid::nil());
        for (k, v) in fields {
          node.set_field(&k, v);
        }

        let result = pollster::block_on(c1_single.as_ref().create_nodes(vec![node]));
        let out_bytes = match result {
          Ok(mut ns) if !ns.is_empty() => serde_json::to_vec(&ns.remove(0)).unwrap_or_default(),
          Ok(_) => {
            serde_json::to_vec(&serde_json::json!({"error": "No node created"})).unwrap_or_default()
          }
          Err(e) => serde_json::to_vec(&serde_json::json!({"error": &e})).unwrap_or_default(),
        };

        let data_mut = mem.data_mut(&mut caller);
        let r_start = r_ptr as usize;
        if r_start >= data_mut.len() {
          return 0;
        }
        let wl = out_bytes.len().min(data_mut.len() - r_start);
        data_mut[r_start..r_start + wl].copy_from_slice(&out_bytes[..wl]);
        wl as i32
      },
    )
    .map_err(|e| PluginError::internal(format!("link host_ctx_create_node: {}", e)))?;

  // ── host_ctx_get_node ──────────────────────────────────────────────
  let c2 = ctx.clone();
  linker
    .func_wrap(
      "env",
      "host_ctx_get_node",
      move |mut caller: wasmtime::Caller<'_, WasiCtx>,
            id_ptr: i32,
            id_len: i32,
            r_ptr: i32|
            -> i32 {
        let mem = match caller.get_export("memory").and_then(|e| e.into_memory()) {
          Some(m) => m,
          None => return 0,
        };
        let data = mem.data(&caller);
        let start = id_ptr as usize;
        let end = start.saturating_add(id_len as usize);
        if end > data.len() {
          return 0;
        }

        let id_str = match std::str::from_utf8(&data[start..end]) {
          Ok(s) => s,
          Err(e) => {
            error!("[host_ctx_get_node] UTF-8 decode failed: {}", e);
            return 0;
          }
        };
        let id = match Uuid::parse_str(id_str) {
          Ok(id) => id,
          Err(e) => {
            error!(
              "[host_ctx_get_node] UUID parse failed: {} (input: {})",
              e, id_str
            );
            return 0;
          }
        };

        let result = pollster::block_on(c2.as_ref().get_node(id));
        let out_bytes = match result {
          Ok(Some(n)) => serde_json::to_vec(&n).unwrap_or_default(),
          Ok(None) => serde_json::to_vec(&serde_json::Value::Null).unwrap_or_default(),
          Err(e) => serde_json::to_vec(&serde_json::json!({"error": &e})).unwrap_or_default(),
        };

        let data_mut = mem.data_mut(&mut caller);
        let r_start = r_ptr as usize;
        if r_start >= data_mut.len() {
          return 0;
        }
        let wl = out_bytes.len().min(data_mut.len() - r_start);
        data_mut[r_start..r_start + wl].copy_from_slice(&out_bytes[..wl]);
        wl as i32
      },
    )
    .map_err(|e| PluginError::internal(format!("link host_ctx_get_node: {}", e)))?;

  // ── host_ctx_update_node ───────────────────────────────────────────
  let c3 = ctx.clone();
  linker
    .func_wrap(
      "env",
      "host_ctx_update_node",
      move |mut caller: wasmtime::Caller<'_, WasiCtx>,
            id_ptr: i32,
            id_len: i32,
            f_ptr: i32,
            f_len: i32,
            r_ptr: i32|
            -> i32 {
        let mem = match caller.get_export("memory").and_then(|e| e.into_memory()) {
          Some(m) => m,
          None => return 0,
        };
        let data = mem.data(&caller);
        let id_start = id_ptr as usize;
        let id_end = id_start.saturating_add(id_len as usize);
        if id_end > data.len() {
          return 0;
        }
        let id_str = match std::str::from_utf8(&data[id_start..id_end]) {
          Ok(s) => s,
          Err(e) => {
            error!("[host_ctx_update_node] UTF-8 decode failed for id: {}", e);
            return 0;
          }
        };
        let id = match Uuid::parse_str(id_str) {
          Ok(id) => id,
          Err(e) => {
            error!(
              "[host_ctx_update_node] UUID parse failed: {} (input: {})",
              e, id_str
            );
            return 0;
          }
        };
        let f_start = f_ptr as usize;
        let f_end = f_start.saturating_add(f_len as usize);
        if f_end > data.len() {
          return 0;
        }
        let f_json = match std::str::from_utf8(&data[f_start..f_end]) {
          Ok(s) => s,
          Err(e) => {
            error!(
              "[host_ctx_update_node] UTF-8 decode failed for fields: {}",
              e
            );
            return 0;
          }
        };
        let fields: HashMap<String, panorama_core::types::FieldValue> =
          match serde_json::from_str(f_json) {
            Ok(f) => f,
            Err(e) => {
              error!(
                "[host_ctx_update_node] JSON parse failed: {} (input: {})",
                e,
                &f_json[..f_json.len().min(500)]
              );
              return 0;
            }
          };

        let result = pollster::block_on(c3.as_ref().update_node(id, fields));
        let out_bytes = match result {
          Ok(n) => serde_json::to_vec(&n).unwrap_or_default(),
          Err(e) => serde_json::to_vec(&serde_json::json!({"error": &e})).unwrap_or_default(),
        };

        let data_mut = mem.data_mut(&mut caller);
        let r_start = r_ptr as usize;
        if r_start >= data_mut.len() {
          return 0;
        }
        let wl = out_bytes.len().min(data_mut.len() - r_start);
        data_mut[r_start..r_start + wl].copy_from_slice(&out_bytes[..wl]);
        wl as i32
      },
    )
    .map_err(|e| PluginError::internal(format!("link host_ctx_update_node: {}", e)))?;

  // ── host_ctx_delete_node ───────────────────────────────────────────
  let c4 = ctx.clone();
  linker
    .func_wrap(
      "env",
      "host_ctx_delete_node",
      move |mut caller: wasmtime::Caller<'_, WasiCtx>, id_ptr: i32, id_len: i32| {
        let mem = match caller.get_export("memory").and_then(|e| e.into_memory()) {
          Some(m) => m,
          None => return,
        };
        let data = mem.data(&caller);
        let start = id_ptr as usize;
        let end = start.saturating_add(id_len as usize);
        if end > data.len() {
          return;
        }
        let id_str = match std::str::from_utf8(&data[start..end]) {
          Ok(s) => s,
          Err(e) => {
            error!("[host_ctx_delete_node] UTF-8 decode failed: {}", e);
            return;
          }
        };
        if let Ok(id) = Uuid::parse_str(id_str) {
          let _ = pollster::block_on(c4.as_ref().delete_node(id));
        } else {
          error!("[host_ctx_delete_node] UUID parse failed: {}", id_str);
        }
      },
    )
    .map_err(|e| PluginError::internal(format!("link host_ctx_delete_node: {}", e)))?;

  // ── host_ctx_query ─────────────────────────────────────────────────
  let c5 = ctx.clone();
  linker
    .func_wrap(
      "env",
      "host_ctx_query",
      move |mut caller: wasmtime::Caller<'_, WasiCtx>, q_ptr: i32, q_len: i32, r_ptr: i32| -> i32 {
        let mem = match caller.get_export("memory").and_then(|e| e.into_memory()) {
          Some(m) => m,
          None => return 0,
        };
        let qs = {
          let data = mem.data(&caller);
          let start = q_ptr as usize;
          let end = start.saturating_add(q_len as usize);
          if end > data.len() {
            return 0;
          }
          match std::str::from_utf8(&data[start..end]) {
            Ok(s) => s.to_string(),
            Err(e) => {
              error!("[host_ctx_query] UTF-8 decode failed: {}", e);
              return 0;
            }
          }
        }; // data borrow released here

        let result = pollster::block_on(c5.as_ref().query(&qs));
        let rows = match result {
          Ok(r) => r,
          Err(ref e) => {
            // Capture the wasm backtrace before logging so the Sentry
            // event shows which guest code called this host function.
            capture_and_attach_host_error_backtrace(&mut caller, c5.as_ref().plugin_id());
            error!(
              "[host_ctx_query] ERROR: {} | query={}",
              e.message,
              &qs[..qs.len().min(200)]
            );
            vec![]
          }
        };
        let json = serde_json::to_vec(&rows).unwrap_or_default();

        let data_mut = mem.data_mut(&mut caller);
        let r_start = r_ptr as usize;
        if r_start >= data_mut.len() {
          return 0;
        }
        let wl = json.len().min(data_mut.len() - r_start);
        data_mut[r_start..r_start + wl].copy_from_slice(&json[..wl]);
        wl as i32
      },
    )
    .map_err(|e| PluginError::internal(format!("link host_ctx_query: {}", e)))?;

  // ── host_ctx_log ───────────────────────────────────────────────────
  let c6 = ctx.clone();
  linker
    .func_wrap(
      "env",
      "host_ctx_log",
      move |mut caller: wasmtime::Caller<'_, WasiCtx>, msg_ptr: i32, msg_len: i32| {
        let mem = match caller.get_export("memory").and_then(|e| e.into_memory()) {
          Some(m) => m,
          None => return,
        };
        let data = mem.data(&caller);
        let start = msg_ptr as usize;
        let end = start.saturating_add(msg_len as usize);
        if end > data.len() {
          return;
        }
        if let Ok(msg) = std::str::from_utf8(&data[start..end]) {
          let _ = pollster::block_on(c6.as_ref().log(LogLevel::Info, msg));
        }
      },
    )
    .map_err(|e| PluginError::internal(format!("link host_ctx_log: {}", e)))?;

  // ── host_report_panic ──────────────────────────────────────────────
  // Fire-and-forget: the panic hook in wasm_adapter calls this before the
  // wasm module aborts.  The message is stored so the trap error path
  // can attach it to the error.
  let panic_store = ctx.backtrace_store.clone();
  linker
    .func_wrap(
      "env",
      "host_report_panic",
      move |mut caller: wasmtime::Caller<'_, WasiCtx>, msg_ptr: i32, msg_len: i32| {
        let mem = match caller.get_export("memory").and_then(|e| e.into_memory()) {
          Some(m) => m,
          None => return,
        };
        let data = mem.data(&caller);
        let start = msg_ptr as usize;
        let end = start.saturating_add(msg_len as usize);
        if end > data.len() {
          return;
        }
        if let Ok(msg) = std::str::from_utf8(&data[start..end]) {
          if let Ok(mut p) = panic_store.last_panic.lock() {
            *p = Some(msg.to_string());
          }
        }
      },
    )
    .map_err(|e| PluginError::internal(format!("link host_report_panic: {}", e)))?;

  // ── host_capture_backtrace ──────────────────────────────────────────
  let bt_store = ctx.backtrace_store.clone();
  let bt_plugin_id = plugin_id.to_string();
  linker
    .func_wrap(
      "env",
      "host_capture_backtrace",
      move |mut caller: wasmtime::Caller<'_, WasiCtx>| -> u64 {
        // Capture the wasm backtrace through the JIT frame-pointer chain.
        // wasmtime::WasmBacktrace::force_capture walks the native stack
        // (Cranelift-compiled wasm → real call instructions, real frame
        // pointers) and returns every wasm frame currently live.
        let bt = wasmtime::WasmBacktrace::force_capture(&mut caller);
        let frames: Vec<crate::backtrace::StoredFrame> = bt
          .frames()
          .iter()
          .map(|f| crate::backtrace::StoredFrame::from_frame_info(f))
          .collect();

        // Capture the current tracing span (innermost only — tracing 0.1
        // has no Span::parent() to walk the chain).
        let spans = match Span::current().id() {
          Some(_) => {
            let name = Span::current()
              .metadata()
              .map(|m| m.name().to_string())
              .unwrap_or_default();
            vec![SpanSnapshot {
              name,
              fields: HashMap::new(),
            }]
          }
          None => Vec::new(),
        };

        let ctx = CapturedContext {
          frames,
          spans,
          plugin_id: bt_plugin_id.clone(),
          request_id: None, // filled in by caller if available
        };

        bt_store.insert(ctx)
      },
    )
    .map_err(|e| PluginError::internal(format!("link host_capture_backtrace: {}", e)))?;

  // ── Pre-link ───────────────────────────────────────────────────────
  linker
    .instantiate_pre(module)
    .map_err(|e| PluginError::internal(format!("pre-link: {}", e)))
}

/// Execute a WASM handler using a pre-built `InstancePre`.
///
/// Only the per-request WASI context (stdin/stdout pipes) is created fresh;
/// all host function linking was done at plugin load time via
/// `create_prelinked_instance` (§1.1).
pub async fn execute_wasm_handler(
  engine: &wasmtime::Engine,
  instance_pre: &wasmtime::InstancePre<WasiCtx>,
  endpoint: &str,
  request: &HttpRequest,
  backtrace_store: &BacktraceStore,
) -> Result<HttpResponse, PluginError> {
  let input_json = serde_json::to_vec(&serde_json::json!({
      "endpoint": endpoint,
      "request": {
          "method": &request.method, "path": &request.path,
          "query_params": &request.query_params, "headers": &request.headers,
          "body": request.body.as_ref().map(|b| String::from_utf8_lossy(b).to_string()),
      },
  }))
  .map_err(|e| PluginError::internal(format!("json: {}", e)))?;

  let stdout_pipe = UnboundedOutputPipe::new();
  let stdin_pipe = wasmtime_wasi::pipe::MemoryInputPipe::new(Bytes::from(input_json));

  let mut builder = wasmtime_wasi::WasiCtxBuilder::new();
  builder.stdin(stdin_pipe);
  builder.stdout(stdout_pipe.clone());
  let wasi_ctx = builder.build_p1();

  let mut store = wasmtime::Store::new(engine, wasi_ctx);

  let instance = instance_pre
    .instantiate_async(&mut store)
    .await
    .map_err(|e| PluginError::internal(format!("instantiate: {}", e)))?;

  let start = instance
    .get_typed_func::<(), ()>(&mut store, "_start")
    .map_err(|_| PluginError::internal("no _start export".into()))?;

  // Execute the wasm module.  If it traps, wasmtime attaches a
  // WasmBacktrace to the error automatically — extract it and store
  // the frames directly in the PluginError (trap path, no store needed).
  if let Err(e) = start.call_async(&mut store, ()).await {
    // Extract frames from the wasmtime error, including DWARF file:line
    // symbols when available (requires wasmtime's addr2line feature).
    let frames: Vec<panorama_core::plugin::TrapFrame> = e
      .downcast_ref::<wasmtime::WasmBacktrace>()
      .map(|bt| {
        bt.frames()
          .iter()
          .map(|f| {
            let sf = crate::backtrace::StoredFrame::from_frame_info(f);
            panorama_core::plugin::TrapFrame::from(&sf)
          })
          .collect()
      })
      .unwrap_or_default();

    let trap_msg = if let Some(trap) = e.downcast_ref::<wasmtime::Trap>() {
      format!("{}", trap)
    } else {
      format!("{}", e)
    };

    // If the panic hook stored a message, prepend it to the trap message.
    let msg = if let Ok(mut p) = backtrace_store.last_panic.lock() {
      if let Some(panic_msg) = p.take() {
        format!("panic: {} | trap: {}", panic_msg, trap_msg)
      } else {
        format!("trap: {}", trap_msg)
      }
    } else {
      format!("trap: {}", trap_msg)
    };

    return Err(PluginError {
      trap_frames: if frames.is_empty() {
        None
      } else {
        Some(frames)
      },
      ..PluginError::internal(msg)
    });
  }

  let output_bytes = stdout_pipe.contents();
  let body_bytes = decode_length_prefixed(&output_bytes)
    .map_err(|e| PluginError::internal(format!("stdout: {}", e)))?;
  let output_str = String::from_utf8_lossy(body_bytes);
  let out: serde_json::Value = serde_json::from_str(output_str.trim()).map_err(|e| {
    PluginError::internal(format!(
      "stdout: {} (raw: {})",
      e,
      &output_str[..output_str.len().min(200)]
    ))
  })?;

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

// ── Host function error backtrace helper ────────────────────────────────────

/// Capture a wasm backtrace from within a host function and attach it to
/// the Sentry scope.  Call this before `error!()` in host functions that
/// handle errors from the guest — it shows which guest code called the
/// host function that failed.
fn capture_and_attach_host_error_backtrace(
  caller: &mut wasmtime::Caller<'_, WasiCtx>,
  plugin_id: &str,
) {
  let bt = wasmtime::WasmBacktrace::force_capture(caller);
  let frames: Vec<panorama_core::plugin::TrapFrame> = bt
    .frames()
    .iter()
    .map(|f| {
      let sf = crate::backtrace::StoredFrame::from_frame_info(f);
      panorama_core::plugin::TrapFrame::from(&sf)
    })
    .collect();
  if !frames.is_empty() {
    crate::sentry_middleware::attach_wasm_backtrace_to_scope(&frames, plugin_id);
  }
}

// ── Length-prefixed protocol ───────────────────────────────────────────────

/// Decode a length-prefixed payload: first 4 bytes = little-endian u32
/// length of the body that follows.  Returns a slice of the body bytes.
fn decode_length_prefixed(buf: &[u8]) -> Result<&[u8], String> {
  if buf.len() < 4 {
    return Err(format!(
      "too short: {} bytes (need at least 4 for length prefix)",
      buf.len()
    ));
  }
  let body_len = u32::from_le_bytes(buf[..4].try_into().unwrap()) as usize;
  if buf.len() < 4 + body_len {
    return Err(format!(
      "truncated: expected {} bytes, got {}",
      4 + body_len,
      buf.len()
    ));
  }
  Ok(&buf[4..4 + body_len])
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
  use super::*;
  use wasmtime_wasi::{HostOutputStream, StdoutStream};

  // ── UnboundedOutputPipe ───────────────────────────────────────────────

  #[test]
  fn unbounded_pipe_small_write() {
    let pipe = UnboundedOutputPipe::new();
    let mut stream = pipe.stream();
    stream
      .write(Bytes::from_static(b"hello"))
      .expect("write should succeed");
    let contents = pipe.contents();
    assert_eq!(contents, b"hello");
  }

  #[test]
  fn unbounded_pipe_exceeds_64kb() {
    let pipe = UnboundedOutputPipe::new();
    let mut stream = pipe.stream();
    // Write 128 KB — double the old MemoryOutputPipe cap of 64 KB
    let chunk = vec![0xABu8; 128 * 1024];
    stream
      .write(Bytes::from(chunk.clone()))
      .expect("128KB write should succeed");

    let contents = pipe.contents();
    assert_eq!(contents.len(), 128 * 1024);
    assert_eq!(contents, chunk);
  }

  #[test]
  fn unbounded_pipe_multiple_writes() {
    let pipe = UnboundedOutputPipe::new();
    let mut stream = pipe.stream();
    // 10 writes of 20 KB each = 200 KB total
    for i in 0u8..10 {
      let chunk = vec![i; 20 * 1024];
      stream
        .write(Bytes::from(chunk))
        .expect("write should succeed");
    }
    let contents = pipe.contents();
    assert_eq!(contents.len(), 200 * 1024);
    // Verify each chunk landed in order
    for i in 0u8..10 {
      let start = i as usize * 20 * 1024;
      assert!(contents[start..start + 20 * 1024].iter().all(|&b| b == i));
    }
  }

  #[test]
  fn unbounded_pipe_empty() {
    let pipe = UnboundedOutputPipe::new();
    assert!(pipe.contents().is_empty());
  }

  // ── Length-prefix decoding ────────────────────────────────────────────

  #[test]
  fn decode_empty_body() {
    // length = 0
    let buf = [0u8, 0, 0, 0];
    let body = decode_length_prefixed(&buf).expect("should decode");
    assert!(body.is_empty());
  }

  #[test]
  fn decode_small_body() {
    let body_data = b"hello world";
    let len = body_data.len() as u32;
    let mut buf = Vec::new();
    buf.extend_from_slice(&len.to_le_bytes());
    buf.extend_from_slice(body_data);

    let decoded = decode_length_prefixed(&buf).expect("should decode");
    assert_eq!(decoded, body_data);
  }

  #[test]
  fn decode_too_short() {
    let buf = [1u8, 0, 0]; // only 3 bytes
    let err = decode_length_prefixed(&buf).unwrap_err();
    assert!(err.contains("too short"));
  }

  #[test]
  fn decode_truncated() {
    // Claim 100 bytes but only provide 4 + 10 bytes
    let mut buf = vec![0u8; 4 + 10];
    buf[0] = 100; // length = 100 (LE)
    let err = decode_length_prefixed(&buf).unwrap_err();
    assert!(err.contains("truncated"));
  }

  // ── Round-trip: pipe → length-prefix → decode ────────────────────────

  #[test]
  fn roundtrip_large_payload() {
    let pipe = UnboundedOutputPipe::new();
    let mut stream = pipe.stream();

    // Build a 100 KB JSON-ish payload
    let payload = {
      let mut s = String::from(r#"{"status":200,"body":"#);
      while s.len() < 100 * 1024 - 20 {
        s.push_str("abcdefghij");
      }
      s.push_str(r#""}"#);
      assert!(s.len() > 64 * 1024, "payload should exceed old 64KB cap");
      s
    };
    let payload_bytes = payload.into_bytes();
    let len = payload_bytes.len() as u32;

    // Write length-prefixed (mimics wasm_adapter)
    stream
      .write(Bytes::copy_from_slice(&len.to_le_bytes()))
      .expect("len write");
    stream
      .write(Bytes::from(payload_bytes.clone()))
      .expect("payload write");

    // Read back (mimics execute_wasm_handler)
    let contents = pipe.contents();
    let decoded = decode_length_prefixed(&contents).expect("should decode");
    assert_eq!(decoded.len(), payload_bytes.len());
    assert_eq!(decoded, payload_bytes);
  }
}

//! WASM adapter for Panorama plugins (compiled for wasm32-wasip1 only).
//!
//! Each `panorama-app-*` crate has a `[[bin]]` target that calls
//! [`run_plugin`] with its plugin instance.  The adapter handles
//! stdin/stdout I/O, a sync async-bridge, and a [`WasmPluginContext`]
//! that maps `PluginContext` trait methods to host function imports.
//!
//! Architecture (no special backdoors):
//!   app crate (WASM) → Plugin::handle_http_request()
//!                    → PluginContext trait methods
//!                    → host function imports (env::host_ctx_*)
//!   host (panorama-server) → wasm_runtime links host_ctx_* functions
//!                          → RuntimeContext (capability checks + storage)

use crate::plugin::{
  HttpRequest, HttpResponse, LogLevel, ObjectData, Plugin, PluginContext, PluginError,
};
use crate::types::ObjectRef;
use crate::types::{FieldValue, Node};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::future::Future;
use std::io::Read;
use std::pin::Pin;
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
use uuid::Uuid;

// ── Host function imports (wasm32 only) ─────────────────────────────────────

extern "C" {
  fn host_ctx_create_nodes(fields_ptr: i32, fields_len: i32, result_ptr: i32) -> i32;
  fn host_ctx_create_node(fields_ptr: i32, fields_len: i32, result_ptr: i32) -> i32;
  fn host_ctx_get_node(id_ptr: i32, id_len: i32, result_ptr: i32) -> i32;
  fn host_ctx_update_node(
    id_ptr: i32,
    id_len: i32,
    fields_ptr: i32,
    fields_len: i32,
    result_ptr: i32,
  ) -> i32;
  fn host_ctx_delete_node(id_ptr: i32, id_len: i32);
  fn host_ctx_query(query_ptr: i32, query_len: i32, result_ptr: i32) -> i32;
  fn host_ctx_query_fetch(cursor_id: i32, result_ptr: i32) -> i32;
  fn host_ctx_query_close(cursor_id: i32);
  fn host_ctx_log(msg_ptr: i32, msg_len: i32);
  /// Report a panic message + location to the host before aborting.
  /// Fire-and-forget; the host stores the message for the trap error path.
  fn host_report_panic(msg_ptr: i32, msg_len: i32);
}

// ── I/O types ───────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct WasmInput {
  pub endpoint: String,
  pub request: WasmHttpRequest,
}

#[derive(Debug, Deserialize, Clone)]
struct WasmHttpRequest {
  pub method: String,
  #[serde(default)]
  pub query_params: HashMap<String, String>,
  #[serde(default)]
  pub headers: HashMap<String, String>,
  pub body: Option<String>,
}

#[derive(Debug, Serialize)]
struct WasmOutput {
  pub status: u16,
  #[serde(default)]
  pub headers: HashMap<String, String>,
  #[serde(default)]
  pub body: serde_json::Value,
}

/// Response from host_ctx_query: contains a cursor ID for streaming.
#[derive(Debug, Deserialize)]
struct CursorResponse {
  cursor: u32,
}

// ── Async bridge (sync-only, panics if any future yields Pending) ───────────

fn dummy_waker() -> Waker {
  unsafe fn clone_raw(_: *const ()) -> RawWaker {
    RawWaker::new(std::ptr::null(), &VTABLE)
  }
  unsafe fn drop_raw(_: *const ()) {}
  unsafe fn wake(_: *const ()) {}
  static VTABLE: RawWakerVTable =
    RawWakerVTable::new(|p| RawWaker::new(p, &VTABLE), wake, wake, drop_raw);
  unsafe { Waker::from_raw(RawWaker::new(std::ptr::null(), &VTABLE)) }
}

pub fn block_on<F: Future>(mut f: F) -> F::Output {
  let waker = dummy_waker();
  let mut cx = Context::from_waker(&waker);
  // Safety: we own the future and pin it on the stack
  let mut pinned = unsafe { Pin::new_unchecked(&mut f) };
  loop {
    match pinned.as_mut().poll(&mut cx) {
      Poll::Ready(v) => return v,
      Poll::Pending => {
        panic!("async future yielded Pending in WASM — all host functions must be synchronous")
      }
    }
  }
}

// ── WASM PluginContext ──────────────────────────────────────────────────────

pub struct WasmPluginContext;

impl WasmPluginContext {
  pub fn new() -> Self {
    Self
  }
}

fn call_host(name: &str, input: &[u8], out_buf: &mut [u8]) -> Option<usize> {
  let result_len = match name {
    "create_nodes" => unsafe {
      host_ctx_create_nodes(
        input.as_ptr() as i32,
        input.len() as i32,
        out_buf.as_mut_ptr() as i32,
      )
    },
    "create_node" => unsafe {
      host_ctx_create_node(
        input.as_ptr() as i32,
        input.len() as i32,
        out_buf.as_mut_ptr() as i32,
      )
    },
    "get_node" => unsafe {
      host_ctx_get_node(
        input.as_ptr() as i32,
        input.len() as i32,
        out_buf.as_mut_ptr() as i32,
      )
    },
    "update_node" => unsafe {
      let id_len = u32::from_le_bytes([input[0], input[1], input[2], input[3]]) as usize;
      let id_bytes = &input[4..4 + id_len];
      let fields_bytes = &input[4 + id_len..];
      host_ctx_update_node(
        id_bytes.as_ptr() as i32,
        id_bytes.len() as i32,
        fields_bytes.as_ptr() as i32,
        fields_bytes.len() as i32,
        out_buf.as_mut_ptr() as i32,
      )
    },
    "delete_node" => unsafe {
      host_ctx_delete_node(input.as_ptr() as i32, input.len() as i32);
      return Some(0);
    },
    "query" => unsafe {
      host_ctx_query(
        input.as_ptr() as i32,
        input.len() as i32,
        out_buf.as_mut_ptr() as i32,
      )
    },
    _ => return None,
  };
  if result_len <= 0 {
    None
  } else {
    Some(result_len as usize)
  }
}

/// Buffer size for host calls (1 MiB).  Fixed allocation — growing
/// (doubling + zeroing) crashes dlmalloc in WASM, so we pre-allocate.
const HOST_BUF_SIZE: usize = 1024 * 1024;

fn call_host_buffered(name: &str, input: &[u8]) -> Option<Vec<u8>> {
  let mut buf = vec![0u8; HOST_BUF_SIZE];
  match call_host(name, input, &mut buf) {
    Some(written) => {
      buf.truncate(written);
      Some(buf)
    }
    None => None,
  }
}

/// Try to deserialize a host response buffer into `T`.  If the response is
/// a host error (`{"error": PluginError}`), extract and propagate the
/// error with its structured data intact — never flatten to string.
fn parse_host_response<T: serde::de::DeserializeOwned>(
  data: &[u8],
  host_fn_name: &str,
) -> Result<T, PluginError> {
  // Try the expected type first.
  match serde_json::from_slice::<T>(data) {
    Ok(val) => return Ok(val),
    Err(deser_err) => {
      // If that fails, check if it's a host error response.
      if let Ok(wrapper) = serde_json::from_slice::<serde_json::Value>(data) {
        if let Some(err_val) = wrapper.get("error") {
          if let Ok(host_err) = serde_json::from_value::<PluginError>(err_val.clone()) {
            return Err(host_err);
          }
          // If the error field is a plain string (legacy format), wrap it.
          if let Some(msg) = err_val.as_str() {
            return Err(PluginError::internal(format!(
              "{} host error: {}",
              host_fn_name, msg
            )));
          }
        }
        // We parsed valid JSON but it doesn't match T and isn't an error
        // wrapper.  The schema doesn't match — surface what we got.
        let preview = truncate_utf8(&String::from_utf8_lossy(data), MAX_ERROR_PREVIEW_LEN);
        return Err(PluginError::internal(format!(
          "{} response parse failed: deserialization error: {} | data ({} B): {}",
          host_fn_name,
          deser_err,
          data.len(),
          preview
        )));
      }
      // Not even valid JSON — probably truncated or binary garbage.
      let preview = truncate_utf8(&String::from_utf8_lossy(data), MAX_ERROR_PREVIEW_LEN);
      Err(PluginError::internal(format!(
        "{} response parse failed: deserialization error: {} | data ({} B) is not valid JSON: {}",
        host_fn_name,
        deser_err,
        data.len(),
        preview
      )))
    }
  }
}

/// Max bytes of raw response data to embed in error messages.
const MAX_ERROR_PREVIEW_LEN: usize = 500;

fn truncate_utf8(s: &str, max_len: usize) -> String {
  if s.len() <= max_len {
    return s.to_string();
  }
  let mut end = max_len;
  while end > 0 && !s.is_char_boundary(end) {
    end -= 1;
  }
  format!("{}…<truncated>", &s[..end])
}

#[async_trait]
impl PluginContext for WasmPluginContext {
  async fn create_nodes(&self, nodes: Vec<Node>) -> Result<Vec<Node>, PluginError> {
    let json = serde_json::to_vec(&nodes).unwrap_or_default();
    let data = call_host_buffered("create_nodes", &json)
      .ok_or_else(|| PluginError::internal("host_ctx_create_nodes failed".into()))?;
    parse_host_response(&data, "create_nodes")
  }

  async fn get_node(&self, id: Uuid) -> Result<Option<Node>, PluginError> {
    let id_str = id.to_string();
    let data = call_host_buffered("get_node", id_str.as_bytes())
      .ok_or_else(|| PluginError::internal("host_ctx_get_node failed".into()))?;
    parse_host_response(&data, "get_node")
  }

  async fn update_node(
    &self,
    id: Uuid,
    fields: HashMap<String, FieldValue>,
  ) -> Result<Node, PluginError> {
    let id_str = id.to_string();
    let id_bytes = id_str.as_bytes();
    let fields_json = serde_json::to_vec(&fields).unwrap_or_default();
    let mut packed = Vec::with_capacity(4 + id_bytes.len() + fields_json.len());
    packed.extend_from_slice(&(id_bytes.len() as u32).to_le_bytes());
    packed.extend_from_slice(id_bytes);
    packed.extend_from_slice(&fields_json);

    let data = call_host_buffered("update_node", &packed)
      .ok_or_else(|| PluginError::internal("host_ctx_update_node failed".into()))?;
    parse_host_response(&data, "update_node")
  }

  async fn delete_node(&self, id: Uuid) -> Result<(), PluginError> {
    let id_str = id.to_string();
    call_host("delete_node", id_str.as_bytes(), &mut []);
    Ok(())
  }

  async fn query(&self, query_string: &str) -> Result<Vec<serde_json::Value>, PluginError> {
    // ── Step 1: Initiate query, get cursor ID ──────────────────────────
    let data = call_host_buffered("query", query_string.as_bytes())
      .ok_or_else(|| PluginError::internal("host_ctx_query failed".into()))?;
    let cursor_resp: CursorResponse = parse_host_response(&data, "query")?;

    // ── Step 2: Fetch chunks in a loop ─────────────────────────────────
    let mut rows = Vec::new();
    loop {
      let mut buf = vec![0u8; HOST_BUF_SIZE];
      let written =
        unsafe { host_ctx_query_fetch(cursor_resp.cursor as i32, buf.as_mut_ptr() as i32) };
      if written <= 0 {
        break;
      }
      let chunk: Vec<serde_json::Value> = serde_json::from_slice(&buf[..written as usize])
        .map_err(|e| {
          PluginError::internal(format!(
            "query fetch parse failed: {} | preview: {}",
            e,
            truncate_utf8(
              &String::from_utf8_lossy(&buf[..written as usize]),
              MAX_ERROR_PREVIEW_LEN
            )
          ))
        })?;
      if chunk.is_empty() {
        break;
      }
      rows.extend(chunk);
    }

    // ── Step 3: Close cursor ───────────────────────────────────────────
    unsafe {
      host_ctx_query_close(cursor_resp.cursor as i32);
    }

    Ok(rows)
  }

  async fn register_schema(
    &self,
    _schema: crate::schema::Schema,
  ) -> Result<crate::schema::Schema, PluginError> {
    Err(PluginError::internal(
      "schema ops not available in WASM".into(),
    ))
  }

  async fn get_schema(
    &self,
    _schema_node_id: Uuid,
  ) -> Result<Option<crate::schema::Schema>, PluginError> {
    Err(PluginError::internal(
      "schema ops not available in WASM".into(),
    ))
  }

  async fn put_object(
    &self,
    _bucket: &str,
    _key: &str,
    _data: bytes::Bytes,
    _mime_type: &str,
  ) -> Result<ObjectRef, PluginError> {
    Err(PluginError::internal(
      "object storage not available in WASM".into(),
    ))
  }

  async fn get_object(&self, _bucket: &str, _key: &str) -> Result<Option<ObjectData>, PluginError> {
    Err(PluginError::internal(
      "object storage not available in WASM".into(),
    ))
  }

  async fn delete_object(&self, _bucket: &str, _key: &str) -> Result<(), PluginError> {
    Err(PluginError::internal(
      "object storage not available in WASM".into(),
    ))
  }

  async fn list_objects(
    &self,
    _bucket: &str,
    _prefix: Option<&str>,
  ) -> Result<Vec<ObjectRef>, PluginError> {
    Err(PluginError::internal(
      "object storage not available in WASM".into(),
    ))
  }

  fn plugin_id(&self) -> &str {
    "wasm-plugin"
  }

  fn base_path(&self) -> String {
    "/plugin/wasm-plugin".into()
  }

  async fn log(&self, _level: LogLevel, message: &str) {
    unsafe {
      host_ctx_log(message.as_ptr() as i32, message.len() as i32);
    }
  }
}

// ── Entry point ─────────────────────────────────────────────────────────────

pub fn run_plugin(plugin: impl Plugin + 'static) {
  // Install a panic hook that reports the panic message + location to the
  // host before the wasm module aborts.  This is fire-and-forget — the host
  // stores the message so the trap error path can attach it to the error.
  // Must be installed ONCE at plugin init, not per call site.
  #[cfg(target_arch = "wasm32")]
  {
    std::panic::set_hook(Box::new(|info| {
      let msg = if let Some(s) = info.payload().downcast_ref::<&str>() {
        s.to_string()
      } else if let Some(s) = info.payload().downcast_ref::<String>() {
        s.clone()
      } else {
        "unknown panic".to_string()
      };
      let loc = info
        .location()
        .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
        .unwrap_or_default();
      let full_msg = format!("{} (at {})", msg, loc);
      unsafe {
        host_report_panic(full_msg.as_ptr() as i32, full_msg.len() as i32);
      }
    }));
  }

  let mut buf = String::new();
  std::io::stdin().read_to_string(&mut buf).ok();
  let input: WasmInput = serde_json::from_str(&buf).unwrap_or_else(|_| WasmInput {
    endpoint: String::new(),
    request: WasmHttpRequest {
      method: "GET".into(),
      query_params: HashMap::new(),
      headers: HashMap::new(),
      body: None,
    },
  });

  let ctx = WasmPluginContext::new();
  let request = HttpRequest {
    method: input.request.method.clone(),
    path: input.endpoint.clone(),
    query_params: input.request.query_params.clone(),
    headers: input.request.headers.clone(),
    body: input
      .request
      .body
      .as_ref()
      .map(|b| bytes::Bytes::from(b.clone())),
  };

  let result = block_on(plugin.handle_http_request(&input.endpoint, request, &ctx));

  let output = match result {
    Ok(response) => WasmOutput {
      status: response.status,
      headers: response.headers,
      body: serde_json::from_slice(&response.body).unwrap_or(serde_json::Value::Null),
    },
    Err(e) => {
      let mut error_body = serde_json::json!({
        "error": e.message,
        "code": e.code,
      });
      // Carry structured diagnostics — never flatten to string.
      if let Some(ref loc) = e.location {
        error_body["location"] = serde_json::json!({
          "file": loc.file,
          "line": loc.line,
          "column": loc.column,
        });
      }
      if let Some(id) = e.backtrace_id {
        error_body["backtrace_id"] = serde_json::json!(id);
      }
      if let Some(ref frames) = e.trap_frames {
        error_body["trap_frames"] = serde_json::to_value(frames).unwrap_or_default();
      }
      WasmOutput {
        status: e.status,
        headers: {
          let mut h = HashMap::new();
          h.insert("Content-Type".into(), "application/json".into());
          h
        },
        body: error_body,
      }
    }
  };

  // Length-prefixed protocol: 4-byte LE u32 length then JSON payload.
  // This lets the host know the exact size and avoids fixed-buffer truncation.
  let output_json = serde_json::to_string(&output).unwrap_or_default();
  let output_bytes = output_json.as_bytes();
  let len = output_bytes.len() as u32;
  let mut stdout = std::io::stdout();
  use std::io::Write;
  stdout.write_all(&len.to_le_bytes()).ok();
  stdout.write_all(output_bytes).ok();
  stdout.flush().ok();
}

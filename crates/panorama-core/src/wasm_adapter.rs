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

use async_trait::async_trait;
use crate::plugin::{
    HttpRequest, HttpResponse, LogLevel, ObjectData, Plugin, PluginContext, PluginError,
};
use crate::types::ObjectRef;
use crate::types::{FieldValue, Node};
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
    fn host_ctx_update_node(id_ptr: i32, id_len: i32, fields_ptr: i32, fields_len: i32, result_ptr: i32) -> i32;
    fn host_ctx_delete_node(id_ptr: i32, id_len: i32);
    fn host_ctx_query(query_ptr: i32, query_len: i32, result_ptr: i32) -> i32;
    fn host_ctx_log(msg_ptr: i32, msg_len: i32);
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

// ── Async bridge (sync-only, panics if any future yields Pending) ───────────

fn dummy_waker() -> Waker {
    unsafe fn clone_raw(_: *const ()) -> RawWaker {
        RawWaker::new(std::ptr::null(), &VTABLE)
    }
    unsafe fn drop_raw(_: *const ()) {}
    unsafe fn wake(_: *const ()) {}
    static VTABLE: RawWakerVTable = RawWakerVTable::new(
        |p| RawWaker::new(p, &VTABLE),
        wake,
        wake,
        drop_raw,
    );
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
            Poll::Pending => panic!(
                "async future yielded Pending in WASM — all host functions must be synchronous"
            ),
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
            host_ctx_create_nodes(input.as_ptr() as i32, input.len() as i32, out_buf.as_mut_ptr() as i32)
        }
        "create_node" => unsafe {
            host_ctx_create_node(input.as_ptr() as i32, input.len() as i32, out_buf.as_mut_ptr() as i32)
        }
        "get_node" => unsafe {
            host_ctx_get_node(input.as_ptr() as i32, input.len() as i32, out_buf.as_mut_ptr() as i32)
        }
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
        }
        "delete_node" => unsafe {
            host_ctx_delete_node(input.as_ptr() as i32, input.len() as i32);
            return Some(0);
        }
        "query" => unsafe {
            host_ctx_query(input.as_ptr() as i32, input.len() as i32, out_buf.as_mut_ptr() as i32)
        }
        _ => return None,
    };
    if result_len <= 0 {
        None
    } else {
        Some(result_len as usize)
    }
}

#[async_trait]
impl PluginContext for WasmPluginContext {
    async fn create_nodes(&self, nodes: Vec<Node>) -> Result<Vec<Node>, PluginError> {
        let json = serde_json::to_vec(&nodes).unwrap_or_default();
        let mut buf = vec![0u8; 131072];
        match call_host("create_nodes", &json, &mut buf) {
            Some(len) => {
                let len = len.min(buf.len());
                serde_json::from_slice(&buf[..len])
                    .map_err(|e| PluginError::internal(e.to_string()))
            }
            None => Err(PluginError::internal("host_ctx_create_nodes failed".into())),
        }
    }

    async fn get_node(&self, id: Uuid) -> Result<Option<Node>, PluginError> {
        let id_str = id.to_string();
        let mut buf = vec![0u8; 32768];
        match call_host("get_node", id_str.as_bytes(), &mut buf) {
            Some(len) => {
                let len = len.min(buf.len());
                serde_json::from_slice(&buf[..len])
                    .map_err(|e| PluginError::internal(e.to_string()))
            }
            None => Ok(None),
        }
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

        let mut buf = vec![0u8; 32768];
        match call_host("update_node", &packed, &mut buf) {
            Some(len) => {
                let len = len.min(buf.len());
                serde_json::from_slice(&buf[..len])
                    .map_err(|e| PluginError::internal(e.to_string()))
            }
            None => Err(PluginError::internal("host_ctx_update_node failed".into())),
        }
    }

    async fn delete_node(&self, id: Uuid) -> Result<(), PluginError> {
        let id_str = id.to_string();
        call_host("delete_node", id_str.as_bytes(), &mut []);
        Ok(())
    }

    async fn query(&self, query_string: &str) -> Result<Vec<serde_json::Value>, PluginError> {
        let mut buf = vec![0u8; 65536];
        match call_host("query", query_string.as_bytes(), &mut buf) {
            Some(len) => {
                let len = len.min(buf.len());
                serde_json::from_slice(&buf[..len])
                    .map_err(|e| PluginError::internal(e.to_string()))
            }
            None => Ok(vec![]),
        }
    }

    async fn register_schema(
        &self,
        _schema: crate::schema::Schema,
    ) -> Result<crate::schema::Schema, PluginError> {
        Err(PluginError::internal("schema ops not available in WASM".into()))
    }

    async fn get_schema(
        &self,
        _schema_node_id: Uuid,
    ) -> Result<Option<crate::schema::Schema>, PluginError> {
        Err(PluginError::internal("schema ops not available in WASM".into()))
    }

    async fn put_object(
        &self,
        _bucket: &str,
        _key: &str,
        _data: bytes::Bytes,
        _mime_type: &str,
    ) -> Result<ObjectRef, PluginError> {
        Err(PluginError::internal("object storage not available in WASM".into()))
    }

    async fn get_object(
        &self,
        _bucket: &str,
        _key: &str,
    ) -> Result<Option<ObjectData>, PluginError> {
        Err(PluginError::internal("object storage not available in WASM".into()))
    }

    async fn delete_object(&self, _bucket: &str, _key: &str) -> Result<(), PluginError> {
        Err(PluginError::internal("object storage not available in WASM".into()))
    }

    async fn list_objects(
        &self,
        _bucket: &str,
        _prefix: Option<&str>,
    ) -> Result<Vec<ObjectRef>, PluginError> {
        Err(PluginError::internal("object storage not available in WASM".into()))
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
        Err(e) => WasmOutput {
            status: e.status,
            headers: {
                let mut h = HashMap::new();
                h.insert("Content-Type".into(), "application/json".into());
                h
            },
            body: serde_json::json!({"error": e.message, "code": e.code}),
        },
    };

    println!("{}", serde_json::to_string(&output).unwrap_or_default());
}

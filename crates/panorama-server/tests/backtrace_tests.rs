//! Integration tests for wasm backtrace capture.
//!
//! Verifies both the trap path (automatic) and the recoverable-error path
//! (opt-in via `PluginError::with_backtrace` / `host_capture_backtrace`).
//!
//! These tests compile and run the `panorama-app-backtrace-test` WASM binary,
//! then assert that structured backtrace data survives the full round-trip.

use std::sync::Arc;

use panorama_core::capabilities::CapabilityGrants;
use panorama_core::plugin::HttpRequest;
use panorama_server::backtrace::BacktraceStore;
use panorama_server::object_store::ObjectStorage;
use panorama_server::plugin_runtime::RuntimeContext;
use panorama_server::schema_registry::SchemaRegistry;
use panorama_server::storage::{sqlite::SqliteBackend, NodeStorage};

/// Path to the compiled WASM binary (relative to workspace root at runtime).
/// Built by: `cargo build -p panorama-app-backtrace-test --target wasm32-wasip1`
const WASM_PATH: &str = concat!(
  env!("CARGO_MANIFEST_DIR"),
  "/../../target/wasm32-wasip1/debug/backtrace-test.wasm"
);

fn setup_test_env() -> (
  NodeStorage,
  SchemaRegistry,
  ObjectStorage,
  tempfile::TempDir,
) {
  let tmp = tempfile::tempdir().unwrap();
  let backend = Arc::new(SqliteBackend::new(tmp.path().join("nodes")));
  let storage = NodeStorage::new(backend);
  let schema_registry = SchemaRegistry::new(None);

  // Register system schemas
  schema_registry
    .register(panorama_core::schema::system_schemas::node_time_schema())
    .unwrap();
  schema_registry
    .register(panorama_core::schema::system_schemas::node_info_schema())
    .unwrap();
  schema_registry
    .register(panorama_core::schema::system_schemas::reactors_schema())
    .unwrap();
  schema_registry
    .register(panorama_core::schema::system_schemas::op_stream_schema())
    .unwrap();
  schema_registry
    .register(panorama_core::schema::system_schemas::reactor_state_schema())
    .unwrap();

  let object_storage = ObjectStorage::new(tmp.path().join("objects"));
  (storage, schema_registry, object_storage, tmp)
}

fn compile_wasm(engine: &wasmtime::Engine) -> wasmtime::Module {
  let wasm_bytes = std::fs::read(WASM_PATH).expect("WASM binary not found — build it first with: cargo build -p panorama-app-backtrace-test --target wasm32-wasip1");
  wasmtime::Module::from_binary(engine, &wasm_bytes).expect("WASM compile failed")
}

fn make_request(endpoint: &str) -> HttpRequest {
  HttpRequest {
    method: "POST".into(),
    path: endpoint.into(),
    query_params: Default::default(),
    headers: Default::default(),
    body: Some(
      serde_json::json!({"test": true})
        .to_string()
        .into_bytes()
        .into(),
    ),
  }
}

// ── Trap path ─────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_trap_backtrace_capture() {
  let _ = tracing_subscriber::fmt().with_test_writer().try_init();
  let (storage, schema_registry, object_storage, _tmp) = setup_test_env();

  let mut config = wasmtime::Config::new();
  config.async_support(true);
  config.generate_address_map(true);
  config.wasm_backtrace_details(wasmtime::WasmBacktraceDetails::Enable);
  let engine = wasmtime::Engine::new(&config).expect("WASM engine");

  let module = compile_wasm(&engine);
  let backtrace_store = Arc::new(BacktraceStore::new());

  let caps = CapabilityGrants::default();
  let instance_pre = panorama_server::wasm_runtime::create_prelinked_instance(
    &engine,
    &module,
    "backtrace-test",
    &caps,
    &storage,
    &schema_registry,
    &object_storage,
    backtrace_store.clone(),
  )
  .expect("pre-link");

  let result = panorama_server::wasm_runtime::execute_wasm_handler(
    &engine,
    &instance_pre,
    "test_trap_error",
    &make_request("test_trap_error"),
    &backtrace_store,
  )
  .await;

  // Must be an error (panic → trap).
  let err = result.expect_err("expected trap error, got Ok");

  // Must have trap frames.
  let frames = err
    .trap_frames
    .as_ref()
    .expect("trap_frames must be Some for a trap error");

  eprintln!(
    "TRAP FRAMES ({}):\n{}",
    frames.len(),
    frames
      .iter()
      .map(|f| {
        let loc = match (&f.file, f.line) {
          (Some(file), Some(line)) => format!("{}:{}", file, line),
          (Some(file), None) => file.clone(),
          _ => "<no source>".to_string(),
        };
        format!(
          "  {} @ {} [{}]",
          f.func_name.as_deref().unwrap_or("<unknown>"),
          f.module_name.as_deref().unwrap_or("<unknown>"),
          loc,
        )
      })
      .collect::<Vec<_>>()
      .join("\n")
  );

  // At minimum we should have frames — the exact count depends on
  // compilation and inlining, but there must be at least one frame.
  assert!(
    !frames.is_empty(),
    "expected at least one wasm frame in trap backtrace"
  );

  // The error message should contain our panic message (reported via
  // the panic hook → host_report_panic → last_panic).
  assert!(
    err.message.contains("intentional trap"),
    "error message should contain the panic text, got: {}",
    err.message
  );
}

// ── Recoverable path ──────────────────────────────────────────────────────────

#[tokio::test]
async fn test_recoverable_backtrace_capture() {
  let (storage, schema_registry, object_storage, _tmp) = setup_test_env();

  let mut config = wasmtime::Config::new();
  config.async_support(true);
  config.generate_address_map(true);
  config.wasm_backtrace_details(wasmtime::WasmBacktraceDetails::Enable);
  let engine = wasmtime::Engine::new(&config).expect("WASM engine");

  let module = compile_wasm(&engine);
  let backtrace_store = Arc::new(BacktraceStore::new());

  let caps = CapabilityGrants::default();
  let instance_pre = panorama_server::wasm_runtime::create_prelinked_instance(
    &engine,
    &module,
    "backtrace-test",
    &caps,
    &storage,
    &schema_registry,
    &object_storage,
    backtrace_store.clone(),
  )
  .expect("pre-link");

  let result = panorama_server::wasm_runtime::execute_wasm_handler(
    &engine,
    &instance_pre,
    "test_backtrace_error",
    &make_request("test_backtrace_error"),
    &backtrace_store,
  )
  .await;

  // The recoverable error is returned as a "successful" HTTP response
  // with error status/body (the WASM module itself doesn't trap, it
  // returns an error response).  Let's check both paths.
  match result {
    Err(err) => {
      // Direct error from execute_wasm_handler (e.g., if we change the protocol)
      eprintln!("Got direct error: {:?}", err);
      let frames = err
        .trap_frames
        .as_ref()
        .expect("trap_frames must be Some for recoverable error with backtrace");

      eprintln!(
        "RECOVERABLE FRAMES ({}): {:#?}",
        frames.len(),
        frames
          .iter()
          .map(|f| format!(
            "  {} @ {}",
            f.func_name.as_deref().unwrap_or("<unknown>"),
            f.module_name.as_deref().unwrap_or("<unknown>")
          ))
          .collect::<Vec<_>>()
          .join("\n")
      );

      assert!(
        !frames.is_empty(),
        "expected at least one wasm frame in recoverable backtrace"
      );
    }
    Ok(resp) => {
      // The recoverable error comes back as a 500 JSON response body.
      eprintln!(
        "Got response: status={} body={}",
        resp.status,
        String::from_utf8_lossy(&resp.body)
      );
      assert_eq!(resp.status, 500, "expected 500 status for error response");

      // Parse the error body and verify backtrace_id is present.
      let body: serde_json::Value =
        serde_json::from_slice(&resp.body).expect("response body must be valid JSON");
      let backtrace_id = body["backtrace_id"].as_u64();
      assert!(
        backtrace_id.is_some(),
        "response body must contain backtrace_id, got: {}",
        body
      );

      // The store should have the context for this id.
      let ctx = backtrace_store
        .take(backtrace_id.unwrap())
        .expect("backtrace store must contain context for the id");
      eprintln!(
        "RECOVERABLE FRAMES from store ({}):\n{}",
        ctx.frames.len(),
        ctx
          .frames
          .iter()
          .map(|f| {
            let loc = match (&f.file, f.line) {
              (Some(file), Some(line)) => format!("{}:{}", file, line),
              (Some(file), None) => file.clone(),
              _ => "<no source>".to_string(),
            };
            format!(
              "  {} @ {} [{}]",
              f.func_name.as_deref().unwrap_or("<unknown>"),
              f.module_name.as_deref().unwrap_or("<unknown>"),
              loc,
            )
          })
          .collect::<Vec<_>>()
          .join("\n")
      );
      assert!(
        !ctx.frames.is_empty(),
        "expected at least one wasm frame in stored backtrace context"
      );
      assert_eq!(ctx.plugin_id, "backtrace-test");
    }
  }
}

// ── No-flattening check ───────────────────────────────────────────────────────

#[test]
fn test_plugin_error_carries_structured_data() {
  // Verify that PluginError doesn't flatten structured data into the message.
  let err = panorama_core::plugin::PluginError::not_found("test message");

  // The message is human-readable.
  assert_eq!(err.message, "test message");

  // But location (file:line) is carried separately.
  assert!(
    err.location.is_some(),
    "location must be set by #[track_caller]"
  );
  let loc = err.location.unwrap();
  assert!(
    loc.file.contains("backtrace_tests.rs"),
    "location should be in this test file, got: {}",
    loc.file
  );
  assert!(loc.line > 0, "line must be positive");

  // backtrace_id is None for default constructors.
  assert!(
    err.backtrace_id.is_none(),
    "backtrace_id must be None without opt-in"
  );
  assert!(
    err.trap_frames.is_none(),
    "trap_frames must be None for non-trap errors"
  );
}

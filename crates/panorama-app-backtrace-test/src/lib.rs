//! Test-only WASM plugin for backtrace capture verification.
//!
//! Exports two endpoints:
//! - `test_trap_error` — panics to trigger a wasm trap (free backtrace path)
//! - `test_backtrace_error` — returns a recoverable error created deep in a
//!   call chain using `PluginError::with_backtrace` (opt-in host import path)

use async_trait::async_trait;
use panorama_core::plugin::{HttpRequest, HttpResponse, Plugin, PluginContext, PluginError};

pub struct BacktraceTestPlugin;

#[async_trait]
impl Plugin for BacktraceTestPlugin {
  fn id(&self) -> &str {
    "io.mzhang.panorama.backtrace-test"
  }
  fn name(&self) -> &str {
    "Backtrace Test Plugin"
  }
  fn version(&self) -> &str {
    "0.1.0"
  }
  fn description(&self) -> &str {
    "Test-only WASM plugin for backtrace capture verification"
  }

  async fn handle_http_request(
    &self,
    endpoint: &str,
    _request: HttpRequest,
    _ctx: &dyn PluginContext,
  ) -> Result<HttpResponse, PluginError> {
    match endpoint {
      "test_trap_error" => handle_trap_error(),
      "test_backtrace_error" => handle_backtrace_error(),
      _ => Err(PluginError::not_found(&format!(
        "Unknown endpoint: {}",
        endpoint
      ))),
    }
  }
}

// ── Trap path ─────────────────────────────────────────────────────────────────

/// Panics to trigger a wasm trap.  The wasmtime runtime automatically
/// attaches a `WasmBacktrace` to the resulting error — the host extracts
/// it and stores frames in `PluginError::trap_frames`.
fn handle_trap_error() -> Result<HttpResponse, PluginError> {
  panic!("intentional trap for backtrace verification: level 0");
}

// ── Recoverable path ──────────────────────────────────────────────────────────

/// Nested call chain: the error is created at the deepest level using
/// `PluginError::with_backtrace`, which calls `host_capture_backtrace`
/// ONCE.  The resulting backtrace must contain all three levels.
fn handle_backtrace_error() -> Result<HttpResponse, PluginError> {
  level1()
}

fn level1() -> Result<HttpResponse, PluginError> {
  level2()
}

fn level2() -> Result<HttpResponse, PluginError> {
  level3()
}

fn level3() -> Result<HttpResponse, PluginError> {
  Err(PluginError::with_backtrace(
    "TEST_ERROR",
    "deep recoverable error for backtrace verification".into(),
    500,
  ))
}

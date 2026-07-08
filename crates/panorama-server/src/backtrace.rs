//! Host-side backtrace capture and storage.
//!
//! `BacktraceStore` maps opaque u64 ids to `CapturedContext` snapshots
//! taken by `host_capture_backtrace`.  The guest holds the id in its
//! `PluginError`; when the error surfaces at the export boundary the
//! host resolves the id and attaches the structured data to Sentry.
//!
//! One call to `host_capture_backtrace` captures everything reachable
//! at that instant — wasm frames, tracing spans, and request context —
//! so no per-frame or per-function host imports are needed.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

/// A single wasm frame, extracted from `wasmtime::FrameInfo` at capture time.
/// `FrameInfo` does not implement `Clone`, so we extract the data we need
/// into our own type immediately.
///
/// If wasmtime was compiled with `addr2line` and the wasm module has DWARF
/// debug info, `file`, `line`, and `column` will be populated from the
/// `FrameSymbol` data.
#[derive(Debug, Clone)]
pub struct StoredFrame {
  pub func_name: Option<String>,
  pub func_index: u32,
  pub module_name: Option<String>,
  /// Source file (from DWARF), if available.
  pub file: Option<String>,
  /// Source line (from DWARF), if available.
  pub line: Option<u32>,
  /// Source column (from DWARF), if available.
  pub column: Option<u32>,
}

impl StoredFrame {
  pub fn from_frame_info(f: &wasmtime::FrameInfo) -> Self {
    // Extract the first symbol with file/line info (from addr2line/DWARF).
    let symbols = f.symbols();
    let best_symbol = symbols
      .iter()
      .find(|s| s.file().is_some() || s.name().is_some());

    Self {
      func_name: f
        .func_name()
        .map(|s| s.to_string())
        .or_else(|| best_symbol.and_then(|s| s.name().map(|n| n.to_string()))),
      func_index: f.func_index(),
      module_name: f.module().name().map(|s| s.to_string()),
      file: best_symbol.and_then(|s| s.file().map(|f| f.to_string())),
      line: best_symbol.and_then(|s| s.line()),
      column: best_symbol.and_then(|s| s.column()),
    }
  }
}

impl From<&StoredFrame> for panorama_core::plugin::TrapFrame {
  fn from(f: &StoredFrame) -> Self {
    Self {
      func_name: f.func_name.clone(),
      func_index: f.func_index,
      module_name: f.module_name.clone(),
      file: f.file.clone(),
      line: f.line,
      column: f.column,
    }
  }
}

/// A snapshot of a tracing span at the moment of capture.
#[derive(Debug, Clone)]
pub struct SpanSnapshot {
  pub name: String,
  pub fields: HashMap<String, String>,
}

/// Everything the host can observe at the instant `host_capture_backtrace`
/// fires inside the guest.
#[derive(Debug, Clone)]
pub struct CapturedContext {
  /// Wasm frames, innermost-first (as returned by `WasmBacktrace::frames`).
  pub frames: Vec<StoredFrame>,
  /// Tracing spans from current → root, innermost-first.
  pub spans: Vec<SpanSnapshot>,
  /// The plugin that produced this backtrace.
  pub plugin_id: String,
  /// Per-request correlation id, if available.
  pub request_id: Option<String>,
}

/// Thread-safe store for host-captured backtrace contexts.
///
/// Shared between the `host_capture_backtrace` host function (writer)
/// and the error-reporting path that reads frames after execution
/// completes (reader).
///
/// Also stores the most recent panic message from
/// `host_report_panic`, consumed by the trap error path.
#[derive(Debug)]
pub struct BacktraceStore {
  contexts: Mutex<HashMap<u64, CapturedContext>>,
  next_id: AtomicU64,
  /// Most recent panic message + location, set by `host_report_panic`.
  /// Consumed by the trap error path and cleared after read.
  pub last_panic: Mutex<Option<String>>,
}

impl BacktraceStore {
  pub fn new() -> Self {
    Self {
      contexts: Mutex::new(HashMap::new()),
      next_id: AtomicU64::new(1),
      last_panic: Mutex::new(None),
    }
  }

  /// Store a captured context and return its opaque id.
  /// Returns 0 on failure (e.g. poison error).
  pub fn insert(&self, ctx: CapturedContext) -> u64 {
    let id = self.next_id.fetch_add(1, Ordering::Relaxed);
    match self.contexts.lock() {
      Ok(mut map) => {
        map.insert(id, ctx);
        id
      }
      Err(_) => 0,
    }
  }

  /// Retrieve and remove a captured context by id.
  pub fn take(&self, id: u64) -> Option<CapturedContext> {
    self
      .contexts
      .lock()
      .ok()
      .and_then(|mut map| map.remove(&id))
  }

  /// Retrieve a captured context without removing it.
  pub fn get(&self, id: u64) -> Option<CapturedContext> {
    self
      .contexts
      .lock()
      .ok()
      .and_then(|map| map.get(&id).cloned())
  }
}

impl Default for BacktraceStore {
  fn default() -> Self {
    Self::new()
  }
}

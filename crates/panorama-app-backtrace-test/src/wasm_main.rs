//! WASM binary entry point for the backtrace test plugin.

#[cfg(target_arch = "wasm32")]
fn main() {
  panorama_core::wasm_adapter::run_plugin(panorama_app_backtrace_test::BacktraceTestPlugin);
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {}

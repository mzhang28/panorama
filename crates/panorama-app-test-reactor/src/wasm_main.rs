//! WASM binary entry point for the test reactor plugin.

#[cfg(target_arch = "wasm32")]
fn main() {
  panorama_core::wasm_adapter::run_plugin(panorama_app_test_reactor::TestReactorPlugin);
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {}

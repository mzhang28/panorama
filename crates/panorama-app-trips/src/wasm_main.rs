#[cfg(target_arch = "wasm32")]
fn main() {
    panorama_core::wasm_adapter::run_plugin(panorama_app_trips::TripsPlugin::new());
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {}


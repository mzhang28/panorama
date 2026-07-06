#[cfg(target_arch = "wasm32")]
fn main() {
    panorama_core::wasm_adapter::run_plugin(panorama_app_music::MusicPlugin::new());
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {}


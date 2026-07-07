---
title: Building & Packaging
description: Step-by-step guide to compiling and packaging a new application into a .panoapp bundle.
---

This guide describes how to develop, compile, and package a new third-party application **out-of-tree** (as an independent repository outside the main Panorama platform tree). 

The final output is a `.panoapp` bundle containing the compiled WASM backend, static frontend resources, and a descriptor manifest.

---

## Step 1: Implement the Plugin Trait

Create a new standalone Rust library crate on your local machine:

```bash
cargo new --lib my-plugin
cd my-plugin
```

### Configure `Cargo.toml`
Add `panorama-core` as a dependency pointing to the main git repository. Your crate needs both a library target (for the plugin logic) and a binary target (for the WASM entry point):

```toml
[package]
name = "my-plugin"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[[bin]]
name = "my-plugin"
path = "src/wasm_main.rs"

[dependencies]
async-trait = "0.1"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Import core plugin abstractions from the platform repository
panorama-core = { git = "https://github.com/withpanorama/panorama.git", branch = "main" }
```

### Implement the trait in `src/lib.rs`
Implement the `Plugin` trait to define your schemas, endpoint routes, UI components, capabilities, and request logic:

```rust
use async_trait::async_trait;
use panorama_core::plugin::*;
use panorama_core::schema::Schema;
use panorama_core::types::{HttpRequest, HttpResponse};
use panorama_core::capabilities::CapabilityGrants;

pub struct MyPlugin;

#[async_trait]
impl Plugin for MyPlugin {
    fn id(&self) -> &str {
        "com.example.myplugin"
    }

    fn name(&self) -> &str {
        "My Custom Plugin"
    }

    fn version(&self) -> &str {
        "0.1.0"
    }

    fn description(&self) -> &str {
        "Handles custom records and queries out-of-tree."
    }

    fn schemas(&self) -> Vec<Schema> {
        vec![]
    }

    fn http_endpoints(&self) -> Vec<HttpEndpoint> {
        vec![
            HttpEndpoint {
                path: "/hello".to_string(),
                method: HttpMethod::GET,
                description: "Returns a greeting".to_string(),
            }
        ]
    }

    fn ui_components(&self) -> Vec<UiComponent> {
        vec![
            UiComponent {
                id: "my-plugin-main".to_string(),
                name: "MainView".to_string(),
                mount_point: UiMountPoint::MainPage,
                bundle_path: "ui/remoteEntry.js".to_string(),
            }
        ]
    }

    fn background_tasks(&self) -> Vec<BackgroundTask> {
        vec![]
    }

    fn required_capabilities(&self) -> CapabilityGrants {
        CapabilityGrants::default()
    }

    async fn initialize(&self, _ctx: &dyn PluginContext) -> Result<(), PluginError> {
        Ok(())
    }

    async fn handle_http_request(
        &self,
        endpoint: &str,
        request: HttpRequest,
        ctx: &dyn PluginContext,
    ) -> Result<HttpResponse, PluginError> {
        match (endpoint, request.method.as_str()) {
            ("/hello", "GET") => {
                Ok(HttpResponse::ok(b"{\"message\":\"Hello from out-of-tree WASM!\"}"))
            }
            _ => Err(PluginError::not_found("Endpoint not found")),
        }
    }

    async fn run_background_task(
        &self,
        _task_name: &str,
        _ctx: &dyn PluginContext,
    ) -> Result<(), PluginError> {
        Err(PluginError::not_found("No background task implemented"))
    }
}
```

### Wire up the WASM entry point in `src/wasm_main.rs`
Create a separate binary entry point that hands your plugin instance to the WASM adapter:

```rust
#[cfg(target_arch = "wasm32")]
fn main() {
    panorama_core::wasm_adapter::run_plugin(
        my_plugin::MyPlugin
    );
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {}
```

---

## Step 2: Compile the WASM Module

Compile the Rust crate targeting `wasm32-wasip1`. You must pass the `-C link-arg=--allow-undefined` flag via `RUSTFLAGS` so that the imports for the host's database and logging functions resolve dynamically at runtime inside the server sandbox:

```bash
RUSTFLAGS="-C link-arg=--allow-undefined" cargo build --release --target wasm32-wasip1
```

The compiled WASM module is outputted to:
`target/wasm32-wasip1/release/my_plugin.wasm`

---

## Step 3: Build the UI Assets

If your application provides a custom frontend panel, create a standard frontend project (e.g. React + Vite) that builds a Module Federation remote bundle:

```bash
cd ui
bun install
bun run build
```

This compiles your production static web resources into `ui/dist/` (which must include `index.html` and a `remoteEntry.js` entrypoint file).

---

## Step 4: Package into a `.panoapp` File

A `.panoapp` file is simply a standard **ZIP archive** with a specific folder layout. You do not need the platform's build scripts to package it; any standard ZIP utility works.

### Layout Requirements
The ZIP archive must contain:
1.  `manifest.json` at the root.
2.  `plugin.wasm` (renamed from `my_plugin.wasm`) at the root.
3.  A `ui/` directory containing all your static web assets.

### Packaging Example
You can bundle the application from your terminal using standard filesystem utilities:

```bash
# 1. Create a temporary packaging directory
mkdir -p build/ui/

# 2. Copy the manifest and rename the WASM binary
cp manifest.json build/
cp target/wasm32-wasip1/release/my_plugin.wasm build/plugin.wasm

# 3. Copy the compiled UI assets
cp -r ui/dist/* build/ui/

# 4. Generate the ZIP bundle
cd build
zip -r ../my-app.panoapp manifest.json plugin.wasm ui/

# 5. Clean up temporary files
cd ..
rm -rf build/
```

This creates `my-app.panoapp` which is ready to be dropped into the server's `plugins/` directory.


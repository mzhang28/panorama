use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    pub authors: Vec<String>,
    pub lua_entrypoint: String,
    pub qt_library: Option<String>,
    pub permissions: Vec<String>,
    pub lua_functions: Vec<String>,
    pub fields: HashMap<String, String>, // field name -> type
    #[serde(default)]
    pub regexes: Vec<String>,
}

impl PluginManifest {
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let content = fs::read_to_string(path)?;
        let manifest: PluginManifest = toml::from_str(&content)?;
        Ok(manifest)
    }
}

pub struct PluginLoader {
    pub plugins_dir: PathBuf,
    pub loaded_plugins: Vec<PluginManifest>,
}

impl PluginLoader {
    pub fn new<P: Into<PathBuf>>(plugins_dir: P) -> Self {
        Self {
            plugins_dir: plugins_dir.into(),
            loaded_plugins: Vec::new(),
        }
    }

    pub async fn load_plugins(
        &mut self,
        dal: &crate::db::Dal,
    ) -> Result<(), Box<dyn std::error::Error>> {
        for entry in fs::read_dir(&self.plugins_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                let manifest_path = path.join("manifest.toml");
                if manifest_path.exists() {
                    let manifest = PluginManifest::from_file(&manifest_path)?;
                    // Create fields declared in manifest
                    if !manifest.fields.is_empty() {
                        let mut missing = std::collections::HashMap::new();
                        for (key, ty) in &manifest.fields {
                            if !dal.has_schema_key(key).await? {
                                missing.insert(key.clone(), ty.clone());
                            }
                        }
                        if !missing.is_empty() {
                            dal.create_table(&manifest.name, missing).await?;
                        }
                    }
                    // TODO: Load Lua entrypoint and Qt library
                    self.loaded_plugins.push(manifest);
                }
            }
        }
        Ok(())
    }

    // Placeholder for loading Lua entrypoint
    pub fn load_lua_entrypoint(&self, _manifest: &PluginManifest) {
        // TODO: Implement Lua VM allocation and loading
    }

    // Placeholder for loading Qt shared library
    pub fn load_qt_library(&self, _manifest: &PluginManifest) {
        // TODO: Implement Qt plugin loader usage
    }
}

// Configuration for startup plugin function invocation
#[derive(Debug, Deserialize)]
pub struct StartupConfig {
    pub plugin_name: String,
    pub function_name: String,
}

impl StartupConfig {
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let content = fs::read_to_string(path)?;
        let config: StartupConfig = toml::from_str(&content)?;
        Ok(config)
    }
}

// IPC and WebSocket stubs
pub mod ipc {
    use std::sync::mpsc::{Receiver, Sender, channel};
    use std::thread;

    // // Channels for IPC communication
    // lazy_static::lazy_static! {
    //     static ref TO_RUST_SENDER: (Sender<String>, Receiver<String>) = channel();
    //     static ref TO_FRONTEND_SENDER: (Sender<String>, Receiver<String>) = channel();
    // }

    // pub fn send_to_rust(msg: &str) {
    //     let _ = TO_RUST_SENDER.0.send(msg.to_string());
    // }

    // pub fn recv_from_lua() -> Option<String> {
    //     TO_RUST_SENDER.1.try_recv().ok()
    // }

    // pub fn send_to_frontend(msg: &str) {
    //     let _ = TO_FRONTEND_SENDER.0.send(msg.to_string());
    // }

    // pub fn recv_from_rust() -> Option<String> {
    //     TO_FRONTEND_SENDER.1.try_recv().ok()
    // }

    // Placeholder for WebSocket server thread
    pub fn start_websocket_server() {
        thread::spawn(|| {
            // TODO: Implement WebSocket server to communicate with Qt frontend
        });
    }
}

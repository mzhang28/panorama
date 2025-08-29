use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use mlua::{Function as LuaFunction, Lua, LuaOptions, StdLib, Table as LuaTable};
use serde::Deserialize;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginInfo {
    pub base_path: PathBuf,
    pub manifest: PluginManifest,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    pub authors: Vec<String>,
    pub lua_entrypoint: String,
    pub qt_library: Option<String>,
    pub permissions: Vec<String>,
    pub lua_functions: Vec<String>,

    #[serde(default)]
    pub fields: HashMap<String, String>, // field name -> type

    #[serde(default)]
    pub regexes: Vec<String>,

    #[serde(default)]
    pub on: HashMap<EventKey, String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EventKey {
    #[serde(rename = "self_loaded")]
    SelfLoaded,
    #[serde(rename = "plugins_loaded")]
    PluginsLoaded,
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
    pub loaded_plugins: Vec<PluginInfo>,
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
                    let info = PluginInfo {
                        base_path: path.to_path_buf(),
                        manifest,
                    };
                    self.load_lua_entrypoint(&info);
                    self.loaded_plugins.push(info);
                }
            }
        }
        Ok(())
    }

    // Placeholder for loading Lua entrypoint
    pub fn load_lua_entrypoint(&self, info: &PluginInfo) {
        let lua = Lua::new_with(StdLib::ALL_SAFE, LuaOptions::new()).unwrap();
        let prelude = self.create_lua_prelude(&lua).unwrap();
        lua.globals().set("panorama", prelude).unwrap();
        let entrypoint_path = info.base_path.join(&info.manifest.lua_entrypoint);
        let contents = std::fs::read_to_string(entrypoint_path).unwrap();
        let module = lua.load(contents).eval::<LuaTable>().unwrap();

        if let Some(handler_name) = info.manifest.on.get(&EventKey::SelfLoaded) {
            let handler_func = module.get::<LuaFunction>(handler_name.as_str()).unwrap();
            handler_func.call::<()>(()).unwrap();
        }

        println!("Module: {module:?}");
    }

    fn create_lua_prelude(&self, lua: &Lua) -> Result<LuaTable> {
        let table = lua.create_table()?;
        table.set("version", "0.1.0")?;
        Ok(table)
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

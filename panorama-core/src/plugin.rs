use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use mlua::{Function as LuaFunction, Lua, LuaOptions, StdLib, Table as LuaTable};
use serde::Deserialize;
use lazy_static::lazy_static;
use std::sync::Mutex;
use tokio::sync::broadcast;

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
    pub permissions: Vec<Permission>,
    pub lua_functions: Vec<String>,

    #[serde(default)]
    pub fields: HashMap<String, String>, // field name -> type

    #[serde(default)]
    pub regexes: Vec<String>,

    #[serde(default)]
    pub on: HashMap<EventKey, String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    /// This allows running of arbitrary code on the host.
    /// This is required for having Qt components.
    Privileged,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKey {
    SelfLoaded,
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
        create_lua_prelude(lua)
    }
}

// Global IPC sender for sending messages to frontend websocket clients.
lazy_static! {
    static ref BROADCAST_SENDER: Mutex<Option<broadcast::Sender<String>>> = Mutex::new(None);
}

pub mod ipc {
    use super::*;

    pub fn set_sender(sender: broadcast::Sender<String>) {
        let mut guard = BROADCAST_SENDER.lock().unwrap();
        *guard = Some(sender);
    }

    pub fn send_to_frontend(msg: &str) {
        let guard = BROADCAST_SENDER.lock().unwrap();
        if let Some(sender) = guard.as_ref() {
            let _ = sender.send(msg.to_string());
        }
    }
}

/// Create the `panorama` prelude table for Lua scripts. Public so other
/// modules can use the same prelude when invoking Lua functions later.
pub fn create_lua_prelude(lua: &Lua) -> Result<LuaTable> {
    let table = lua.create_table()?;
    table.set("version", env!("CARGO_PKG_VERSION"))?;

    let date_fn = lua.create_function(|lua, _v: ()| {
        let date = chrono::Local::now().format("%Y-%m-%d").to_string();
        lua.create_string(date.as_str())
    })?;
    table.set("date", date_fn)?;

    let query_fn = lua.create_function(|_lua, _v: ()| {
        println!("panorama.query called");
        Ok(())
    })?;
    table.set("query", query_fn)?;

    // panorama.openUrl should send an event to the frontend via websocket.
    let open_fn = lua.create_function(|_lua, url: String| {
        let payload = serde_json::json!({"type": "openUrl", "url": url});
        let s = payload.to_string();
        ipc::send_to_frontend(&s);
        Ok(())
    })?;
    table.set("openUrl", open_fn)?;

    Ok(table)
}

/// Call all plugins' `on_plugins_loaded` handlers (if present).
pub fn call_on_plugins_loaded(plugins: &Vec<PluginInfo>) -> Result<()> {
    for info in plugins {
        if let Some(handler_name) = info.manifest.on.get(&EventKey::PluginsLoaded) {
            let lua = Lua::new_with(StdLib::ALL_SAFE, LuaOptions::new()).unwrap();
            let prelude = create_lua_prelude(&lua)?;
            lua.globals().set("panorama", prelude)?;
            let entrypoint_path = info.base_path.join(&info.manifest.lua_entrypoint);
            let contents = std::fs::read_to_string(entrypoint_path)?;
            let module = lua.load(contents).eval::<LuaTable>()?;
            let handler_func = module.get::<LuaFunction>(handler_name.as_str())?;
            handler_func.call::<()>(()).map_err(|e| anyhow::anyhow!("Lua handler error: {e:?}"))?;
        }
    }
    Ok(())
}

// (old IPC stub removed - new IPC implementation above)

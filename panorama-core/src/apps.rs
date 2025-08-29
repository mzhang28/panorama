use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use mlua::LuaOptions;
use mlua::StdLib;

use crate::db::Dal;

use mlua::Lua;
use once_cell::sync::Lazy;
use serde::Deserialize;
use serde_yaml;
use std::collections::HashMap as StdHashMap;
use std::sync::{Mutex, mpsc};
use tokio::sync::broadcast;

#[derive(Debug, Deserialize)]
struct AppManifest {
    meta: Option<Meta>,
    /// Fields declared as a list of tables. Each table contains `key` and
    /// `type` attributes.
    #[serde(default)]
    fields: Option<Vec<FieldSpec>>,
    /// Optional Lua script specification
    #[serde(default)]
    lua: Option<LuaSpec>,
    /// UI plugins declared as a list of tables with `target` and `path`.
    #[serde(default)]
    ui_plugins: Option<Vec<UiPluginSpec>>,
    /// Permissions the app requests.
    #[serde(default)]
    permissions: Option<Permissions>,
}

#[derive(Debug, Deserialize)]
struct Meta {
    name: String,
    version: String,
    authors: Option<Vec<String>>,
    license: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LuaSpec {
    /// relative path to the entrypoint script
    entrypoint: String,
    /// list of function names the app exposes for RPC-like calls
    #[serde(default)]
    functions: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct FieldSpec {
    key: String,
    #[serde(rename = "type")]
    ty: String,
}

#[derive(Debug, Deserialize)]
struct UiPluginSpec {
    target: String,
    path: String,
}

// Lightweight request type for calling into the Lua worker
type LuaResponse = Result<String, String>;
type LuaRequest = (String, Vec<String>, Option<String>, mpsc::Sender<LuaResponse>);

// Global registry for Lua workers: map app_dir -> request sender
pub(crate) static LUA_WORKERS: Lazy<Mutex<StdHashMap<String, mpsc::Sender<LuaRequest>>>> =
    Lazy::new(|| Mutex::new(StdHashMap::new()));

#[derive(Clone, Debug, serde::Serialize)]
pub(crate) struct UiEvent {
    pub source: String,
    pub url: String,
    pub widget_id: Option<String>,
    pub direction: Option<String>,
}
pub(crate) static UI_EVENTS: Lazy<Mutex<Vec<UiEvent>>> = Lazy::new(|| Mutex::new(Vec::new()));

// Optional broadcast sender for pushing events to connected UI websocket clients.
pub(crate) static UI_BROADCAST: Lazy<Mutex<Option<broadcast::Sender<UiEvent>>>> =
    Lazy::new(|| Mutex::new(None));

pub(crate) fn set_ui_broadcast(sender: broadcast::Sender<UiEvent>) {
    let mut s = UI_BROADCAST.lock().unwrap();
    *s = Some(sender);
}

pub(crate) fn subscribe_ui_broadcast() -> Option<broadcast::Receiver<UiEvent>> {
    let s = UI_BROADCAST.lock().unwrap();
    s.as_ref().map(|tx| tx.subscribe())
}

pub(crate) fn enqueue_ui_event(source: &str, url: &str, widget_id: Option<&str>, direction: Option<&str>) {
    let ev = UiEvent { source: source.to_string(), url: url.to_string(), widget_id: widget_id.map(|s| s.to_string()), direction: direction.map(|s| s.to_string()) };
    // Try to broadcast to websocket clients first; fallback to queueing if no broadcast available
    if let Some(mut tx_opt) = UI_BROADCAST.lock().unwrap().as_ref().cloned() {
        // best-effort send, ignore errors (no listeners)
        let _ = tx_opt.send(ev);
        return;
    }

    let mut q = UI_EVENTS.lock().unwrap();
    q.push(ev);
}

fn spawn_lua_worker(app_key: String, entrypoint: PathBuf, perms: Permissions) {
    let (tx, rx) = mpsc::channel::<LuaRequest>();
    // Store sender in registry
    {
        let mut reg = LUA_WORKERS.lock().unwrap();
        reg.insert(app_key.clone(), tx.clone());
    }

    std::thread::spawn(move || {
        // Create Lua VM in this dedicated thread
        let lua = Lua::new_with(StdLib::ALL_SAFE, LuaOptions::new()).unwrap();

        // Register a global `openUrl(url)` function that enqueues UI events.
        // This closure captures the app_key to indicate the source.
        // We keep a per-worker current_origin that is set per incoming RPC.
        let current_origin = std::sync::Arc::new(std::sync::Mutex::new(None::<String>));
        {
            let ak = app_key.clone();
            let cur = current_origin.clone();
            let open = lua.create_function(move |_, vals: mlua::MultiValue| {
                // expect first arg url (string), optional second arg placement (string)
                let mut iter = vals.into_iter();
                let url = match iter.next() {
                    Some(mlua::Value::String(s)) => s.to_str()?.to_string(),
                    Some(v) => match v.to_string() { Ok(s) => s, Err(_) => "".to_string() },
                    None => "".to_string(),
                };
                let placement = match iter.next() {
                    Some(mlua::Value::String(s)) => Some(s.to_str()?.to_string()),
                    Some(v) => match v.to_string() { Ok(s) => Some(s), Err(_) => None },
                    None => None,
                };
                // current origin is stored in current_origin mutex
                let w = cur.lock().unwrap().clone();
                enqueue_ui_event(&ak, &url, w.as_deref(), placement.as_deref());
                Ok(())
            });
            match open {
                Ok(f) => { let _ = lua.globals().set("openUrl", f); }
                Err(e) => eprintln!("failed to register openUrl: {}", e),
            }
        }

        // Load entrypoint
        if let Ok(src) = std::fs::read_to_string(&entrypoint) {
            if let Err(e) = lua.load(&src).exec() {
                eprintln!("lua entrypoint error ({}): {}", entrypoint.display(), e);
            }
        } else {
            eprintln!("failed to read lua entrypoint: {}", entrypoint.display());
        }

        // Event loop: handle call requests
        while let Ok((func_name, args, origin_opt, resp_tx)) = rx.recv() {
            // set current origin for this invocation
            {
                let mut cur = current_origin.lock().unwrap();
                *cur = origin_opt.clone();
            }
            let result: LuaResponse = match lua.globals().get::<mlua::Function>(func_name.clone()) {
                Ok(func) => {
                    // For POC we pass arguments as a single concatenated string.
                    let joined = args.join("\n");
                    let call_res: Result<String, mlua::Error> = func.call(joined);
                    match call_res {
                        Ok(s) => Ok(s),
                        Err(e) => Err(format!("lua call error: {}", e)),
                    }
                }
                Err(e) => Err(format!("unknown lua function {}: {}", func_name, e)),
            };

            let _ = resp_tx.send(result);
            // clear current origin
            {
                let mut cur = current_origin.lock().unwrap();
                *cur = None;
            }
        }
    });
}

#[derive(Debug, Deserialize, Default, Clone)]
struct Permissions {
    /// Allow loading native Qt widgets from this app
    #[serde(default)]
    allow_native: bool,
    /// Allow spawning a background web server
    #[serde(default)]
    allow_spawn_server: bool,
    /// Allow broader OS access from scripts
    #[serde(default)]
    allow_os_access: bool,
}

fn default_app_search_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    // Project-local apps folder
    paths.push(PathBuf::from("apps"));
    // Optional additional paths via env var, colon-separated
    if let Ok(s) = env::var("PANORAMA_APPS_PATH") {
        for part in s.split(':') {
            if !part.is_empty() {
                paths.push(PathBuf::from(part));
            }
        }
    }
    paths
}

fn current_target_triple() -> String {
    // Build a simple rust-like target triple for the current binary.
    // This POC handles the common desktop targets.
    let arch = if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else if cfg!(target_arch = "arm") {
        "arm"
    } else {
        "unknown"
    };

    let os_part = if cfg!(target_os = "macos") {
        "apple-darwin"
    } else if cfg!(target_os = "linux") {
        if cfg!(target_env = "musl") {
            "unknown-linux-musl"
        } else {
            "unknown-linux-gnu"
        }
    } else if cfg!(target_os = "windows") {
        "pc-windows-msvc"
    } else {
        "unknown"
    };

    format!("{}-{}", arch, os_part)
}

fn try_load_lua_sandbox(script_path: &Path, perms: &Permissions) {
    // Create a Lua VM. We will zero-out dangerous globals (like `os`) unless
    // the app explicitly asks for OS access. This is a lightweight sandboxing
    // strategy for the POC; future work should use proper seccomp-like
    // restrictions where available.
    let lua = Lua::new();
    if !perms.allow_os_access {
        // Remove access to OS by setting the global `os` to nil.
        let _ = lua.load("os = nil").exec();
    }

    if let Ok(src) = std::fs::read_to_string(script_path) {
        if let Err(e) = lua.load(&src).exec() {
            eprintln!("lua script exec error ({}): {}", script_path.display(), e);
        }
    } else {
        eprintln!("failed to read lua script: {}", script_path.display());
    }
}

fn discover_manifests(search_paths: &[PathBuf]) -> Result<Vec<(PathBuf, AppManifest)>> {
    let mut out = Vec::new();
    for apps_dir in search_paths.iter() {
        if apps_dir.exists() && apps_dir.is_dir() {
            for entry in fs::read_dir(apps_dir)? {
                if let Ok(entry) = entry {
                    let path = entry.path();
                    if path.is_dir() {
                        let manifest_path = path.join("manifest.yaml");
                        if manifest_path.exists() {
                            if let Ok(s) = fs::read_to_string(&manifest_path) {
                                match serde_yaml::from_str::<AppManifest>(&s) {
                                    Ok(manifest) => out.push((path, manifest)),
                                    Err(e) => eprintln!(
                                        "failed to parse manifest {:?}: {}",
                                        manifest_path, e
                                    ),
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(out)
}

fn process_manifest(
    path: &Path,
    manifest: AppManifest,
    desired: &mut HashMap<String, String>,
) -> Result<()> {
    if let Some(field_list) = manifest.fields {
        for f in field_list.into_iter() {
            desired.insert(f.key, f.ty);
        }
    }

    if let Some(lua_spec) = manifest.lua {
        let script_path = path.join(&lua_spec.entrypoint);
        let perms = manifest.permissions.unwrap_or_default();
        let app_key = path.to_string_lossy().to_string();
        spawn_lua_worker(app_key, script_path, perms);
    }

    if let Some(ui_list) = manifest.ui_plugins {
        let triple = current_target_triple();
        if let Some(spec) = ui_list.into_iter().find(|s| s.target == triple) {
            let plugin_path = path.join(&spec.path);
            if plugin_path.exists() {
                eprintln!(
                    "discovered ui plugin for {}: {}",
                    triple,
                    plugin_path.display()
                );
            } else {
                eprintln!(
                    "ui plugin for {} not found: {}",
                    triple,
                    plugin_path.display()
                );
            }
        }
    }

    Ok(())
}

/// Install apps discovered under the top-level `apps/` directory.
///
/// This replaces the previous hardcoded list and allows apps to declare fields
/// via `apps/<app>/manifest.json`. Each manifest describes the fields the app
/// wants to create in the dynamic schema; missing keys are created using the
/// same `Dal::create_table` helper used previously.
pub async fn install_default_apps(dal: Dal) -> Result<()> {
    let mut desired: HashMap<String, String> = HashMap::new();

    let search_paths = default_app_search_paths();
    let manifests = discover_manifests(&search_paths)?;
    for (path, manifest) in manifests.into_iter() {
        process_manifest(&path, manifest, &mut desired)?;
    }

    // If no manifests found, fall back to the legacy journal + file keys
    if desired.is_empty() {
        desired.insert("journal/title".to_string(), "TEXT".to_string());
        desired.insert("journal/content".to_string(), "TEXT".to_string());
        desired.insert("file/sha256".to_string(), "TEXT".to_string());
        desired.insert("file/name".to_string(), "TEXT".to_string());
        desired.insert("file/size".to_string(), "INTEGER".to_string());
    }

    // Determine which keys are missing from _panorama_schema_columns
    let mut missing: HashMap<String, String> = HashMap::new();
    for (key, ty) in desired.into_iter() {
        if !dal.has_schema_key(&key).await? {
            missing.insert(key, ty);
        }
    }

    if !missing.is_empty() {
        // Create a table named `plugin_fields` which groups fields from multiple
        // apps. For backwards-compatibility the previous code used `journal` as
        // the table name; here we reuse that behavior when the keys are
        // journal/*, otherwise create a generic `plugin_fields` table.
        let table_name = if missing.keys().any(|k| k.starts_with("journal/")) {
            "journal"
        } else {
            "plugin_fields"
        };

        dal.create_table(table_name, missing).await?;
    }

    Ok(())
}

use std::collections::HashMap;
use std::sync::Arc;

use panorama_core::capabilities::CapabilityGrants;
use panorama_core::plugin::{
  HttpEndpoint, HttpRequest, HttpResponse, Plugin, PluginError, UiComponent,
};
use panorama_core::schema::Schema;
use tokio::sync::RwLock;

use crate::object_store::ObjectStorage;
use crate::panoapp::PanoAppPackage;
use crate::plugin_runtime::RuntimeContext;
use crate::schema_registry::SchemaRegistry;
use crate::storage::NodeStorage;
use crate::wasm_runtime;

#[derive(Clone)]
pub struct LoadedPluginInfo {
  pub info: PluginInfo,
  pub schemas: Vec<Schema>,
  pub endpoints: Vec<HttpEndpoint>,
  pub ui_components: Vec<UiComponent>,
  pub capabilities: CapabilityGrants,
  /// Whether this plugin was loaded from a .panoapp (WASM-based)
  pub is_wasm: bool,
}

#[derive(Debug, Clone)]
pub struct PluginInfo {
  pub id: String,
  pub name: String,
  pub version: String,
  pub description: String,
}

/// Internal storage for WASM-based plugins
#[derive(Clone)]
struct WasmPlugin {
  info: LoadedPluginInfo,
  wasm_bytes: Vec<u8>,
  /// Pre-compiled WASM module + engine — shared across requests
  compiled: Arc<wasmtime::Module>,
  /// Engine that owns the module (must outlive it)
  _engine: Arc<wasmtime::Engine>,
  ui_files: HashMap<String, Vec<u8>>,
}

pub struct PluginLoader {
  /// Native plugin instances (trait objects)
  instances: RwLock<HashMap<String, Arc<dyn Plugin>>>,
  /// WASM-based plugins (.panoapp loaded)
  wasm_plugins: RwLock<HashMap<String, WasmPlugin>>,
  /// All plugin metadata
  infos: RwLock<HashMap<String, LoadedPluginInfo>>,
  storage: NodeStorage,
  schema_registry: SchemaRegistry,
  object_storage: ObjectStorage,
  wasm_engine: Arc<wasmtime::Engine>,
}

impl PluginLoader {
  pub fn new(
    storage: NodeStorage,
    schema_registry: SchemaRegistry,
    object_storage: ObjectStorage,
  ) -> Self {
    let mut config = wasmtime::Config::new();
    config.async_support(true);
    let wasm_engine =
      Arc::new(wasmtime::Engine::new(&config).expect("Failed to initialize WASM engine"));

    Self {
      instances: RwLock::new(HashMap::new()),
      wasm_plugins: RwLock::new(HashMap::new()),
      infos: RwLock::new(HashMap::new()),
      storage,
      schema_registry,
      object_storage,
      wasm_engine,
    }
  }

  /// Load a native plugin (trait object).
  pub async fn load(&self, plugin: Arc<dyn Plugin>) -> Result<LoadedPluginInfo, String> {
    let plugin_id = plugin.id().to_string();
    let caps = plugin.required_capabilities();
    let ctx = self.create_context(&plugin_id, caps.clone());

    plugin
      .initialize(&ctx)
      .await
      .map_err(|e| format!("Failed to init plugin '{}': {}", plugin_id, e))?;

    let mut registered_schemas = Vec::new();
    for schema in plugin.schemas() {
      registered_schemas.push(self.schema_registry.register(schema));
    }

    let info = LoadedPluginInfo {
      info: PluginInfo {
        id: plugin.id().to_string(),
        name: plugin.name().to_string(),
        version: plugin.version().to_string(),
        description: plugin.description().to_string(),
      },
      schemas: registered_schemas,
      endpoints: plugin.http_endpoints(),
      ui_components: plugin.ui_components(),
      capabilities: caps,
      is_wasm: false,
    };

    self
      .infos
      .write()
      .await
      .insert(plugin_id.clone(), info.clone());
    self.instances.write().await.insert(plugin_id, plugin);

    tracing::info!(
      "Loaded native plugin: {} v{}",
      info.info.name,
      info.info.version
    );
    Ok(info)
  }

  /// Load a plugin from a .panoapp file (ZIP archive).
  /// This is the primary distribution method for third-party plugins.
  pub async fn load_from_panoapp(
    &self,
    package: PanoAppPackage,
  ) -> Result<LoadedPluginInfo, String> {
    let m = &package.manifest;
    let plugin_id = m.id.clone();

    // Register schemas from manifest
    let mut registered_schemas = Vec::new();
    for schema in package.to_schemas() {
      registered_schemas.push(self.schema_registry.register(schema));
    }

    let info = LoadedPluginInfo {
      info: PluginInfo {
        id: plugin_id.clone(),
        name: m.name.clone(),
        version: m.version.clone(),
        description: m.description.clone(),
      },
      schemas: registered_schemas,
      endpoints: m.http_endpoints.clone(),
      ui_components: m.ui_components.clone(),
      capabilities: m.capabilities.clone(),
      is_wasm: package.wasm_bytes.is_some(),
    };

    // If WASM module is present, compile it now and store for execution
    if let Some(wasm_bytes) = &package.wasm_bytes {
      let compiled = wasmtime::Module::from_binary(&self.wasm_engine, wasm_bytes)
        .map_err(|e| format!("wasm compile: {}", e))?;
      tracing::info!(plugin = %plugin_id, "Pre-compiled WASM module");

      self.wasm_plugins.write().await.insert(
        plugin_id.clone(),
        WasmPlugin {
          info: info.clone(),
          wasm_bytes: wasm_bytes.clone(),
          compiled: Arc::new(compiled),
          _engine: self.wasm_engine.clone(),
          ui_files: package.ui_files.clone(),
        },
      );
    }

    self
      .infos
      .write()
      .await
      .insert(plugin_id.clone(), info.clone());
    tracing::info!(
      "Loaded .panoapp plugin: {} v{} ({})",
      m.name,
      m.version,
      plugin_id
    );
    Ok(info)
  }

  pub fn create_context(&self, plugin_id: &str, caps: CapabilityGrants) -> RuntimeContext {
    RuntimeContext::new(
      plugin_id,
      self.storage.clone(),
      self.schema_registry.clone(),
      self.object_storage.clone(),
      caps,
    )
  }

  pub async fn get(&self, plugin_id: &str) -> Option<LoadedPluginInfo> {
    self.infos.read().await.get(plugin_id).cloned()
  }

  pub async fn list_all(&self) -> Vec<LoadedPluginInfo> {
    self.infos.read().await.values().cloned().collect()
  }

  /// Get a UI file from a loaded plugin.
  /// Returns the file bytes and MIME type if found.
  pub async fn get_ui_file(&self, plugin_id: &str, path: &str) -> Option<(Vec<u8>, String)> {
    let wasm = self.wasm_plugins.read().await;
    if let Some(wp) = wasm.get(plugin_id) {
      if let Some(data) = wp.ui_files.get(path) {
        let mime = mime_for_path(path);
        return Some((data.clone(), mime.to_string()));
      }
    }
    None
  }

  /// List all available static UI file paths for a loaded plugin.
  pub async fn list_ui_files(&self, plugin_id: &str) -> Option<Vec<String>> {
    let wasm = self.wasm_plugins.read().await;
    if let Some(wp) = wasm.get(plugin_id) {
      let mut paths: Vec<String> = wp.ui_files.keys().cloned().collect();
      paths.sort();
      return Some(paths);
    }
    None
  }

  /// Dispatch an HTTP request to a plugin.
  /// For native plugins, calls the trait method directly.
  /// For WASM plugins, executes via the WASM runtime.
  pub async fn dispatch_http(
    &self,
    plugin_id: &str,
    endpoint: &str,
    request: HttpRequest,
  ) -> Result<HttpResponse, PluginError> {
    // Check if it's a WASM plugin first
    let wasm_plugin = { self.wasm_plugins.read().await.get(plugin_id).cloned() };

    if let Some(wp) = wasm_plugin {
      // Execute via WASM runtime on a blocking thread — host functions
      // use pollster::block_on which would otherwise starve tokio workers.
      let engine = wp._engine.clone();
      let compiled = wp.compiled.clone();
      let plugin_id = wp.info.info.id.clone();
      let capabilities = wp.info.capabilities.clone();
      let storage = self.storage.clone();
      let schema_registry = self.schema_registry.clone();
      let object_storage = self.object_storage.clone();
      let endpoint = endpoint.to_string();
      let result = tokio::task::spawn_blocking(move || {
        pollster::block_on(wasm_runtime::execute_wasm_handler(
          &engine,
          &compiled,
          &endpoint,
          &request,
          &plugin_id,
          &capabilities,
          &storage,
          &schema_registry,
          &object_storage,
        ))
      })
      .await
      .map_err(|e| PluginError::internal(format!("wasm panic: {}", e)))?;
      return result;
    }

    // Otherwise, try native plugin
    let instance = { self.instances.read().await.get(plugin_id).cloned() };

    match instance {
      Some(plugin) => {
        let ctx = {
          let infos = self.infos.read().await;
          infos
            .get(plugin_id)
            .map(|info| self.create_context(plugin_id, info.capabilities.clone()))
            .ok_or_else(|| PluginError::not_found("Plugin info not found"))?
        };
        plugin.handle_http_request(endpoint, request, &ctx).await
      }
      None => Err(PluginError::not_found(&format!(
        "Plugin '{}' not found",
        plugin_id
      ))),
    }
  }
}

/// Map a file path to its MIME type based on extension.
fn mime_for_path(path: &str) -> &'static str {
  let ext = path.rsplit('.').next().unwrap_or("");
  match ext {
    "js" => "application/javascript",
    "mjs" => "application/javascript",
    "css" => "text/css",
    "html" => "text/html",
    "json" => "application/json",
    "map" => "application/json",
    "svg" => "image/svg+xml",
    "png" => "image/png",
    "jpg" | "jpeg" => "image/jpeg",
    "gif" => "image/gif",
    "woff" => "font/woff",
    "woff2" => "font/woff2",
    _ => "application/octet-stream",
  }
}

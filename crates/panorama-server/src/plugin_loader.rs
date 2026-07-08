use std::collections::HashMap;
use std::sync::Arc;

use panorama_core::capabilities::CapabilityGrants;
use panorama_core::plugin::{
  HttpEndpoint, HttpRequest, HttpResponse, Plugin, PluginError, UiComponent,
};
use panorama_core::schema::Schema;
use tokio::sync::RwLock;

use crate::backtrace::BacktraceStore;
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
  /// Pre-compiled WASM module — shared across requests
  compiled: Arc<wasmtime::Module>,
  /// Engine that owns the module (must outlive it)
  _engine: Arc<wasmtime::Engine>,
  ui_files: HashMap<String, Vec<u8>>,
  /// Pre-linked InstancePre — linker + host functions done once at load time (§1.1)
  instance_pre: wasmtime::InstancePre<crate::wasm_runtime::WasiCtx>,
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
  /// Shared backtrace store for all WASM plugins.
  pub backtrace_store: Arc<BacktraceStore>,
}

impl PluginLoader {
  pub fn new(
    storage: NodeStorage,
    schema_registry: SchemaRegistry,
    object_storage: ObjectStorage,
  ) -> Self {
    let mut config = wasmtime::Config::new();
    config.async_support(true);
    // Generate address maps so wasm byte offsets can be resolved to
    // source file:line via DWARF (used for backtrace symbolication).
    config.generate_address_map(true);
    // Parse DWARF debug info from wasm custom sections — required for
    // FrameInfo::symbols() to return file:line data.
    config.wasm_backtrace_details(wasmtime::WasmBacktraceDetails::Enable);
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
      backtrace_store: Arc::new(BacktraceStore::new()),
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
      registered_schemas.push(self.schema_registry.register(schema)?);
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
      registered_schemas.push(self.schema_registry.register(schema)?);
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

      // Pre-link: build linker, register WASI + all host functions,
      // and cache an InstancePre so per-request instantiation is nearly free (§1.1).
      let instance_pre = crate::wasm_runtime::create_prelinked_instance(
        &self.wasm_engine,
        &compiled,
        &info.info.id,
        &info.capabilities,
        &self.storage,
        &self.schema_registry,
        &self.object_storage,
        self.backtrace_store.clone(),
      )
      .map_err(|e| format!("wasm pre-link: {}", e))?;
      tracing::info!(plugin = %plugin_id, "Cached pre-linked InstancePre");

      self.wasm_plugins.write().await.insert(
        plugin_id.clone(),
        WasmPlugin {
          info: info.clone(),
          wasm_bytes: wasm_bytes.clone(),
          compiled: Arc::new(compiled),
          _engine: self.wasm_engine.clone(),
          ui_files: package.ui_files.clone(),
          instance_pre,
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
      // The InstancePre was pre-linked at load time (§1.1); only the WASI
      // stdin/stdout store is created per-request.
      let engine = wp._engine.clone();
      let instance_pre = wp.instance_pre.clone();
      let endpoint = endpoint.to_string();
      let bt_store = self.backtrace_store.clone();
      let bt_store_for_closure = bt_store.clone();
      let result = tokio::task::spawn_blocking(move || {
        pollster::block_on(wasm_runtime::execute_wasm_handler(
          &engine,
          &instance_pre,
          &endpoint,
          &request,
          &bt_store_for_closure,
        ))
      })
      .await
      .map_err(|e| PluginError::internal(format!("wasm spawn_blocking: {}", e)))?;

      // If the response carries an error with a backtrace_id, resolve it
      // against the store and convert it to a PluginError so the Sentry
      // attachment path in api.rs is triggered.
      //
      // Recoverable wasm errors arrive as Ok(HttpResponse) with status >= 400
      // and a JSON body containing the structured error — the Err arm only
      // handles traps (wasmtime errors).  We need to handle both.
      match result {
        Err(mut err) => {
          if let Some(id) = err.backtrace_id {
            if let Some(ctx) = bt_store.take(id) {
              err.trap_frames = Some(
                ctx
                  .frames
                  .iter()
                  .map(|f| panorama_core::plugin::TrapFrame::from(f))
                  .collect(),
              );
            }
          }
          return Err(err);
        }
        Ok(resp) => {
          // Wasm errors arrive as Ok(HttpResponse) with status >= 400.
          // Convert to Err so api.rs logs + reports to Sentry.
          if resp.status >= 400 {
            let body: serde_json::Value = serde_json::from_slice(&resp.body).unwrap_or_default();
            let err_msg = body
              .get("error")
              .and_then(|v| v.as_str())
              .unwrap_or("wasm error")
              .to_string();
            let err_code = body
              .get("code")
              .and_then(|v| v.as_str())
              .unwrap_or("WASM_ERROR")
              .to_string();

            // If the error carries a backtrace_id, resolve frames.
            let backtrace_id = body.get("backtrace_id").and_then(|v| v.as_u64());
            let trap_frames = backtrace_id.and_then(|id| bt_store.take(id)).map(|ctx| {
              ctx
                .frames
                .iter()
                .map(|f| panorama_core::plugin::TrapFrame::from(f))
                .collect()
            });

            // Extract the guest-side source location if present.
            let location = body.get("location").and_then(|loc| {
              let file = loc.get("file")?.as_str()?.to_string();
              let line = loc.get("line")?.as_u64()? as u32;
              let column = loc.get("column").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
              Some(panorama_core::plugin::SourceLocation { file, line, column })
            });

            return Err(PluginError {
              code: err_code,
              message: err_msg,
              status: resp.status,
              location,
              backtrace_id,
              trap_frames,
            });
          }
          return Ok(resp);
        }
      }
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

  /// Execute a reactor action via WASM.
  ///
  /// Reuses the existing `execute_wasm_handler` infrastructure — the reactor
  /// input is serialized as an HTTP-like request body and dispatched to the
  /// WASM module's `_start` entry point. The module reads stdin, processes,
  /// and writes the result to stdout.
  ///
  /// Returns `Ok(None)` if the plugin is not a WASM plugin. Returns the
  /// reactor's output on success, or an error string on failure.
  pub async fn execute_reactor_action(
    &self,
    plugin_id: &str,
    function_name: &str,
    input: &panorama_core::reactor::ReactorActionInput,
  ) -> Result<Option<panorama_core::reactor::ReactorActionOutput>, String> {
    use panorama_core::reactor::ReactorActionOutput;

    let wasm_plugin = { self.wasm_plugins.read().await.get(plugin_id).cloned() };

    let wp = match wasm_plugin {
      Some(wp) => wp,
      None => return Ok(None),
    };

    // Serialize the reactor input as the request body
    let body_json =
      serde_json::to_vec(input).map_err(|e| format!("Failed to serialize reactor input: {}", e))?;

    // Build an HTTP-like request that carries the reactor payload
    let request = panorama_core::plugin::HttpRequest {
      method: "POST".into(),
      path: format!("__reactor__/{}", function_name),
      query_params: std::collections::HashMap::new(),
      headers: std::collections::HashMap::from([(
        "Content-Type".into(),
        "application/json".into(),
      )]),
      body: Some(bytes::Bytes::from(body_json)),
    };

    // Clone everything needed by the 'static spawn_blocking closure.
    // The InstancePre was pre-linked at load time (§1.1); no linker work here.
    let engine = wp._engine.clone();
    let instance_pre = wp.instance_pre.clone();
    let path = request.path.clone();
    let bt_store = self.backtrace_store.clone();

    // Execute via the same WASM runtime used for HTTP handlers.
    // This gives reactors access to all host functions (create_nodes,
    // query, etc.) subject to the plugin's capability grants.
    let result = tokio::task::spawn_blocking(move || {
      pollster::block_on(crate::wasm_runtime::execute_wasm_handler(
        &engine,
        &instance_pre,
        &path,
        &request,
        &bt_store,
      ))
    })
    .await
    .map_err(|e| format!("WASM spawn_blocking panic: {}", e))?;

    match result {
      Ok(response) => {
        if response.body.is_empty() {
          return Ok(None);
        }
        let output: ReactorActionOutput = serde_json::from_slice(&response.body).map_err(|e| {
          format!(
            "Failed to parse reactor output: {} (body: {})",
            e,
            String::from_utf8_lossy(&response.body[..response.body.len().min(200)])
          )
        })?;
        Ok(Some(output))
      }
      Err(e) => Err(format!("WASM execution error: {}", e)),
    }
  }
}

// ── Plugin load state tracking ──────────────────────────────────────────────

/// Tracks the progress of plugin loading at startup.
///
/// This is exposed via `GET /api/plugins/status` so the frontend can
/// long-poll until all plugins are ready, rather than showing a blank
/// screen while the server boots.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PluginLoadState {
  pub phase: LoadPhase,
  pub total: u32,
  pub loaded: u32,
  pub failed: u32,
  pub plugins: Vec<PluginStatusEntry>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LoadPhase {
  Scanning,
  Loading,
  Ready,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PluginStatusEntry {
  pub id: String,
  pub name: String,
  pub version: String,
  pub status: String, // "pending" | "loading" | "loaded" | "failed"
  #[serde(skip_serializing_if = "Option::is_none")]
  pub error: Option<String>,
}

impl PluginLoadState {
  pub fn new() -> Self {
    Self {
      phase: LoadPhase::Scanning,
      total: 0,
      loaded: 0,
      failed: 0,
      plugins: Vec::new(),
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

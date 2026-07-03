use std::collections::HashMap;
use std::sync::Arc;

use panorama_core::capabilities::CapabilityGrants;
use panorama_core::plugin::{
    HttpEndpoint, HttpRequest, HttpResponse, Plugin, PluginError, UiComponent,
};
use panorama_core::schema::Schema;
use tokio::sync::RwLock;

use crate::panoapp::PanoAppPackage;
use crate::plugin_runtime::RuntimeContext;
use crate::storage::NodeStorage;
use crate::schema_registry::SchemaRegistry;
use crate::object_store::ObjectStorage;
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
}

impl PluginLoader {
    pub fn new(
        storage: NodeStorage,
        schema_registry: SchemaRegistry,
        object_storage: ObjectStorage,
    ) -> Self {
        Self {
            instances: RwLock::new(HashMap::new()),
            wasm_plugins: RwLock::new(HashMap::new()),
            infos: RwLock::new(HashMap::new()),
            storage,
            schema_registry,
            object_storage,
        }
    }

    /// Load a native plugin (trait object).
    pub async fn load(&self, plugin: Arc<dyn Plugin>) -> Result<LoadedPluginInfo, String> {
        let plugin_id = plugin.id().to_string();
        let caps = plugin.required_capabilities();
        let ctx = self.create_context(&plugin_id, caps.clone());

        plugin.initialize(&ctx).await.map_err(|e| format!("Failed to init plugin '{}': {}", plugin_id, e))?;

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

        self.infos.write().await.insert(plugin_id.clone(), info.clone());
        self.instances.write().await.insert(plugin_id, plugin);

        tracing::info!("Loaded native plugin: {} v{}", info.info.name, info.info.version);
        Ok(info)
    }

    /// Load a plugin from a .panoapp file (ZIP archive).
    /// This is the primary distribution method for third-party plugins.
    pub async fn load_from_panoapp(&self, package: PanoAppPackage) -> Result<LoadedPluginInfo, String> {
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

        // If WASM module is present, store it for execution
        if let Some(wasm_bytes) = &package.wasm_bytes {
            self.wasm_plugins.write().await.insert(plugin_id.clone(), WasmPlugin {
                info: info.clone(),
                wasm_bytes: wasm_bytes.clone(),
            });
        }

        self.infos.write().await.insert(plugin_id.clone(), info.clone());
        tracing::info!("Loaded .panoapp plugin: {} v{} ({})", m.name, m.version, plugin_id);
        Ok(info)
    }

    pub fn create_context(
        &self,
        plugin_id: &str,
        caps: CapabilityGrants,
    ) -> RuntimeContext {
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

    /// Get UI files for a WASM-loaded plugin
    pub async fn get_ui_file(&self, plugin_id: &str, path: &str) -> Option<Vec<u8>> {
        None // UI files are served from the .panoapp; stored separately
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
        let wasm_plugin = {
            self.wasm_plugins.read().await.get(plugin_id).cloned()
        };

        if let Some(wp) = wasm_plugin {
            // Execute via WASM runtime
            return tokio::task::spawn_blocking({
                let wasm_bytes = wp.wasm_bytes.clone();
                let endpoint = endpoint.to_string();
                let storage = self.storage.clone();
                let object_storage = self.object_storage.clone();
                move || {
                    wasm_runtime::execute_wasm_handler(
                        &wasm_bytes,
                        &endpoint,
                        &request,
                        &storage,
                        &object_storage,
                    )
                }
            })
            .await
            .map_err(|e| PluginError::internal(format!("WASM task join: {}", e)))?
        }

        // Otherwise, try native plugin
        let instance = {
            self.instances.read().await.get(plugin_id).cloned()
        };

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

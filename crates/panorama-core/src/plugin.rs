use async_trait::async_trait;
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::capabilities::CapabilityGrants;
use crate::schema::Schema;
use crate::types::{FieldValue, Node, ObjectRef};

/// The core Plugin trait that ALL third-party apps must implement.
/// This is the ONLY interface apps use to interact with the Panorama platform.
/// Apps MUST NOT depend on any internal platform APIs.
#[async_trait]
pub trait Plugin: Send + Sync {
    /// Unique identifier for this plugin
    fn id(&self) -> &str;

    /// Human-readable name
    fn name(&self) -> &str;

    /// Plugin version
    fn version(&self) -> &str;

    /// Description of what this plugin does
    fn description(&self) -> &str;

    /// Schemas this plugin defines.
    /// These will be registered in the platform's schema registry.
    fn schemas(&self) -> Vec<Schema> {
        vec![]
    }

    /// HTTP endpoints this plugin serves.
    /// Each endpoint gets mounted at `/plugin/{plugin_id}/{path}`
    fn http_endpoints(&self) -> Vec<HttpEndpoint> {
        vec![]
    }

    /// Background tasks that should run continuously.
    /// The platform will start these when the plugin is loaded.
    fn background_tasks(&self) -> Vec<BackgroundTask> {
        vec![]
    }

    /// UI components this plugin provides.
    /// These are rendered in the frontend.
    fn ui_components(&self) -> Vec<UiComponent> {
        vec![]
    }

    /// Required capabilities for this plugin to function
    fn required_capabilities(&self) -> CapabilityGrants {
        CapabilityGrants::default()
    }

    /// Called when the plugin is initialized.
    /// The context provides access to the platform APIs.
    async fn initialize(&self, _ctx: &dyn PluginContext) -> Result<(), PluginError> {
        Ok(())
    }

    /// Handle an HTTP request to one of this plugin's endpoints
    async fn handle_http_request(
        &self,
        _endpoint: &str,
        _request: HttpRequest,
        _ctx: &dyn PluginContext,
    ) -> Result<HttpResponse, PluginError> {
        Err(PluginError::not_found("No HTTP handler implemented"))
    }

    /// Called to execute a background task
    async fn run_background_task(
        &self,
        _task_name: &str,
        _ctx: &dyn PluginContext,
    ) -> Result<(), PluginError> {
        Err(PluginError::not_found("No background task implemented"))
    }
}

/// Context provided to plugins for interacting with the platform.
/// This is the ONLY API surface plugins have access to.
#[async_trait]
pub trait PluginContext: Send + Sync {
    // -- Node operations --

    /// Create multiple nodes in a batch operation.
    async fn create_nodes(&self, nodes: Vec<Node>) -> Result<Vec<Node>, PluginError>;

    /// Create a single node (special case of create_nodes).
    async fn create_node(&self, node: Node) -> Result<Node, PluginError> {
        let mut results = self.create_nodes(vec![node]).await?;
        if results.is_empty() {
            Err(PluginError::internal("create_nodes returned empty vector".into()))
        } else {
            Ok(results.remove(0))
        }
    }

    /// Get a node by ID
    async fn get_node(&self, id: Uuid) -> Result<Option<Node>, PluginError>;

    /// Update a node's fields
    async fn update_node(
        &self,
        id: Uuid,
        fields: HashMap<String, FieldValue>,
    ) -> Result<Node, PluginError>;

    /// Delete a node
    async fn delete_node(&self, id: Uuid) -> Result<(), PluginError>;

    /// Execute a Panorama Query Language query.
    /// Returns rows as JSON objects with columns named by their expression aliases.
    async fn query(&self, _query_string: &str) -> Result<Vec<serde_json::Value>, PluginError> {
        Err(PluginError::internal(
            "query() not implemented for this context".into(),
        ))
    }

    // -- Schema operations --

    /// Register a schema with the platform
    async fn register_schema(&self, schema: Schema) -> Result<Schema, PluginError>;

    /// Get a schema by its node ID
    async fn get_schema(&self, schema_node_id: Uuid) -> Result<Option<Schema>, PluginError>;

    // -- Object storage --

    /// Store an object in the platform's object storage
    async fn put_object(
        &self,
        bucket: &str,
        key: &str,
        data: Bytes,
        mime_type: &str,
    ) -> Result<ObjectRef, PluginError>;

    /// Get an object from the platform's object storage
    async fn get_object(&self, bucket: &str, key: &str)
        -> Result<Option<ObjectData>, PluginError>;

    /// Delete an object
    async fn delete_object(&self, bucket: &str, key: &str) -> Result<(), PluginError>;

    /// List objects in a bucket
    async fn list_objects(
        &self,
        bucket: &str,
        prefix: Option<&str>,
    ) -> Result<Vec<ObjectRef>, PluginError>;

    // -- Utility --

    /// Get the plugin's own ID (set by the platform)
    fn plugin_id(&self) -> &str;

    /// Get the base URL path for this plugin's HTTP endpoints
    fn base_path(&self) -> String;

    /// Log a message through the platform
    async fn log(&self, level: LogLevel, message: &str);
}

/// HTTP endpoint definition for a plugin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpEndpoint {
    /// HTTP method
    pub method: HttpMethod,
    /// Path relative to the plugin's base path (e.g., "/webhook")
    pub path: String,
    /// Description of what this endpoint does
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HttpMethod {
    GET,
    POST,
    PUT,
    DELETE,
    PATCH,
}

/// HTTP request passed to a plugin handler
#[derive(Debug, Clone)]
pub struct HttpRequest {
    pub method: String,
    pub path: String,
    pub query_params: HashMap<String, String>,
    pub headers: HashMap<String, String>,
    pub body: Option<Bytes>,
}

/// HTTP response from a plugin handler
#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: Bytes,
}

impl HttpResponse {
    pub fn ok(body: impl Into<Bytes>) -> Self {
        Self {
            status: 200,
            headers: HashMap::new(),
            body: body.into(),
        }
    }

    pub fn json(data: &impl Serialize) -> Result<Self, PluginError> {
        let body = serde_json::to_vec(data)
            .map_err(|e| PluginError::internal(format!("JSON serialize: {}", e)))?;
        let mut headers = HashMap::new();
        headers.insert("Content-Type".into(), "application/json".into());
        Ok(Self {
            status: 200,
            headers,
            body: Bytes::from(body),
        })
    }

    pub fn status(mut self, status: u16) -> Self {
        self.status = status;
        self
    }

    pub fn not_found() -> Self {
        Self {
            status: 404,
            headers: HashMap::new(),
            body: Bytes::from("Not Found"),
        }
    }
}

/// Background task definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackgroundTask {
    /// Unique name for this task
    pub name: String,
    /// How often to run (in seconds), None = runs continuously
    pub interval_seconds: Option<u64>,
    /// Description of what this task does
    pub description: String,
}

/// UI component provided by a plugin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiComponent {
    /// Unique component ID
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Where this component should be mounted
    pub mount_point: UiMountPoint,
    /// Path to the component's JS bundle (relative to plugin dir)
    pub bundle_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UiMountPoint {
    /// Main app page (full page component)
    MainPage,
    /// Sidebar widget
    Sidebar,
    /// Dashboard widget
    Dashboard,
    /// Custom mount point name
    Custom(String),
}

/// Data returned from object storage
#[derive(Debug, Clone)]
pub struct ObjectData {
    pub data: Bytes,
    pub mime_type: String,
    pub size: u64,
}

/// Error type for plugin operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginError {
    pub code: String,
    pub message: String,
    pub status: u16,
}

impl PluginError {
    pub fn not_found(message: &str) -> Self {
        Self {
            code: "NOT_FOUND".into(),
            message: message.into(),
            status: 404,
        }
    }

    pub fn permission_denied(message: &str) -> Self {
        Self {
            code: "PERMISSION_DENIED".into(),
            message: message.into(),
            status: 403,
        }
    }

    pub fn internal(message: String) -> Self {
        Self {
            code: "INTERNAL_ERROR".into(),
            message,
            status: 500,
        }
    }

    pub fn bad_request(message: &str) -> Self {
        Self {
            code: "BAD_REQUEST".into(),
            message: message.into(),
            status: 400,
        }
    }
}

impl std::fmt::Display for PluginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)
    }
}

impl std::error::Error for PluginError {}

/// Log levels for plugin logging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

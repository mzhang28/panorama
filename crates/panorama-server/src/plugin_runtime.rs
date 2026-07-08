use async_trait::async_trait;
use bytes::Bytes;
use panorama_core::plugin::{LogLevel, ObjectData, PluginContext, PluginError};
use panorama_core::schema::Schema;
use panorama_core::types::{FieldValue, Node, ObjectRef};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use crate::backtrace::BacktraceStore;
use crate::object_store::ObjectStorage;
use crate::schema_registry::SchemaRegistry;
use crate::storage::NodeStorage;

/// Holds a query result set for streaming to the WASM guest in chunks.
pub struct QueryCursor {
  pub rows: Vec<serde_json::Value>,
  pub next_idx: usize,
}

/// Concrete implementation of PluginContext that plugins use
/// to interact with the Panorama platform.
pub struct RuntimeContext {
  plugin_id: String,
  base_path: String,
  storage: NodeStorage,
  schema_registry: SchemaRegistry,
  object_storage: ObjectStorage,
  /// The capabilities granted to this plugin
  granted_caps: panorama_core::capabilities::CapabilityGrants,
  /// Host-side backtrace store shared with wasm host functions
  pub backtrace_store: Arc<BacktraceStore>,
  /// Active query cursors for streaming results to WASM guests.
  pub query_cursors: Mutex<HashMap<u32, QueryCursor>>,
  next_cursor_id: AtomicU32,
}

impl RuntimeContext {
  pub fn new(
    plugin_id: &str,
    storage: NodeStorage,
    schema_registry: SchemaRegistry,
    object_storage: ObjectStorage,
    granted_caps: panorama_core::capabilities::CapabilityGrants,
  ) -> Self {
    Self {
      plugin_id: plugin_id.to_string(),
      base_path: format!("/plugin/{}", plugin_id),
      storage,
      schema_registry,
      object_storage,
      granted_caps,
      backtrace_store: Arc::new(BacktraceStore::new()),
      query_cursors: Mutex::new(HashMap::new()),
      next_cursor_id: AtomicU32::new(1),
    }
  }

  /// Store a query result and return a cursor ID for streaming it out.
  pub fn create_query_cursor(&self, rows: Vec<serde_json::Value>) -> u32 {
    let id = self.next_cursor_id.fetch_add(1, Ordering::Relaxed);
    self
      .query_cursors
      .lock()
      .unwrap()
      .insert(id, QueryCursor { rows, next_idx: 0 });
    id
  }

  pub fn schema_registry(&self) -> &SchemaRegistry {
    &self.schema_registry
  }

  fn check_field_read(&self, field_key: &str) -> Result<(), PluginError> {
    if !self.granted_caps.can_read_field(field_key) {
      Err(PluginError::permission_denied(&format!(
        "Plugin '{}' does not have read access to field '{}'",
        self.plugin_id, field_key
      )))
    } else {
      Ok(())
    }
  }

  fn check_field_write(&self, field_key: &str) -> Result<(), PluginError> {
    if !self.granted_caps.can_write_field(field_key) {
      Err(PluginError::permission_denied(&format!(
        "Plugin '{}' does not have write access to field '{}'",
        self.plugin_id, field_key
      )))
    } else {
      Ok(())
    }
  }
}

#[async_trait]
impl PluginContext for RuntimeContext {
  async fn create_nodes(&self, nodes: Vec<Node>) -> Result<Vec<Node>, PluginError> {
    for node in &nodes {
      if let Err(errors) = self
        .schema_registry
        .validate_required(&node.fields, &node.preferred_schemas)
      {
        return Err(PluginError::bad_request(&format!(
          "Schema validation failed: {}",
          errors.join("; ")
        )));
      }
    }
    self
      .storage
      .create_batch(nodes)
      .map_err(|e| PluginError::internal(e))
  }

  async fn get_node(&self, id: Uuid) -> Result<Option<Node>, PluginError> {
    self.storage.get(id).map_err(|e| PluginError::internal(e))
  }

  async fn update_node(
    &self,
    id: Uuid,
    fields: HashMap<String, FieldValue>,
  ) -> Result<Node, PluginError> {
    // Check permissions on each field being written
    for key in fields.keys() {
      self.check_field_write(key)?;
    }

    // Validate merged fields against required schemas
    if let Ok(Some(existing)) = self.storage.get(id) {
      let mut merged = existing.fields.clone();
      for (key, value) in &fields {
        merged.insert(key.clone(), value.clone());
      }
      if let Err(errors) = self
        .schema_registry
        .validate_required(&merged, &existing.preferred_schemas)
      {
        return Err(PluginError::bad_request(&format!(
          "Schema validation failed: {}",
          errors.join("; ")
        )));
      }
    }

    self
      .storage
      .update(id, fields)
      .map_err(|e| PluginError::internal(e))
  }

  async fn delete_node(&self, id: Uuid) -> Result<(), PluginError> {
    self
      .storage
      .delete(id)
      .map_err(|e| PluginError::internal(e))
  }

  async fn query(&self, query_string: &str) -> Result<Vec<serde_json::Value>, PluginError> {
    self
      .storage
      .query_lang(query_string)
      .map_err(|e| PluginError::internal(e))
  }

  async fn register_schema(&self, schema: Schema) -> Result<Schema, PluginError> {
    self
      .schema_registry
      .register(schema)
      .map_err(|e| PluginError::internal(e))
  }

  async fn get_schema(&self, schema_node_id: Uuid) -> Result<Option<Schema>, PluginError> {
    Ok(self.schema_registry.get(&schema_node_id))
  }

  async fn put_object(
    &self,
    bucket: &str,
    key: &str,
    data: Bytes,
    mime_type: &str,
  ) -> Result<ObjectRef, PluginError> {
    self
      .object_storage
      .put(bucket, key, &data, mime_type)
      .map_err(|e| PluginError::internal(e))
  }

  async fn get_object(&self, bucket: &str, key: &str) -> Result<Option<ObjectData>, PluginError> {
    self
      .object_storage
      .get(bucket, key)
      .map(|opt| {
        opt.map(|(data, mime_type)| {
          let size = data.len() as u64;
          ObjectData {
            data,
            mime_type,
            size,
          }
        })
      })
      .map_err(|e| PluginError::internal(e))
  }

  async fn delete_object(&self, bucket: &str, key: &str) -> Result<(), PluginError> {
    self
      .object_storage
      .delete(bucket, key)
      .map_err(|e| PluginError::internal(e))
  }

  async fn list_objects(
    &self,
    bucket: &str,
    prefix: Option<&str>,
  ) -> Result<Vec<ObjectRef>, PluginError> {
    self
      .object_storage
      .list(bucket, prefix)
      .map_err(|e| PluginError::internal(e))
  }

  fn plugin_id(&self) -> &str {
    &self.plugin_id
  }

  fn base_path(&self) -> String {
    self.base_path.clone()
  }

  async fn log(&self, level: LogLevel, message: &str) {
    match level {
      LogLevel::Debug => tracing::debug!(plugin=%self.plugin_id, "{}", message),
      LogLevel::Info => tracing::info!(plugin=%self.plugin_id, "{}", message),
      LogLevel::Warn => tracing::warn!(plugin=%self.plugin_id, "{}", message),
      LogLevel::Error => tracing::error!(plugin=%self.plugin_id, "{}", message),
    }
  }
}

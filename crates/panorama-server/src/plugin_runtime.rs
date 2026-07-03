use std::collections::HashMap;
use bytes::Bytes;
use async_trait::async_trait;
use panorama_core::plugin::{
    LogLevel, NodeQuery, ObjectData, PluginContext, PluginError,
};
use panorama_core::types::{FieldValue, Node, ObjectRef};
use panorama_core::schema::Schema;
use uuid::Uuid;

use crate::storage::NodeStorage;
use crate::schema_registry::SchemaRegistry;
use crate::object_store::ObjectStorage;

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
        }
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
    async fn create_node(&self, node: Node) -> Result<Node, PluginError> {
        // Enforce required schemas before persisting
        if let Err(errors) = self.schema_registry.validate_required(&node.fields, &node.preferred_schemas) {
            return Err(PluginError::bad_request(&format!(
                "Schema validation failed: {}", errors.join("; ")
            )));
        }
        self.storage
            .create(node)
            .map_err(|e| PluginError::internal(e))
    }

    async fn get_node(&self, id: Uuid) -> Result<Option<Node>, PluginError> {
        Ok(self.storage.get(&id))
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
        if let Some(existing) = self.storage.get(&id) {
            let mut merged = existing.fields.clone();
            for (key, value) in &fields {
                merged.insert(key.clone(), value.clone());
            }
            if let Err(errors) = self.schema_registry.validate_required(&merged, &existing.preferred_schemas) {
                return Err(PluginError::bad_request(&format!(
                    "Schema validation failed: {}", errors.join("; ")
                )));
            }
        }

        self.storage
            .update(&id, fields)
            .map_err(|e| PluginError::internal(e))
    }

    async fn delete_node(&self, id: Uuid) -> Result<(), PluginError> {
        self.storage
            .delete(&id)
            .map_err(|e| PluginError::internal(e))
    }

    async fn query_nodes(&self, query: NodeQuery) -> Result<Vec<Node>, PluginError> {
        // Check read permission if filtering by specific fields
        for key in query.field_filters.keys() {
            self.check_field_read(key)?;
        }
        let mut results = self.storage.query(
            &query.field_filters,
            query.space_id,
            query.limit,
        );
        // Apply sort
        if let Some(sort_by) = &query.sort_by {
            let descending = sort_by.starts_with('-');
            let field_key = if descending { &sort_by[1..] } else { sort_by.as_str() };
            results.sort_by(|a, b| {
                let va = a.get_field(field_key).map(|v| format!("{:?}", v));
                let vb = b.get_field(field_key).map(|v| format!("{:?}", v));
                if descending {
                    vb.cmp(&va)
                } else {
                    va.cmp(&vb)
                }
            });
        }
        // Apply offset
        if let Some(offset) = query.offset {
            results = results.into_iter().skip(offset).collect();
        }
        Ok(results)
    }

    async fn register_schema(&self, schema: Schema) -> Result<Schema, PluginError> {
        Ok(self.schema_registry.register(schema))
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
        self.object_storage
            .put(bucket, key, &data, mime_type)
            .map_err(|e| PluginError::internal(e))
    }

    async fn get_object(
        &self,
        bucket: &str,
        key: &str,
    ) -> Result<Option<ObjectData>, PluginError> {
        self.object_storage
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
        self.object_storage
            .delete(bucket, key)
            .map_err(|e| PluginError::internal(e))
    }

    async fn list_objects(
        &self,
        bucket: &str,
        prefix: Option<&str>,
    ) -> Result<Vec<ObjectRef>, PluginError> {
        self.object_storage
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

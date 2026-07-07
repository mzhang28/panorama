use std::collections::HashMap;
use std::sync::Arc;

use dashmap::DashMap;
use panorama_core::schema::Schema;
use panorama_core::types::{FieldValue, SchemaRef};
use uuid::Uuid;

/// Registry for all schemas in the platform.
/// Schemas are stored as nodes but also indexed here for fast lookup.
#[derive(Clone)]
pub struct SchemaRegistry {
  schemas: Arc<DashMap<Uuid, Schema>>,
  /// name -> latest version's schema node ID
  name_index: Arc<DashMap<String, Uuid>>,
}

impl SchemaRegistry {
  pub fn new() -> Self {
    Self {
      schemas: Arc::new(DashMap::new()),
      name_index: Arc::new(DashMap::new()),
    }
  }

  pub fn register(&self, mut schema: Schema) -> Result<Schema, String> {
    // Validate schema definition before registering
    schema.validate_definition()?;

    // Assign a node ID if not already set
    if schema.node_id.is_nil() {
      schema.node_id = Uuid::new_v4();
    }
    self.name_index.insert(schema.name.clone(), schema.node_id);
    self.schemas.insert(schema.node_id, schema.clone());
    Ok(schema)
  }

  pub fn get(&self, schema_node_id: &Uuid) -> Option<Schema> {
    self.schemas.get(schema_node_id).map(|s| s.clone())
  }

  pub fn get_by_name(&self, name: &str) -> Option<Schema> {
    self
      .name_index
      .get(name)
      .and_then(|id| self.schemas.get(&id))
      .map(|s| s.clone())
  }

  pub fn list_all(&self) -> Vec<Schema> {
    self.schemas.iter().map(|s| s.value().clone()).collect()
  }

  pub fn list_by_app(&self, app_id: &str) -> Vec<Schema> {
    // Schema ownership is tracked via a naming convention: "app_id/schema_name"
    self
      .schemas
      .iter()
      .filter(|s| s.name.starts_with(&format!("{}/", app_id)))
      .map(|s| s.value().clone())
      .collect()
  }

  /// Validate `fields` against every **required** schema referenced by
  /// `preferred_schemas`.  Returns `Ok(())` when all required schemas pass,
  /// or `Err(messages)` with human-readable error lines for each violation.
  /// Schemas in `Preferred` mode and schemas not found in the registry are
  /// silently skipped — only `Required` schemas block the write.
  pub fn validate_required(
    &self,
    fields: &HashMap<String, FieldValue>,
    preferred_schemas: &[SchemaRef],
  ) -> Result<(), Vec<String>> {
    let mut errors: Vec<String> = Vec::new();

    for schema_ref in preferred_schemas {
      if let Some(schema) = self.get(&schema_ref.schema_node_id) {
        if schema.schema_mode == panorama_core::schema::SchemaMode::Required {
          let result = schema.validate(fields);
          if !result.is_valid {
            for e in &result.errors {
              errors.push(format!("[{}] {}: {}", schema.name, e.field_name, e.message));
            }
          }
        }
      }
    }

    if errors.is_empty() {
      Ok(())
    } else {
      Err(errors)
    }
  }
}

impl Default for SchemaRegistry {
  fn default() -> Self {
    Self::new()
  }
}

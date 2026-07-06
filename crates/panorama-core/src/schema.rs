use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::types::FieldValue;
pub use crate::types::SchemaVersion;

/// A schema defines a group of fields with requirements.
/// Schemas are themselves nodes (with the Schema schema type).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Schema {
  /// The node ID of this schema definition
  pub node_id: Uuid,
  /// Human-readable name
  pub name: String,
  /// Current version
  pub version: SchemaVersion,
  /// The fields defined by this schema
  pub fields: Vec<SchemaField>,
  /// Whether this schema is required or preferred
  pub schema_mode: SchemaMode,
  /// IDs of old schema versions (for migration tracking)
  pub previous_versions: Vec<SchemaVersionRef>,
  /// Migration scripts for upgrading from previous versions
  pub migrations: Vec<Migration>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaVersionRef {
  pub schema_node_id: Uuid,
  pub version: SchemaVersion,
}

/// A field definition within a schema
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaField {
  /// The field name (without namespace)
  pub name: String,
  /// The namespace this field belongs to
  pub namespace: String,
  /// Expected type for this field
  pub field_type: Option<FieldTypeConstraint>,
  /// Whether this field is required
  pub required: bool,
  /// Default value if not provided
  pub default: Option<FieldValue>,
  /// Human-readable description
  pub description: Option<String>,
  /// Whether this is a computed field
  pub computed: Option<ComputedFieldConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldTypeConstraint {
  /// The expected type tag matching FieldValue's serde tag
  pub type_tag: String,
  /// For arrays, the expected element type
  pub element_type: Option<Box<FieldTypeConstraint>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputedFieldConfig {
  /// The computation mode
  pub mode: ComputeMode,
  /// Expression or reference to compute the value
  /// This is a simple expression language for v0.0
  pub expression: String,
  /// Dependencies: field references this computation depends on
  pub dependencies: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ComputeMode {
  /// Computation must complete before write is considered done
  Eager,
  /// Computation starts on write but write completes immediately
  Deferred,
  /// Computation happens on read, caller is responsible
  Read,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SchemaMode {
  /// Nodes can fall out of schema (with warnings)
  Preferred,
  /// Nodes must always conform to schema
  Required,
}

/// A migration from one schema version to another
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Migration {
  pub from_version: SchemaVersion,
  pub to_version: SchemaVersion,
  /// Migration script (simple field mappings for v0.0)
  pub field_mappings: HashMap<String, String>,
  /// Description of what this migration does
  pub description: String,
}

/// Result of validating a node against a schema
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaValidationResult {
  pub schema_node_id: Uuid,
  pub schema_name: String,
  pub is_valid: bool,
  pub warnings: Vec<SchemaWarning>,
  pub errors: Vec<SchemaError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaWarning {
  pub field_name: String,
  pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaError {
  pub field_name: String,
  pub message: String,
}

impl Schema {
  /// Validate a set of fields against this schema
  pub fn validate(&self, fields: &HashMap<String, FieldValue>) -> SchemaValidationResult {
    let mut warnings = Vec::new();
    let mut errors = Vec::new();

    for schema_field in &self.fields {
      let key = format!("{}:{}", schema_field.namespace, schema_field.name);
      match fields.get(&key) {
        Some(value) => {
          // Check type constraint if specified
          if let Some(type_constraint) = &schema_field.field_type {
            if !self.check_type(value, type_constraint) {
              let msg = format!(
                "Field '{}' expected type '{}' but got '{}'",
                key,
                type_constraint.type_tag,
                self.value_type_tag(value)
              );
              if schema_field.required {
                errors.push(SchemaError {
                  field_name: key.clone(),
                  message: msg,
                });
              } else {
                warnings.push(SchemaWarning {
                  field_name: key.clone(),
                  message: msg,
                });
              }
            }
          }
        }
        None => {
          if schema_field.required && schema_field.default.is_none() {
            errors.push(SchemaError {
              field_name: key.clone(),
              message: format!("Required field '{}' is missing", key),
            });
          } else if schema_field.required && schema_field.default.is_some() {
            warnings.push(SchemaWarning {
              field_name: key.clone(),
              message: format!(
                "Required field '{}' is missing but has a default value",
                key
              ),
            });
          }
        }
      }
    }

    SchemaValidationResult {
      schema_node_id: self.node_id,
      schema_name: self.name.clone(),
      is_valid: errors.is_empty(),
      warnings,
      errors,
    }
  }

  fn check_type(&self, value: &FieldValue, constraint: &FieldTypeConstraint) -> bool {
    match (value, constraint.type_tag.as_str()) {
      (FieldValue::String(_), "String") => true,
      (FieldValue::Integer(_), "Integer") => true,
      (FieldValue::Float(_), "Float") => true,
      (FieldValue::Boolean(_), "Boolean") => true,
      (FieldValue::DateTime(_), "DateTime") => true,
      (FieldValue::NodeRef(_), "NodeRef") => true,
      (FieldValue::Json(_), "Json") => true,
      (FieldValue::ObjectRef(_), "ObjectRef") => true,
      (FieldValue::Binary(_), "Binary") => true,
      (FieldValue::Array(elements), "Array") => {
        if let Some(elem_constraint) = &constraint.element_type {
          elements.iter().all(|e| self.check_type(e, elem_constraint))
        } else {
          true
        }
      }
      _ => false,
    }
  }

  fn value_type_tag(&self, value: &FieldValue) -> &str {
    match value {
      FieldValue::String(_) => "String",
      FieldValue::Integer(_) => "Integer",
      FieldValue::Float(_) => "Float",
      FieldValue::Boolean(_) => "Boolean",
      FieldValue::DateTime(_) => "DateTime",
      FieldValue::Array(_) => "Array",
      FieldValue::NodeRef(_) => "NodeRef",
      FieldValue::Json(_) => "Json",
      FieldValue::ObjectRef(_) => "ObjectRef",
      FieldValue::Binary(_) => "Binary",
    }
  }
}

/// Built-in system schema IDs and their definitions
pub mod system_schemas {
  use super::*;

  /// The "Node Time" system schema - provides temporal fields
  pub fn node_time_schema() -> Schema {
    Schema {
      node_id: Uuid::nil(), // Will be assigned on registration
      name: "NodeTime".into(),
      version: SchemaVersion::new(1, 0),
      fields: vec![
        SchemaField {
          name: "node_time".into(),
          namespace: "system".into(),
          field_type: Some(FieldTypeConstraint {
            type_tag: "DateTime".into(),
            element_type: None,
          }),
          required: false,
          default: None,
          description: Some("Primary time associated with this node".into()),
          computed: None,
        },
        SchemaField {
          name: "node_start_time".into(),
          namespace: "system".into(),
          field_type: Some(FieldTypeConstraint {
            type_tag: "DateTime".into(),
            element_type: None,
          }),
          required: false,
          default: None,
          description: Some("Start time; falls back to node_time if not set".into()),
          computed: None,
        },
        SchemaField {
          name: "node_end_time".into(),
          namespace: "system".into(),
          field_type: Some(FieldTypeConstraint {
            type_tag: "DateTime".into(),
            element_type: None,
          }),
          required: false,
          default: None,
          description: Some("End time; falls back to node_time if not set".into()),
          computed: None,
        },
      ],
      schema_mode: SchemaMode::Preferred,
      previous_versions: vec![],
      migrations: vec![],
    }
  }

  /// The "Node Info" system schema - basic human-readable metadata
  pub fn node_info_schema() -> Schema {
    Schema {
      node_id: Uuid::nil(),
      name: "NodeInfo".into(),
      version: SchemaVersion::new(1, 0),
      fields: vec![
        SchemaField {
          name: "node_title".into(),
          namespace: "system".into(),
          field_type: Some(FieldTypeConstraint {
            type_tag: "String".into(),
            element_type: None,
          }),
          required: false,
          default: None,
          description: Some("Human-readable label for this node".into()),
          computed: None,
        },
        SchemaField {
          name: "node_description".into(),
          namespace: "system".into(),
          field_type: Some(FieldTypeConstraint {
            type_tag: "String".into(),
            element_type: None,
          }),
          required: false,
          default: None,
          description: Some("Human-readable description".into()),
          computed: None,
        },
      ],
      schema_mode: SchemaMode::Preferred,
      previous_versions: vec![],
      migrations: vec![],
    }
  }
}

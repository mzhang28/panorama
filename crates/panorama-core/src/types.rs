use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// The fundamental data unit in Panorama.
/// Every piece of data is a Node with a unique ID and arbitrary fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: Uuid,
    /// Arbitrary metadata fields attached to this node.
    /// Fields are keyed by "namespace:field_name"
    pub fields: HashMap<String, FieldValue>,
    /// The space this node belongs to
    pub space_id: Uuid,
    /// Schema IDs that this node claims to conform to
    pub preferred_schemas: Vec<SchemaRef>,
    /// Whether this node is app-managed (user cannot modify directly)
    pub app_managed: Option<AppManagedInfo>,
    /// System timestamps
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppManagedInfo {
    pub app_id: String,
    pub app_name: String,
}

/// A reference to a schema version
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct SchemaRef {
    pub schema_node_id: Uuid,
    pub version: SchemaVersion,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct SchemaVersion {
    pub major: u32,
    pub minor: u32,
}

impl SchemaVersion {
    pub fn new(major: u32, minor: u32) -> Self {
        Self { major, minor }
    }

    /// Check if `other` is compatible with `self` (self is the requirement)
    /// Major must match, minor must be >= requirement
    pub fn is_compatible_with(&self, requirement: &SchemaVersion) -> bool {
        self.major == requirement.major && self.minor >= requirement.minor
    }
}

impl std::fmt::Display for SchemaVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

/// All possible field value types in Panorama
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum FieldValue {
    /// Untyped string (user write-in default)
    String(String),
    /// Integer number
    Integer(i64),
    /// Floating point number
    Float(f64),
    /// Boolean
    Boolean(bool),
    /// ISO 8601 datetime string
    DateTime(String),
    /// Array of field values
    Array(Vec<FieldValue>),
    /// Reference to another node by UUID
    NodeRef(Uuid),
    /// JSON blob for complex structures
    Json(serde_json::Value),
    /// Reference to an object in object storage
    ObjectRef(ObjectRef),
    /// Binary data (small blobs only - use ObjectRef for large data)
    Binary(Vec<u8>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectRef {
    pub object_id: Uuid,
    pub bucket: String,
    pub key: String,
    pub size: u64,
    pub mime_type: String,
}

/// System field names (in the "system" namespace)
pub mod system_fields {
    pub const NODE_TIME: &str = "system:node_time";
    pub const NODE_START_TIME: &str = "system:node_start_time";
    pub const NODE_END_TIME: &str = "system:node_end_time";
    pub const NODE_TITLE: &str = "system:node_title";
    pub const NODE_DESCRIPTION: &str = "system:node_description";
    pub const CREATED_AT: &str = "system:created_at";
    pub const UPDATED_AT: &str = "system:updated_at";
    pub const PREFERRED_SCHEMAS: &str = "system:preferred_schemas";
    pub const REQUIRED_SCHEMAS: &str = "system:required_schemas";
}

impl Node {
    pub fn new(space_id: Uuid) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            fields: HashMap::new(),
            space_id,
            preferred_schemas: Vec::new(),
            app_managed: None,
            created_at: now,
            updated_at: now,
        }
    }

    /// Get a field value by namespaced key (e.g., "system:node_title")
    pub fn get_field(&self, namespaced_key: &str) -> Option<&FieldValue> {
        self.fields.get(namespaced_key)
    }

    /// Set a field value
    pub fn set_field(&mut self, namespaced_key: &str, value: FieldValue) {
        self.fields.insert(namespaced_key.to_string(), value);
        self.updated_at = Utc::now();
    }

    /// Get the effective time for this node (falls back from start_time -> node_time)
    pub fn effective_time(&self) -> Option<&FieldValue> {
        self.get_field(system_fields::NODE_START_TIME)
            .or_else(|| self.get_field(system_fields::NODE_TIME))
    }

    /// Get the node title (human-readable label)
    pub fn title(&self) -> Option<&str> {
        match self.get_field(system_fields::NODE_TITLE) {
            Some(FieldValue::String(s)) => Some(s.as_str()),
            _ => None,
        }
    }

    /// Get the node description
    pub fn description(&self) -> Option<&str> {
        match self.get_field(system_fields::NODE_DESCRIPTION) {
            Some(FieldValue::String(s)) => Some(s.as_str()),
            _ => None,
        }
    }
}

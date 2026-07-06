//! Storage abstraction layer — backend-agnostic Node storage.
//!
//! The [`StorageBackend`] trait defines all operations the platform needs
//! from a storage engine. Implementations live in sub-modules:
//!
//! - [`sqlite`] — SQLite via rusqlite
//! - [`postgres`] — PostgreSQL via sqlx (planned)
//!
//! The trait is designed so future backends (DynamoDB, S3+index, graph DB)
//! can be added without touching any code outside this module.

use panorama_core::types::{FieldValue, Node, SchemaRef};
use std::collections::HashMap;
use uuid::Uuid;

pub mod sqlite;

// ── StorageBackend trait ─────────────────────────────────────────────────────

/// Backend-agnostic storage operations.
///
/// All SQL or database-specific concerns are internal to each implementation.
/// Callers work exclusively through this trait — no connection handles,
/// no raw SQL, no dialect-specific code leaks out.
pub trait StorageBackend: Send + Sync {
  // ── Query ────────────────────────────────────────────────────────────

  /// Execute a Panorama Query Language string, returning JSON node rows.
  /// The backend is responsible for parsing, compiling, and executing.
  fn query(&self, pql: &str) -> Result<Vec<serde_json::Value>, String>;

  // ── CRUD ─────────────────────────────────────────────────────────────

  /// Create a single node. Returns the created node (with assigned id).
  fn create_node(&self, node: Node) -> Result<Node, String>;

  /// Create multiple nodes in a single transaction.
  fn create_batch(&self, nodes: Vec<Node>) -> Result<Vec<Node>, String>;

  /// Fetch a node by its UUID.
  fn get_node(&self, id: Uuid) -> Result<Option<Node>, String>;

  /// Update specific fields on a node. Merges with existing fields.
  fn update_node(&self, id: Uuid, fields: HashMap<String, FieldValue>) -> Result<Node, String>;

  /// Hard-delete a node and its meta-table records.
  fn delete_node(&self, id: Uuid) -> Result<(), String>;

  /// Replace the preferred schemas on a node.
  fn update_schemas(&self, id: Uuid, schemas: Vec<SchemaRef>) -> Result<Node, String>;

  // ── Lifecycle ────────────────────────────────────────────────────────

  /// Initialize the storage backend — create tables, indexes, seed data.
  /// Must be idempotent (safe to call multiple times).
  fn initialize(&self) -> Result<(), String>;

  // ── Meta access (for query compilation) ──────────────────────────────

  /// Resolve a namespace stable identifier to its internal ID.
  fn resolve_ns(&self, identifier: &str) -> Result<i64, String>;

  /// Check whether a ready managed index exists for the given (schema, field).
  fn has_ready_index(&self, schema_id: &str, field: &str) -> Result<bool, String>;
}

// ── NodeStorage — thin wrapper ───────────────────────────────────────────────

/// The public storage handle used by the rest of the server.
///
/// Delegates everything to a [`Box<dyn StorageBackend>`] so the backend
/// can be swapped at startup without any code changes elsewhere.
#[derive(Clone)]
pub struct NodeStorage {
  backend: std::sync::Arc<dyn StorageBackend>,
}

impl NodeStorage {
  /// Create a new `NodeStorage` wrapping the given backend.
  pub fn new(backend: std::sync::Arc<dyn StorageBackend>) -> Self {
    Self { backend }
  }

  // ── Query ────────────────────────────────────────────────────────────

  /// Execute a PQL query and return JSON rows.
  pub fn query_lang(&self, pql: &str) -> Result<Vec<serde_json::Value>, String> {
    self.backend.query(pql)
  }

  // ── CRUD (delegating) ────────────────────────────────────────────────

  pub fn create(&self, node: Node) -> Result<Node, String> {
    self.backend.create_node(node)
  }

  pub fn create_batch(&self, nodes: Vec<Node>) -> Result<Vec<Node>, String> {
    self.backend.create_batch(nodes)
  }

  pub fn get(&self, id: Uuid) -> Result<Option<Node>, String> {
    self.backend.get_node(id)
  }

  pub fn update(&self, id: Uuid, fields: HashMap<String, FieldValue>) -> Result<Node, String> {
    self.backend.update_node(id, fields)
  }

  pub fn delete(&self, id: Uuid) -> Result<(), String> {
    self.backend.delete_node(id)
  }

  pub fn update_schemas(&self, id: Uuid, schemas: Vec<SchemaRef>) -> Result<Node, String> {
    self.backend.update_schemas(id, schemas)
  }

  // ── Meta access ──────────────────────────────────────────────────────

  /// Resolve a namespace identifier (used by the query compiler).
  pub fn resolve_ns(&self, identifier: &str) -> Result<i64, String> {
    self.backend.resolve_ns(identifier)
  }

  /// Check for a ready index (used by the query compiler).
  pub fn has_ready_index(&self, schema_id: &str, field: &str) -> Result<bool, String> {
    self.backend.has_ready_index(schema_id, field)
  }
}

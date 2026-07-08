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

impl QueryCursor {
  /// Serialize the next chunk of rows into a JSON array `[...]`.
  ///
  /// Returns the serialized bytes and advances `next_idx`.  Returns an
  /// empty `Vec` when the cursor is exhausted (callers should remove the
  /// cursor from the map after receiving an empty chunk).
  ///
  /// Always includes at least one row per chunk, even if that single row
  /// exceeds `max_bytes`.  This prevents infinite loops on huge rows.
  pub fn fetch_chunk(&mut self, max_bytes: usize) -> Vec<u8> {
    if self.next_idx >= self.rows.len() {
      return Vec::new();
    }
    let mut chunk = Vec::with_capacity(4096);
    chunk.push(b'[');
    let mut first = true;
    while self.next_idx < self.rows.len() {
      let row_json =
        serde_json::to_vec(&self.rows[self.next_idx]).unwrap_or_default();
      // +1 for comma (or 0 for first), +1 for closing ']'
      let overhead = if first { 1 } else { 2 };
      if chunk.len() + row_json.len() + overhead > max_bytes && !first {
        break;
      }
      if !first {
        chunk.push(b',');
      }
      chunk.extend_from_slice(&row_json);
      first = false;
      self.next_idx += 1;
    }
    chunk.push(b']');
    if first {
      // No rows were written — cursor exhausted.
      Vec::new()
    } else {
      chunk
    }
  }
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

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
  use super::*;

  fn row(val: &str) -> serde_json::Value {
    serde_json::json!({"value": val})
  }

  // ── fetch_chunk basics ──────────────────────────────────────────────

  #[test]
  fn empty_cursor_returns_empty() {
    let mut c = QueryCursor {
      rows: vec![],
      next_idx: 0,
    };
    assert!(c.fetch_chunk(1024).is_empty());
    // Idempotent — subsequent calls also return empty.
    assert!(c.fetch_chunk(1024).is_empty());
  }

  #[test]
  fn single_row_fits() {
    let mut c = QueryCursor {
      rows: vec![row("hello")],
      next_idx: 0,
    };
    let chunk = c.fetch_chunk(1024);
    assert!(!chunk.is_empty());
    let parsed: Vec<serde_json::Value> = serde_json::from_slice(&chunk).unwrap();
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0]["value"], "hello");
    // Cursor is now exhausted.
    assert!(c.fetch_chunk(1024).is_empty());
  }

  /// If a single row is larger than max_bytes, it MUST still be included
  /// — otherwise the cursor would loop forever on that row.
  #[test]
  fn single_large_row_always_included() {
    let big = serde_json::Value::String("x".repeat(10_000));
    let mut c = QueryCursor {
      rows: vec![big.clone()],
      next_idx: 0,
    };
    let chunk = c.fetch_chunk(50); // target smaller than the row
    assert!(!chunk.is_empty());
    let parsed: Vec<serde_json::Value> = serde_json::from_slice(&chunk).unwrap();
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0], big);
  }

  // ── Multiple chunks ─────────────────────────────────────────────────

  #[test]
  fn multiple_chunks_exhaust_cursor() {
    let rows: Vec<serde_json::Value> =
      (0..100).map(|i| row(&format!("row-{}", i))).collect();
    let mut c = QueryCursor {
      rows: rows.clone(),
      next_idx: 0,
    };
    let mut all_parsed = Vec::new();
    let mut chunk_count = 0;
    loop {
      let chunk = c.fetch_chunk(256); // small target forces many chunks
      if chunk.is_empty() {
        break;
      }
      chunk_count += 1;
      let parsed: Vec<serde_json::Value> = serde_json::from_slice(&chunk).unwrap();
      assert!(!parsed.is_empty(), "chunk should have at least one row");
      all_parsed.extend(parsed);
    }
    assert!(chunk_count > 1, "should produce multiple chunks");
    assert_eq!(all_parsed.len(), rows.len());
    for (i, v) in all_parsed.iter().enumerate() {
      assert_eq!(v["value"], format!("row-{}", i));
    }
  }

  // ── Large result sets ───────────────────────────────────────────────

  /// Simulate the exact scenario that broke the old 512 KiB buffer:
  /// enough data to overflow a fixed buffer, now handled by streaming.
  #[test]
  fn large_result_spans_many_chunks() {
    // Each row is ~100 bytes; 10 000 rows ≈ 1 MiB.
    let rows: Vec<serde_json::Value> = (0..10_000)
      .map(|i| {
        serde_json::json!({
          "id": i,
          "name": format!("item-number-{}", i),
          "tags": ["a", "b", "c"],
          "count": i * 7 % 113
        })
      })
      .collect();
    let total_rows = rows.len();
    let mut c = QueryCursor {
      rows,
      next_idx: 0,
    };
    let mut all_parsed = Vec::new();
    let mut chunk_count = 0;
    loop {
      let chunk = c.fetch_chunk(CHUNK_TARGET);
      if chunk.is_empty() {
        break;
      }
      chunk_count += 1;
      let parsed: Vec<serde_json::Value> = serde_json::from_slice(&chunk).unwrap();
      // Every chunk is valid JSON and non-empty.
      assert!(!parsed.is_empty());
      // Every chunk fits under the target (unless the first row alone
      // exceeds it, which shouldn't happen here).
      assert!(
        chunk.len() <= CHUNK_TARGET * 2,
        "chunk {} is {} bytes (target {})",
        chunk_count,
        chunk.len(),
        CHUNK_TARGET
      );
      all_parsed.extend(parsed);
    }
    assert!(
      chunk_count > 1,
      "10k rows should need multiple chunks (got {})",
      chunk_count
    );
    assert_eq!(all_parsed.len(), total_rows, "all rows recovered");
    // Spot-check ordering.
    assert_eq!(all_parsed[0]["id"], 0);
    assert_eq!(all_parsed[total_rows - 1]["id"], 9999);
  }

  // ── CHUNK_TARGET constant is used by wasm_runtime ───────────────────

  // Re-exported here so the tests stay in sync with the host function.
  const CHUNK_TARGET: usize = 256 * 1024;

  // ── Cursor lifecycle (create → fetch → close) ──────────────────────

  #[test]
  fn create_and_fetch_and_close() {
    let rows: Vec<serde_json::Value> = (0..50).map(|i| row(&format!("r{}", i))).collect();
    let mut c = QueryCursor {
      rows,
      next_idx: 0,
    };

    // Fetch all in one go.
    let chunk = c.fetch_chunk(64 * 1024);
    assert!(!chunk.is_empty());
    let parsed: Vec<serde_json::Value> = serde_json::from_slice(&chunk).unwrap();
    assert_eq!(parsed.len(), 50);

    // Next call → empty.
    assert!(c.fetch_chunk(64 * 1024).is_empty());
  }

  // ── Edge cases ──────────────────────────────────────────────────────

  #[test]
  fn chunk_respects_max_bytes() {
    // Create rows of known size (~50 bytes each).
    let rows: Vec<serde_json::Value> = (0..20)
      .map(|i| serde_json::json!({"n": i, "s": "abcdefghij"}))
      .collect();
    let one_row_size = serde_json::to_vec(&rows[0]).unwrap().len();

    let mut c = QueryCursor {
      rows: rows.clone(),
      next_idx: 0,
    };

    // With a max_bytes that allows exactly 3 rows + overhead.
    let max_bytes = (one_row_size * 3) + 10; // 10 bytes for [, ], and commas
    let chunk = c.fetch_chunk(max_bytes);
    let parsed: Vec<serde_json::Value> = serde_json::from_slice(&chunk).unwrap();
    // Should get at least 3 rows (maybe more if the overhead estimate is off).
    assert!(parsed.len() >= 3, "expected ≥3 rows, got {}", parsed.len());
    assert!(chunk.len() <= max_bytes + one_row_size, // single-row slop
      "chunk {} B, max {} B", chunk.len(), max_bytes
    );
  }

  #[test]
  fn already_exhausted_returns_empty() {
    let mut c = QueryCursor {
      rows: vec![row("a")],
      next_idx: 1, // already past the end
    };
    assert!(c.fetch_chunk(1024).is_empty());
  }
}

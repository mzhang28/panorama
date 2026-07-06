//! Durable Operation Stream — post-commit event log for deferred reactors.
//!
//! Implements the op stream described in HOOK_DESIGN.md §3.
//! Each write transaction atomically appends an entry to the op stream.
//! Deferred reactors consume the stream with at-least-once delivery semantics.

use chrono::Utc;
use panorama_core::reactor::{OpStreamEntry, OpType};
use std::sync::atomic::{AtomicU64, Ordering};
use uuid::Uuid;

use crate::storage::NodeStorage;

// ── Op Stream ─────────────────────────────────────────────────────────────────

/// The durable operation stream.
///
/// Each entry is persisted as a node (conforming to the `OpStream` system schema)
/// in the same transaction as the write that produced it.
///
/// Sequence numbers are assigned atomically from an in-memory counter.
/// After a restart, the counter is initialized from `MAX(sequence)` in storage.
pub struct OpStream {
  /// Reference to the storage backend for persisting entries.
  storage: NodeStorage,

  /// Monotonically increasing sequence number counter.
  next_sequence: AtomicU64,
}

impl OpStream {
  /// Create a new op stream.
  /// Call `initialize()` after construction to seed the sequence counter.
  pub fn new(storage: NodeStorage) -> Self {
    Self {
      storage,
      next_sequence: AtomicU64::new(1),
    }
  }

  /// Initialize the sequence counter from storage.
  /// Should be called once at startup.
  pub async fn initialize(&self) -> Result<(), String> {
    let query =
      "MATCH (n) IN space(\"default\") WHERE HAS_FIELD(n, \"system\", \"op_sequence\") RETURN n";
    let rows = self
      .storage
      .query_lang(query)
      .map_err(|e| format!("Failed to query op stream: {}", e))?;

    let max_seq = rows
      .iter()
      .filter_map(|row| row.get("system:op_sequence").and_then(|v| v.as_i64()))
      .max()
      .unwrap_or(0);

    if max_seq > 0 {
      self
        .next_sequence
        .store((max_seq + 1) as u64, Ordering::SeqCst);
      tracing::info!(max_sequence = max_seq, "Op stream initialized");
    } else {
      tracing::info!("Op stream initialized (empty)");
    }

    Ok(())
  }

  /// Append an entry to the op stream.
  ///
  /// This should be called within the same transaction as the write
  /// that produced the event. The entry is persisted as a node.
  pub fn append_sync(
    &self,
    op_type: OpType,
    node_id: Option<Uuid>,
    schema_id: Option<Uuid>,
    space_id: Uuid,
    field_path: Option<&str>,
    new_value: Option<&serde_json::Value>,
    previous_value: Option<&serde_json::Value>,
    authorized_by: Option<&str>,
    source_app_id: Option<&str>,
  ) -> Result<OpStreamEntry, String> {
    let sequence = self.next_sequence.fetch_add(1, Ordering::SeqCst);
    let committed_at = Utc::now().to_rfc3339();

    let entry = OpStreamEntry {
      sequence,
      op_type,
      node_id,
      schema_id,
      space_id,
      field_path: field_path.map(|s| s.to_string()),
      new_value: new_value.cloned(),
      previous_value: previous_value.cloned(),
      authorized_by: authorized_by.map(|s| s.to_string()),
      committed_at: committed_at.clone(),
      source_app_id: source_app_id.map(|s| s.to_string()),
    };

    // Persist as a node
    let mut node = panorama_core::types::Node::new(space_id);
    node.preferred_schemas = vec![panorama_core::types::SchemaRef {
      schema_node_id: Uuid::nil(),
      version: panorama_core::types::SchemaVersion::new(1, 0),
    }];

    use panorama_core::types::FieldValue;

    node.set_field("system:op_sequence", FieldValue::Integer(sequence as i64));
    node.set_field(
      "system:op_type",
      FieldValue::String(match &entry.op_type {
        OpType::NodeCreated => "node_created".into(),
        OpType::NodeUpdated => "node_updated".into(),
        OpType::NodeDeleted => "node_deleted".into(),
        OpType::FieldWritten => "field_written".into(),
        OpType::SchemaInstalled => "schema_installed".into(),
        OpType::SchemaMigrated => "schema_migrated".into(),
        OpType::AppInstalled => "app_installed".into(),
        OpType::AppUninstalled => "app_uninstalled".into(),
      }),
    );
    if let Some(nid) = node_id {
      node.set_field("system:op_node_id", FieldValue::NodeRef(nid));
    }
    if let Some(sid) = schema_id {
      node.set_field("system:op_schema_id", FieldValue::NodeRef(sid));
    }
    node.set_field(
      "system:op_space_id",
      FieldValue::String(space_id.to_string()),
    );
    if let Some(fp) = field_path {
      node.set_field("system:op_field_path", FieldValue::String(fp.to_string()));
    }
    let data = serde_json::json!({
        "new_value": entry.new_value,
        "previous_value": entry.previous_value,
        "authorized_by": entry.authorized_by,
        "source_app_id": entry.source_app_id,
    });
    node.set_field("system:op_data", FieldValue::Json(data));
    node.set_field("system:op_committed_at", FieldValue::DateTime(committed_at));

    self
      .storage
      .create(node)
      .map_err(|e| format!("Failed to persist op stream entry: {}", e))?;

    tracing::debug!(
        sequence = sequence,
        op_type = ?entry.op_type,
        "Op stream entry appended"
    );

    Ok(entry)
  }

  /// Query entries after a given sequence number (exclusive).
  /// Used by deferred reactors to catch up on unprocessed events.
  pub fn query_since(&self, since_sequence: u64, limit: u64) -> Result<Vec<OpStreamEntry>, String> {
    let query = format!(
      "MATCH (n) IN space(\"default\") \
             WHERE HAS_FIELD(n, \"system\", \"op_sequence\") \
             RETURN n"
    );
    let rows = self
      .storage
      .query_lang(&query)
      .map_err(|e| format!("Failed to query op stream: {}", e))?;

    let mut entries: Vec<OpStreamEntry> = rows
      .iter()
      .filter_map(|row| self.row_to_entry(row))
      .filter(|e| e.sequence > since_sequence)
      .take(limit as usize)
      .collect();
    entries.sort_by_key(|e| e.sequence);

    Ok(entries)
  }

  /// Get the current maximum sequence number.
  pub fn current_sequence(&self) -> u64 {
    self.next_sequence.load(Ordering::SeqCst).saturating_sub(1)
  }

  /// Parse an OpStreamEntry from a query result row.
  fn row_to_entry(&self, row: &serde_json::Value) -> Option<OpStreamEntry> {
    let node: panorama_core::types::Node = serde_json::from_value(row.clone()).ok()?;

    let sequence = match node.get_field("system:op_sequence") {
      Some(panorama_core::types::FieldValue::Integer(i)) => *i as u64,
      _ => return None,
    };
    let op_type = match node.get_field("system:op_type") {
      Some(panorama_core::types::FieldValue::String(s)) => match s.as_str() {
        "node_created" => OpType::NodeCreated,
        "node_updated" => OpType::NodeUpdated,
        "node_deleted" => OpType::NodeDeleted,
        "field_written" => OpType::FieldWritten,
        "schema_installed" => OpType::SchemaInstalled,
        "schema_migrated" => OpType::SchemaMigrated,
        "app_installed" => OpType::AppInstalled,
        "app_uninstalled" => OpType::AppUninstalled,
        _ => return None,
      },
      _ => return None,
    };
    let node_id = match node.get_field("system:op_node_id") {
      Some(panorama_core::types::FieldValue::NodeRef(u)) => Some(*u),
      _ => None,
    };
    let schema_id = match node.get_field("system:op_schema_id") {
      Some(panorama_core::types::FieldValue::NodeRef(u)) => Some(*u),
      _ => None,
    };
    let space_id = match node.get_field("system:op_space_id") {
      Some(panorama_core::types::FieldValue::String(s)) => {
        Uuid::parse_str(s).unwrap_or(Uuid::nil())
      }
      _ => Uuid::nil(),
    };
    let field_path = match node.get_field("system:op_field_path") {
      Some(panorama_core::types::FieldValue::String(s)) => Some(s.clone()),
      _ => None,
    };
    let committed_at = match node.get_field("system:op_committed_at") {
      Some(panorama_core::types::FieldValue::DateTime(dt)) => dt.clone(),
      _ => String::new(),
    };

    // Parse op_data JSON
    let (new_value, previous_value, authorized_by, source_app_id) =
      match node.get_field("system:op_data") {
        Some(panorama_core::types::FieldValue::Json(j)) => {
          let new_val =
            j.get("new_value")
              .and_then(|v| if v.is_null() { None } else { Some(v.clone()) });
          let prev_val =
            j.get("previous_value")
              .and_then(|v| if v.is_null() { None } else { Some(v.clone()) });
          let auth = j
            .get("authorized_by")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
          let src = j
            .get("source_app_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
          (new_val, prev_val, auth, src)
        }
        _ => (None, None, None, None),
      };

    Some(OpStreamEntry {
      sequence,
      op_type,
      node_id,
      schema_id,
      space_id,
      field_path,
      new_value,
      previous_value,
      authorized_by,
      committed_at,
      source_app_id,
    })
  }
}

//! Physical schema resolution — consults `schema_tables` and `managed_indexes`
//! meta tables to determine how each field is physically stored (§7.1).
//!
//! The compiler uses this to decide:
//! - Which physical tables to JOIN against (promoted columns)
//! - Whether a field access requires a `SCAN` marker (unpromoted/unindexed)
//! - Which SQL expression to emit for a field reference

use rusqlite::Connection;
use serde::Deserialize;
use std::collections::HashMap;
use uuid::Uuid;

use crate::meta::MetaStore;

/// How a single field is physically accessed.
#[derive(Debug, Clone)]
pub enum FieldAccess {
  /// Field lives in a promoted column on the schema data table.
  /// Access via `{table_alias}.{column}`.
  Promoted { table: String, column: String },
  /// Field has a JSONB expression index (e.g., `CREATE INDEX ... ON ... (json_extract(...))`).
  /// The planner can use this for index lookups, but the value still comes from JSONB.
  Indexed {
    index_name: String,
    json_path: String,
  },
  /// Field is stored only in the JSONB blob. Access requires `json_extract`
  /// and a `SCAN` marker in the query source.
  Unpromoted { json_path: String },
}

impl FieldAccess {
  /// Does this access method require a SCAN marker for predicate use?
  pub fn requires_scan(&self) -> bool {
    matches!(self, FieldAccess::Unpromoted { .. })
  }

  /// The SQL expression to read this field's value.
  /// `node_alias` is the alias for the `nodes` table (for JSONB extraction).
  /// `schema_alias` is the alias for the schema data table (for promoted columns).
  pub fn sql_value_expr(&self, node_alias: &str, schema_alias: &str) -> String {
    match self {
      FieldAccess::Promoted { column, .. } => format!("{}.{}", schema_alias, column),
      FieldAccess::Indexed { json_path, .. } | FieldAccess::Unpromoted { json_path } => {
        format!(
          "json_extract({}.fields_json, '$.\"{json_path}\".value')",
          node_alias,
          json_path = json_path
        )
      }
    }
  }
}

/// Resolved physical schema for a single `CONFORMS TO schema(X)` reference.
#[derive(Debug, Clone)]
pub struct PhysicalSchema {
  /// The physical table name, e.g. `"schema_data_abc123"`.
  pub table_name: String,
  /// The storage mode from the meta table.
  pub storage_mode: String,
  /// Map from logical field name (e.g. `"title"`) to its physical access strategy.
  /// Fields not in this map are treated as `Unpromoted`.
  pub fields: HashMap<String, FieldAccess>,
  /// Map from logical field name to its declared type tag.
  pub field_types: HashMap<String, String>,
}

/// A single entry in the `field_mappings` JSON column.
#[derive(Debug, Clone, Deserialize)]
struct FieldMappingEntry {
  column: String,
  #[serde(rename = "type")]
  field_type: Option<String>,
  #[serde(default)]
  indexed: bool,
}

impl PhysicalSchema {
  /// Return the declared type tag for a field, if known from the schema.
  pub fn field_type(&self, field_name: &str) -> Option<&str> {
    self.field_types.get(field_name).map(|s| s.as_str())
  }

  /// Look up the access strategy for a field. Returns `Unpromoted` for unknown fields.
  pub fn field_access(&self, field_name: &str, ns: Option<&str>) -> FieldAccess {
    // Build the lookup key: for now, field names in the mapping are bare (no
    // namespace prefix). The namespace is implicit from the schema's owning app.
    if let Some(access) = self.fields.get(field_name) {
      return access.clone();
    }
    // Fallback: generate the json_path.  If a namespace is provided, use
    // "ns:field" format; otherwise just the bare field name.
    let json_path = match ns {
      Some(ns_str) => format!("{}:{}", ns_str, field_name),
      None => field_name.to_string(),
    };
    FieldAccess::Unpromoted { json_path }
  }
}

/// Resolve the physical schema for a given schema ID by consulting `schema_tables`.
///
/// Returns `None` if the schema is not registered in `schema_tables`.
/// When the storage mode is `jsonb`, all fields are `Unpromoted` but the
/// struct is still returned (the caller still needs the table name for
/// the `node_schema_conformance` join).
pub fn resolve_physical_schema(
  conn: &Connection,
  schema_id: &Uuid,
) -> Result<Option<PhysicalSchema>, String> {
  let st = match MetaStore::get_schema_table(conn, schema_id)
    .map_err(|e| format!("failed to query schema_tables: {}", e))?
  {
    Some(st) => st,
    None => return Ok(None),
  };

  // Parse field_mappings JSON
  let raw_mappings: HashMap<String, FieldMappingEntry> =
    serde_json::from_value(st.field_mappings.clone()).unwrap_or_default();

  let mut fields: HashMap<String, FieldAccess> = HashMap::new();
  let mut field_types: HashMap<String, String> = HashMap::new();

  // Also look up ready indexes for this schema
  let indexes = MetaStore::get_ready_indexes(conn, Some(schema_id))
    .map_err(|e| format!("failed to query managed_indexes: {}", e))?;

  for (field_name, entry) in &raw_mappings {
    let access = FieldAccess::Promoted {
      table: st.physical_table_name.clone(),
      column: entry.column.clone(),
    };
    fields.insert(field_name.clone(), access);
    if let Some(ref t) = entry.field_type {
      field_types.insert(field_name.clone(), t.clone());
    }
  }

  // For any fields that have expression indexes but are NOT promoted,
  // add Indexed entries.  The target_field column may hold a JSON array
  // for composite indexes, so we parse it and mark every component field.
  for idx in &indexes {
    let target_fields: Vec<String> = serde_json::from_str(&idx.target_field)
      .unwrap_or_else(|_| vec![idx.target_field.clone()]);

    for field in &target_fields {
      if !fields.contains_key(field) {
        let json_path = field.clone();
        fields.insert(
          field.clone(),
          FieldAccess::Indexed {
            index_name: idx.physical_index_name.clone(),
            json_path,
          },
        );
      }
    }
  }

  Ok(Some(PhysicalSchema {
    table_name: st.physical_table_name,
    storage_mode: st.storage_mode.as_str().to_string(),
    fields,
    field_types,
  }))
}

// ── Tests ───────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
  use super::*;
  use crate::meta::{IndexStatus, MetaStore, MigrationState, StorageMode};
  use rusqlite::Connection;

  fn setup() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    MetaStore::initialize(&conn).unwrap();
    conn
  }

  #[test]
  fn test_resolve_promoted_schema() {
    let conn = setup();
    let schema_id = Uuid::new_v4();

    let field_mappings = serde_json::json!({
      "title": {"column": "title_col", "type": "String", "indexed": false},
      "start_time": {"column": "start_col", "type": "DateTime", "indexed": true},
    });

    MetaStore::upsert_schema_table(
      &conn,
      &schema_id,
      "schema_data_abc",
      &field_mappings,
      StorageMode::Hybrid,
      MigrationState::Stable,
    )
    .unwrap();

    let ps = resolve_physical_schema(&conn, &schema_id).unwrap().unwrap();

    assert_eq!(ps.table_name, "schema_data_abc");
    assert_eq!(ps.storage_mode, "hybrid");

    // Promoted field
    match ps.field_access("title", None) {
      FieldAccess::Promoted { column, .. } => assert_eq!(column, "title_col"),
      other => panic!("expected Promoted, got {:?}", other),
    }

    // Unknown field → Unpromoted
    match ps.field_access("notes", Some("app")) {
      FieldAccess::Unpromoted { json_path } => assert_eq!(json_path, "app:notes"),
      other => panic!("expected Unpromoted, got {:?}", other),
    }
  }

  #[test]
  fn test_resolve_jsonb_schema_all_unpromoted() {
    let conn = setup();
    let schema_id = Uuid::new_v4();

    MetaStore::upsert_schema_table(
      &conn,
      &schema_id,
      "schema_data_jsonb",
      &serde_json::json!({}),
      StorageMode::Jsonb,
      MigrationState::Stable,
    )
    .unwrap();

    let ps = resolve_physical_schema(&conn, &schema_id).unwrap().unwrap();

    assert_eq!(ps.fields.len(), 0);
    // All fields should be Unpromoted
    let access = ps.field_access("anything", Some("ns"));
    assert!(access.requires_scan());
  }

  #[test]
  fn test_missing_schema_returns_none() {
    let conn = setup();
    let result = resolve_physical_schema(&conn, &Uuid::new_v4()).unwrap();
    assert!(result.is_none());
  }

  #[test]
  fn test_sql_value_expr_promoted() {
    let access = FieldAccess::Promoted {
      table: "schema_data_abc".into(),
      column: "title_col".into(),
    };
    assert_eq!(access.sql_value_expr("n", "sd"), "sd.title_col");
  }

  #[test]
  fn test_sql_value_expr_unpromoted() {
    let access = FieldAccess::Unpromoted {
      json_path: "app:title".into(),
    };
    assert_eq!(
      access.sql_value_expr("n", "sd"),
      "json_extract(n.fields_json, '$.\"app:title\".value')"
    );
  }

  #[test]
  fn test_composite_index_all_fields_marked_indexed() {
    let conn = setup();
    let schema_id = Uuid::new_v4();

    // Register schema table with no promoted fields (all jsonb)
    MetaStore::upsert_schema_table(
      &conn,
      &schema_id,
      "schema_data_jsonb",
      &serde_json::json!({}),
      StorageMode::Jsonb,
      MigrationState::Stable,
    )
    .unwrap();

    // Create a composite index entry with a JSON array target_field
    let index_id = Uuid::new_v4();
    MetaStore::create_index(
      &conn,
      &index_id,
      Some(&schema_id),
      &["first_name".to_string(), "last_name".to_string()],
      "unique",
      "idx_composite_name",
    )
    .unwrap();

    // Transition to ready so it's picked up
    MetaStore::transition_index_status(&conn, &index_id, IndexStatus::Ready).unwrap();

    let ps = resolve_physical_schema(&conn, &schema_id).unwrap().unwrap();

    // Both fields should be Indexed (not Unpromoted)
    match ps.field_access("first_name", None) {
      FieldAccess::Indexed { index_name, .. } => {
        assert_eq!(index_name, "idx_composite_name");
      }
      other => panic!("expected Indexed for first_name, got {:?}", other),
    }

    match ps.field_access("last_name", None) {
      FieldAccess::Indexed { index_name, .. } => {
        assert_eq!(index_name, "idx_composite_name");
      }
      other => panic!("expected Indexed for last_name, got {:?}", other),
    }

    // Neither should require SCAN
    assert!(!ps.field_access("first_name", None).requires_scan());
    assert!(!ps.field_access("last_name", None).requires_scan());

    // Unknown field still requires SCAN
    assert!(ps.field_access("unknown", None).requires_scan());
  }

  #[test]
  fn test_composite_index_with_promoted_fields_not_overwritten() {
    let conn = setup();
    let schema_id = Uuid::new_v4();

    // Register schema table with one promoted field
    MetaStore::upsert_schema_table(
      &conn,
      &schema_id,
      "schema_data_abc",
      &serde_json::json!({
        "first_name": {"column": "first_col", "type": "String"},
      }),
      StorageMode::Hybrid,
      MigrationState::Stable,
    )
    .unwrap();

    // Create an index that covers both the promoted and an unpromoted field
    let index_id = Uuid::new_v4();
    MetaStore::create_index(
      &conn,
      &index_id,
      Some(&schema_id),
      &["first_name".to_string(), "last_name".to_string()],
      "btree",
      "idx_name",
    )
    .unwrap();
    MetaStore::transition_index_status(&conn, &index_id, IndexStatus::Ready).unwrap();

    let ps = resolve_physical_schema(&conn, &schema_id).unwrap().unwrap();

    // first_name is promoted — should stay Promoted, not be overwritten by Indexed
    match ps.field_access("first_name", None) {
      FieldAccess::Promoted { column, .. } => assert_eq!(column, "first_col"),
      other => panic!("expected Promoted for first_name, got {:?}", other),
    }

    // last_name is not promoted — should be Indexed
    match ps.field_access("last_name", None) {
      FieldAccess::Indexed { .. } => {}
      other => panic!("expected Indexed for last_name, got {:?}", other),
    }
  }
}

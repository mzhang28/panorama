//! SQLite storage backend — rusqlite + r2d2, WAL mode, JSON fields.
//! Implements the [`StorageBackend`] trait directly.

use std::collections::HashMap;
use std::path::PathBuf;

use chrono::Utc;
use panorama_core::types::{AppManagedInfo, FieldValue, Node, SchemaRef};
use r2d2;
use rusqlite::{params, Connection, OpenFlags};
use uuid::Uuid;

use super::StorageBackend;
use crate::meta::MetaStore;
use crate::query::compiler::{compile_phase1, compile_phase2, CompiledQuery};

// ── Connection manager ──────────────────────────────────────────────────────

pub struct SqliteConnManager {
  path: PathBuf,
  flags: OpenFlags,
}

impl r2d2::ManageConnection for SqliteConnManager {
  type Connection = Connection;
  type Error = rusqlite::Error;

  fn connect(&self) -> Result<Self::Connection, Self::Error> {
    let conn = Connection::open_with_flags(&self.path, self.flags)?;
    conn.execute_batch("PRAGMA busy_timeout=5000").ok();
    Ok(conn)
  }

  fn is_valid(&self, conn: &mut Self::Connection) -> Result<(), Self::Error> {
    conn.execute_batch("SELECT 1")
  }

  fn has_broken(&self, _conn: &mut Self::Connection) -> bool {
    false
  }
}

pub type PooledConn = r2d2::PooledConnection<SqliteConnManager>;

// ── SqliteBackend ───────────────────────────────────────────────────────────

pub struct SqliteBackend {
  write_pool: r2d2::Pool<SqliteConnManager>,
  read_pool: r2d2::Pool<SqliteConnManager>,
  statement_cache: crate::query::cache::StatementCache,
}

impl SqliteBackend {
  pub fn new(data_dir: PathBuf) -> Self {
    std::fs::create_dir_all(&data_dir).ok();
    let db_path = data_dir.join("panorama.db");

    let write_mgr = SqliteConnManager {
      path: db_path.clone(),
      flags: OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
    };
    // Use READ_WRITE for the read pool as well.  SQLITE_OPEN_READ_ONLY
    // connections in WAL mode may not see recently committed writes (the
    // WAL file may not be checkpointed yet), causing queries to return
    // stale/empty results and namespace auto-registration to fail with
    // "attempt to write a readonly database".
    let read_mgr = SqliteConnManager {
      path: db_path.clone(),
      flags: OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
    };

    let write_pool = r2d2::Pool::builder().max_size(1).build(write_mgr).unwrap();
    let read_pool = r2d2::Pool::builder().max_size(8).build(read_mgr).unwrap();

    let conn = write_pool.get().unwrap();
    conn.execute_batch("PRAGMA journal_mode=WAL;").unwrap();
    conn
      .execute_batch(
        "CREATE TABLE IF NOT EXISTS nodes (
                id TEXT PRIMARY KEY,
                space_id TEXT NOT NULL,
                fields_json TEXT NOT NULL DEFAULT '{}',
                preferred_schemas_json TEXT NOT NULL DEFAULT '[]',
                app_managed_json TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_nodes_space ON nodes(space_id);
            CREATE INDEX IF NOT EXISTS idx_nodes_updated ON nodes(updated_at);
            CREATE INDEX IF NOT EXISTS idx_nodes_created ON nodes(created_at);",
      )
      .unwrap();
    MetaStore::initialize(&conn).unwrap();
    conn
      .execute_batch("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;")
      .ok();

    Self {
      write_pool,
      read_pool,
      statement_cache: crate::query::cache::StatementCache::new(),
    }
  }

  fn read_conn(&self) -> Result<PooledConn, String> {
    self.read_pool.get().map_err(|e| e.to_string())
  }

  fn write_conn(&self) -> Result<PooledConn, String> {
    self.write_pool.get().map_err(|e| e.to_string())
  }

  fn row_to_node(row: &rusqlite::Row) -> rusqlite::Result<Node> {
    let id_str: String = row.get(0)?;
    let space_id_str: String = row.get(1)?;
    let fields_json: String = row.get(2)?;
    let preferred_schemas_json: String = row.get(3)?;
    let app_managed_json: Option<String> = row.get(4)?;
    let created_at: String = row.get(5)?;
    let updated_at: String = row.get(6)?;

    let fields: HashMap<String, FieldValue> =
      serde_json::from_str(&fields_json).unwrap_or_default();
    let preferred_schemas: Vec<SchemaRef> =
      serde_json::from_str(&preferred_schemas_json).unwrap_or_default();
    let app_managed: Option<AppManagedInfo> =
      app_managed_json.and_then(|j| serde_json::from_str(&j).ok());

    Ok(Node {
      id: Uuid::parse_str(&id_str).unwrap_or_else(|_| Uuid::new_v4()),
      fields,
      space_id: Uuid::parse_str(&space_id_str).unwrap_or_else(|_| Uuid::nil()),
      preferred_schemas,
      app_managed,
      created_at: chrono::DateTime::parse_from_rfc3339(&created_at)
        .map(|d| d.with_timezone(&chrono::Utc))
        .unwrap_or_else(|_| Utc::now()),
      updated_at: chrono::DateTime::parse_from_rfc3339(&updated_at)
        .map(|d| d.with_timezone(&chrono::Utc))
        .unwrap_or_else(|_| Utc::now()),
    })
  }

  /// Execute a compiled query with type-aware row deserialization (§1.5).
  ///
  /// Uses `row.get_ref(i)` to inspect the underlying SQLite data type instead
  /// of fetching every column as a String and running it through `serde_json::from_str`.
  /// Only JSON-parse columns that end with `_json`; everything else uses
  /// native type conversion.
  fn execute_compiled(&self, compiled: &CompiledQuery) -> Result<Vec<serde_json::Value>, String> {
    use rusqlite::types::ValueRef;

    let conn = self.read_conn()?;
    let mut stmt = conn
      .prepare(&compiled.sql)
      .map_err(|e| format!("prepare: {}", e))?;
    let cols: Vec<String> = stmt.column_names().iter().map(|c| c.to_string()).collect();
    let params_refs: Vec<&dyn rusqlite::types::ToSql> = compiled
      .params
      .iter()
      .map(|p| p as &dyn rusqlite::types::ToSql)
      .collect();

    let rows = stmt
      .query_map(params_refs.as_slice(), |row| {
        let mut obj = serde_json::Map::new();
        for (i, col) in cols.iter().enumerate() {
          let val = match row.get_ref(i) {
            Ok(ValueRef::Null) => serde_json::Value::Null,
            Ok(ValueRef::Integer(i)) => serde_json::Value::Number(i.into()),
            Ok(ValueRef::Real(f)) => serde_json::Number::from_f64(f)
              .map(serde_json::Value::Number)
              .unwrap_or(serde_json::Value::Null),
            Ok(ValueRef::Text(bytes)) => {
              let s = std::str::from_utf8(bytes).unwrap_or("");
              if col.ends_with("_json") {
                serde_json::from_str(s).unwrap_or_else(|_| serde_json::Value::String(s.to_string()))
              } else {
                serde_json::Value::String(s.to_string())
              }
            }
            Ok(ValueRef::Blob(_)) => serde_json::Value::Null,
            Err(_) => serde_json::Value::Null,
          };
          obj.insert(col.clone(), val);
        }
        if let Some(v) = obj.get("fields_json").cloned() {
          obj.insert("fields".to_string(), v);
        }
        if let Some(v) = obj.get("preferred_schemas_json").cloned() {
          obj.insert("preferred_schemas".to_string(), v);
        }
        if let Some(v) = obj.get("app_managed_json").cloned() {
          obj.insert("app_managed".to_string(), v);
        }
        if cols.len() == 1 && cols[0] == "n" {
          if let Some(val) = obj.remove("n") {
            return Ok(val);
          }
        }
        Ok(serde_json::Value::Object(obj))
      })
      .map_err(|e| format!("exec: {}", e))?;

    Ok(rows.flatten().collect())
  }
}

// ── StorageBackend impl ─────────────────────────────────────────────────────

impl StorageBackend for SqliteBackend {
  /// Query with statement caching by IR shape (§1.4 + §7.3).
  ///
  /// On cache hit: reuses both the cached SQL template AND the cached
  /// `CompileCtx` (Phase 1 result).  Calls `compile_phase2` directly to
  /// regenerate params, skipping the expensive meta-table lookups.
  /// On cache miss: runs Phase 1 + Phase 2, then caches both.
  fn query(&self, pql: &str) -> Result<Vec<serde_json::Value>, String> {
    let ast = panorama_core::query::parse_query(pql).map_err(|e| format!("parse: {}", e))?;
    let ir = panorama_core::query::ir::lower_to_ir(&ast);
    let cache_key = ir.cache_key();

    // Cache hit with Phase-1 context: skip meta-table lookups (§1.4).
    if let Some((cached_sql, Some(cached_ctx))) = self.statement_cache.get_sql(cache_key) {
      let query_id = uuid::Uuid::new_v4();
      let conn = self.read_conn()?;
      let mut compiled =
        compile_phase2(&ast, &conn, cached_ctx, query_id).map_err(|e| format!("compile: {}", e))?;
      drop(conn);
      compiled.sql = cached_sql;
      return self.execute_compiled(&compiled);
    }

    // Cache miss: full compilation, then store both SQL and Phase-1 context.
    let conn = self.read_conn()?;
    let ctx = compile_phase1(&ast, &conn).map_err(|e| format!("compile: {}", e))?;
    let query_id = uuid::Uuid::new_v4();
    let compiled =
      compile_phase2(&ast, &conn, ctx.clone(), query_id).map_err(|e| format!("compile: {}", e))?;
    drop(conn);

    self
      .statement_cache
      .insert_sql(cache_key, compiled.sql.clone(), ctx);

    self.execute_compiled(&compiled)
  }

  fn create_node(&self, node: Node) -> Result<Node, String> {
    let mut results = self.create_batch(vec![node])?;
    Ok(results.remove(0))
  }

  fn create_batch(&self, nodes: Vec<Node>) -> Result<Vec<Node>, String> {
    if nodes.is_empty() {
      return Ok(Vec::new());
    }
    let conn = self.write_conn()?;
    conn.execute_batch("BEGIN").map_err(|e| e.to_string())?;

    let result = (|| -> Result<(), String> {
      for node in &nodes {
        let fields_json = serde_json::to_string(&node.fields).map_err(|e| e.to_string())?;
        let schemas_json =
          serde_json::to_string(&node.preferred_schemas).map_err(|e| e.to_string())?;
        let app_managed_json = node
          .app_managed
          .as_ref()
          .map(|a| serde_json::to_string(a).unwrap_or_default());

        conn.execute(
                    "INSERT INTO nodes (id, space_id, fields_json, preferred_schemas_json, app_managed_json, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![node.id.to_string(), node.space_id.to_string(), fields_json, schemas_json, app_managed_json,
                            node.created_at.to_rfc3339(), node.updated_at.to_rfc3339()],
                ).map_err(|e| format!("Insert failed: {}", e))?;

        MetaStore::sync_field_presence(&conn, &node.id, &node.fields)
          .map_err(|e| format!("field_presence sync: {}", e))?;
        MetaStore::sync_schema_conformance(&conn, &node.id, &node.preferred_schemas)
          .map_err(|e| format!("schema_conformance sync: {}", e))?;
      }
      Ok(())
    })();

    match result {
      Ok(()) => {
        conn.execute_batch("COMMIT").map_err(|e| e.to_string())?;
        Ok(nodes)
      }
      Err(e) => {
        conn.execute_batch("ROLLBACK").ok();
        Err(e)
      }
    }
  }

  fn get_node(&self, id: Uuid) -> Result<Option<Node>, String> {
    let conn = self.read_conn()?;
    Ok(conn.query_row(
            "SELECT id, space_id, fields_json, preferred_schemas_json, app_managed_json, created_at, updated_at FROM nodes WHERE id = ?1",
            params![id.to_string()],
            Self::row_to_node,
        ).ok())
  }

  fn update_node(&self, id: Uuid, fields: HashMap<String, FieldValue>) -> Result<Node, String> {
    let conn = self.write_conn()?;
    conn.execute_batch("BEGIN").map_err(|e| e.to_string())?;

    let result = (|| -> Result<Node, String> {
      let mut node = conn.query_row(
                "SELECT id, space_id, fields_json, preferred_schemas_json, app_managed_json, created_at, updated_at FROM nodes WHERE id = ?1",
                params![id.to_string()], Self::row_to_node,
            ).map_err(|e| format!("Node not found: {}", e))?;

      for (key, value) in fields {
        node.set_field(&key, value);
      }
      let fields_json = serde_json::to_string(&node.fields).map_err(|e| e.to_string())?;
      conn
        .execute(
          "UPDATE nodes SET fields_json = ?1, updated_at = ?2 WHERE id = ?3",
          params![fields_json, node.updated_at.to_rfc3339(), id.to_string()],
        )
        .map_err(|e| e.to_string())?;

      MetaStore::sync_field_presence(&conn, &node.id, &node.fields)
        .map_err(|e| format!("field_presence sync: {}", e))?;
      MetaStore::sync_schema_conformance(&conn, &node.id, &node.preferred_schemas)
        .map_err(|e| format!("schema_conformance sync: {}", e))?;
      Ok(node)
    })();

    match result {
      Ok(node) => {
        conn.execute_batch("COMMIT").map_err(|e| e.to_string())?;
        Ok(node)
      }
      Err(e) => {
        conn.execute_batch("ROLLBACK").ok();
        Err(e)
      }
    }
  }

  fn delete_node(&self, id: Uuid) -> Result<(), String> {
    let conn = self.write_conn()?;
    conn.execute_batch("BEGIN").map_err(|e| e.to_string())?;
    let result = (|| -> Result<(), String> {
      let affected = conn
        .execute("DELETE FROM nodes WHERE id = ?1", params![id.to_string()])
        .map_err(|e| e.to_string())?;
      if affected == 0 {
        return Err(format!("Node {} not found", id));
      }
      MetaStore::remove_field_presence(&conn, &id).map_err(|e| format!("fp cleanup: {}", e))?;
      MetaStore::remove_schema_conformance(&conn, &id).map_err(|e| format!("sc cleanup: {}", e))?;
      Ok(())
    })();
    match result {
      Ok(()) => {
        conn.execute_batch("COMMIT").map_err(|e| e.to_string())?;
        Ok(())
      }
      Err(e) => {
        conn.execute_batch("ROLLBACK").ok();
        Err(e)
      }
    }
  }

  fn update_schemas(&self, id: Uuid, schemas: Vec<SchemaRef>) -> Result<Node, String> {
    let conn = self.write_conn()?;
    conn.execute_batch("BEGIN").map_err(|e| e.to_string())?;
    let result = (|| -> Result<Node, String> {
      let mut node = conn.query_row(
                "SELECT id, space_id, fields_json, preferred_schemas_json, app_managed_json, created_at, updated_at FROM nodes WHERE id = ?1",
                params![id.to_string()], Self::row_to_node,
            ).map_err(|e| format!("Node not found: {}", e))?;
      node.preferred_schemas = schemas;
      let schemas_json =
        serde_json::to_string(&node.preferred_schemas).map_err(|e| e.to_string())?;
      conn
        .execute(
          "UPDATE nodes SET preferred_schemas_json = ?1, updated_at = ?2 WHERE id = ?3",
          params![schemas_json, Utc::now().to_rfc3339(), id.to_string()],
        )
        .map_err(|e| e.to_string())?;
      MetaStore::sync_schema_conformance(&conn, &node.id, &node.preferred_schemas)
        .map_err(|e| format!("sc sync: {}", e))?;
      Ok(node)
    })();
    match result {
      Ok(node) => {
        conn.execute_batch("COMMIT").map_err(|e| e.to_string())?;
        Ok(node)
      }
      Err(e) => {
        conn.execute_batch("ROLLBACK").ok();
        Err(e)
      }
    }
  }

  fn initialize(&self) -> Result<(), String> {
    Ok(())
  }

  fn resolve_ns(&self, identifier: &str) -> Result<i64, String> {
    let conn = self.read_conn()?;
    MetaStore::resolve_ns_id(&conn, identifier).map_err(|e| e.to_string())
  }

  fn has_ready_index(&self, _schema_id: &str, _field: &str) -> Result<bool, String> {
    Ok(false)
  }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
  use super::*;
  use std::collections::HashMap;
  use tempfile::TempDir;

  /// Regression test: mirror the Journal E2E test's create → query → query-again
  /// pattern.  The second query must return the same rows as the first.
  #[test]
  fn test_journal_list_pages_returns_data_after_repeated_query() {
    let dir = TempDir::new().unwrap();
    let be = SqliteBackend::new(dir.path().to_path_buf());

    // Step 1: create a node with Journal-typical fields (like POST /pages)
    let node_id = Uuid::new_v4();
    let mut fields = HashMap::new();
    fields.insert(
      "system:node_title".to_string(),
      FieldValue::String("Test Page".into()),
    );
    fields.insert(
      "system:node_time".to_string(),
      FieldValue::DateTime("2026-07-06T00:00:00Z".into()),
    );
    fields.insert(
      "journal:content".to_string(),
      FieldValue::String("Hello world".into()),
    );
    fields.insert("journal:deleted".to_string(), FieldValue::Boolean(false));
    fields.insert(
      "journal:journal_day".to_string(),
      FieldValue::String("2026-07-06".into()),
    );
    // Top-level page: no parent_id set

    let mut node = Node::new(Uuid::nil());
    node.id = node_id;
    for (k, v) in &fields {
      node.set_field(k, v.clone());
    }

    let created = be.create_node(node).unwrap();
    assert_eq!(created.id, node_id);

    // Step 2: update to add journal:page_id (like the plugin does post-create)
    let mut patch = HashMap::new();
    patch.insert("journal:page_id".to_string(), FieldValue::NodeRef(node_id));
    be.update_node(node_id, patch).unwrap();

    // Step 3: run the exact listPages query used by the Journal plugin
    let pql = r#"MATCH (n) IN space("default") WHERE HAS_FIELD(n, "journal", "content") AND SCAN(n.journal.parent_id IS NULL) RETURN n ORDER BY n.system.node_time DESC LIMIT 100"#;
    let rows1 = be.query(pql).unwrap();
    assert!(
      !rows1.is_empty(),
      "first query should return the created page"
    );
    assert_eq!(rows1.len(), 1);

    // Step 4: run it again — simulates navigating away and back
    let rows2 = be.query(pql).unwrap();
    assert!(
      !rows2.is_empty(),
      "second query should also return the page (re-navigation)"
    );
    assert_eq!(rows2.len(), 1, "second query should return same count");

    // keep dir alive until end of test
    drop(dir);
  }
}

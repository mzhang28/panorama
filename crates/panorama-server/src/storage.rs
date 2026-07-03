//! SQLite-based node storage.
//! Nodes are stored in a SQLite database with JSON fields for flexible querying.
//!
//! Integrates with the meta-table management protocol (QUERY_DESIGN.md §6):
//! every node write also updates `field_presence` and `node_schema_conformance`
//! in the same transaction.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use chrono::Utc;
use panorama_core::types::{FieldValue, Node};
use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::meta::MetaStore;

#[derive(Clone)]
pub struct NodeStorage {
    conn: Arc<std::sync::Mutex<Connection>>,
}

impl NodeStorage {
    // ── Connection access ─────────────────────────────────────────────────

    /// Returns a reference to the underlying connection lock for direct
    /// SQL access (query engine, migrations, etc.).
    pub fn raw_conn(&self) -> Result<std::sync::MutexGuard<'_, Connection>, String> {
        self.conn.lock().map_err(|e| e.to_string())
    }

    /// Execute a Panorama Query Language query and return JSON rows.
    pub fn query_lang(&self, query_string: &str) -> Result<Vec<serde_json::Value>, String> {
        use panorama_core::query::parse_query;
        use crate::query::compiler::compile;

        let ast = parse_query(query_string).map_err(|e| format!("parse: {}", e))?;
        let conn = self.raw_conn()?;
        let compiled = compile(&ast, &conn).map_err(|e| format!("compile: {}", e))?;
        let mut stmt = conn.prepare(&compiled.sql).map_err(|e| format!("prepare: {}", e))?;
        let column_names: Vec<String> = stmt.column_names().iter().map(|c| c.to_string()).collect();
        let params_refs: Vec<&dyn rusqlite::types::ToSql> = compiled.params.iter()
            .map(|p| p as &dyn rusqlite::types::ToSql).collect();

        let mut results = Vec::new();
        let rows = stmt.query_map(params_refs.as_slice(), |row| {
            let mut obj = serde_json::Map::new();
            for (i, col) in column_names.iter().enumerate() {
                let val: Result<String, _> = row.get(i);
                obj.insert(col.clone(), match val {
                    Ok(s) => serde_json::from_str(&s).unwrap_or(serde_json::Value::String(s)),
                    Err(_) => serde_json::Value::Null,
                });
            }
            Ok(serde_json::Value::Object(obj))
        }).map_err(|e| format!("exec: {}", e))?;

        for row in rows.flatten() { results.push(row); }
        Ok(results)
    }

    // ── Meta store access (for compiler) ───────────────────────────────────

    /// Run an operation against a locked connection for meta-table access.
    /// The compiler uses this during Phase 1 meta lookup.
    pub fn with_conn<F, T>(&self, f: F) -> Result<T, String>
    where
        F: FnOnce(&Connection) -> Result<T, String>,
    {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        f(&conn)
    }

    /// Run an operation with a mutable locked connection (for Stats updates etc).
    pub fn with_conn_mut<F, T>(&self, f: F) -> Result<T, String>
    where
        F: FnOnce(&Connection) -> Result<T, String>,
    {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        f(&conn)
    }
}

impl NodeStorage {
    pub fn new(data_dir: PathBuf) -> Self {
        std::fs::create_dir_all(&data_dir).ok();
        let db_path = data_dir.join("panorama.db");
        let conn = Connection::open(&db_path).expect("Failed to open SQLite database");

        // Create the main nodes table
        conn.execute_batch(
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
            CREATE INDEX IF NOT EXISTS idx_nodes_created ON nodes(created_at);"
        ).expect("Failed to create nodes table");

        // Create the six meta tables (idempotent)
        MetaStore::initialize(&conn).expect("Failed to initialize meta tables");

        // Enable WAL mode for better concurrent access
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;").ok();

        Self { conn: Arc::new(std::sync::Mutex::new(conn)) }
    }

    fn row_to_node(row: &rusqlite::Row) -> rusqlite::Result<Node> {
        let id_str: String = row.get(0)?;
        let space_id_str: String = row.get(1)?;
        let fields_json: String = row.get(2)?;
        let preferred_schemas_json: String = row.get(3)?;
        let app_managed_json: Option<String> = row.get(4)?;
        let created_at: String = row.get(5)?;
        let updated_at: String = row.get(6)?;

        let fields: HashMap<String, FieldValue> = serde_json::from_str(&fields_json).unwrap_or_default();
        let preferred_schemas: Vec<panorama_core::types::SchemaRef> = serde_json::from_str(&preferred_schemas_json).unwrap_or_default();
        let app_managed: Option<panorama_core::types::AppManagedInfo> = app_managed_json
            .and_then(|j| serde_json::from_str(&j).ok());

        Ok(Node {
            id: Uuid::parse_str(&id_str).unwrap_or_else(|_| Uuid::new_v4()),
            fields,
            space_id: Uuid::parse_str(&space_id_str).unwrap_or_else(|_| Uuid::nil()),
            preferred_schemas,
            app_managed,
            created_at: chrono::DateTime::parse_from_rfc3339(&created_at)
                .map(|d| d.with_timezone(&chrono::Utc))
                .unwrap_or_else(|_| chrono::Utc::now()),
            updated_at: chrono::DateTime::parse_from_rfc3339(&updated_at)
                .map(|d| d.with_timezone(&chrono::Utc))
                .unwrap_or_else(|_| chrono::Utc::now()),
        })
    }

    /// Create a node.  Wraps the INSERT + meta-table sync in a single
    /// transaction so field_presence and node_schema_conformance stay
    /// consistent with the node row.
    pub fn create(&self, node: Node) -> Result<Node, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let fields_json = serde_json::to_string(&node.fields).map_err(|e| e.to_string())?;
        let schemas_json = serde_json::to_string(&node.preferred_schemas).map_err(|e| e.to_string())?;
        let app_managed_json = node.app_managed.as_ref()
            .map(|a| serde_json::to_string(a).unwrap_or_default());

        conn.execute_batch("BEGIN").map_err(|e| e.to_string())?;

        let result = (|| -> Result<(), String> {
            conn.execute(
                "INSERT INTO nodes (id, space_id, fields_json, preferred_schemas_json, app_managed_json, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    node.id.to_string(),
                    node.space_id.to_string(),
                    fields_json,
                    schemas_json,
                    app_managed_json,
                    node.created_at.to_rfc3339(),
                    node.updated_at.to_rfc3339(),
                ],
            ).map_err(|e| format!("Insert failed: {}", e))?;

            // §6.2 write invariants — same transaction
            MetaStore::sync_field_presence(&conn, &node.id, &node.fields)
                .map_err(|e| format!("field_presence sync: {}", e))?;
            MetaStore::sync_schema_conformance(&conn, &node.id, &node.preferred_schemas)
                .map_err(|e| format!("schema_conformance sync: {}", e))?;

            Ok(())
        })();

        match result {
            Ok(()) => {
                conn.execute_batch("COMMIT").map_err(|e| e.to_string())?;
                Ok(node)
            }
            Err(e) => {
                conn.execute_batch("ROLLBACK").ok();
                Err(e)
            }
        }
    }

    pub fn get(&self, id: &Uuid) -> Option<Node> {
        let conn = self.conn.lock().ok()?;
        conn.query_row(
            "SELECT id, space_id, fields_json, preferred_schemas_json, app_managed_json, created_at, updated_at FROM nodes WHERE id = ?1",
            params![id.to_string()],
            Self::row_to_node,
        ).ok()
    }

    /// Update a node's fields.  Merges new fields into existing, then
    /// re-syncs field_presence and node_schema_conformance in a single
    /// transaction.
    pub fn update(&self, id: &Uuid, fields: HashMap<String, FieldValue>) -> Result<Node, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;

        conn.execute_batch("BEGIN").map_err(|e| e.to_string())?;

        let result = (|| -> Result<Node, String> {
            // Get existing node
            let mut node = conn.query_row(
                "SELECT id, space_id, fields_json, preferred_schemas_json, app_managed_json, created_at, updated_at FROM nodes WHERE id = ?1",
                params![id.to_string()],
                Self::row_to_node,
            ).map_err(|e| format!("Node not found: {}", e))?;

            // Merge fields
            for (key, value) in fields {
                node.set_field(&key, value);
            }

            let fields_json = serde_json::to_string(&node.fields).map_err(|e| e.to_string())?;

            conn.execute(
                "UPDATE nodes SET fields_json = ?1, updated_at = ?2 WHERE id = ?3",
                params![fields_json, node.updated_at.to_rfc3339(), id.to_string()],
            ).map_err(|e| e.to_string())?;

            // §6.2 write invariants — same transaction
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

    /// Delete a node.  Also cleans up meta-table records (field_presence,
    /// node_schema_conformance) in the same transaction.
    pub fn delete(&self, id: &Uuid) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;

        conn.execute_batch("BEGIN").map_err(|e| e.to_string())?;

        let result = (|| -> Result<(), String> {
            let affected = conn.execute(
                "DELETE FROM nodes WHERE id = ?1",
                params![id.to_string()],
            ).map_err(|e| e.to_string())?;
            if affected == 0 {
                return Err(format!("Node {} not found", id));
            }

            // Clean up meta records
            MetaStore::remove_field_presence(&conn, id)
                .map_err(|e| format!("field_presence cleanup: {}", e))?;
            MetaStore::remove_schema_conformance(&conn, id)
                .map_err(|e| format!("schema_conformance cleanup: {}", e))?;

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

    /// Update a node's preferred schemas (without changing fields).
    /// Re-syncs node_schema_conformance in a transaction.
    pub fn update_schemas(
        &self,
        id: &Uuid,
        schemas: Vec<panorama_core::types::SchemaRef>,
    ) -> Result<Node, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;

        conn.execute_batch("BEGIN").map_err(|e| e.to_string())?;

        let result = (|| -> Result<Node, String> {
            let mut node = conn.query_row(
                "SELECT id, space_id, fields_json, preferred_schemas_json, app_managed_json, created_at, updated_at FROM nodes WHERE id = ?1",
                params![id.to_string()],
                Self::row_to_node,
            ).map_err(|e| format!("Node not found: {}", e))?;

            node.preferred_schemas = schemas;
            let schemas_json = serde_json::to_string(&node.preferred_schemas).map_err(|e| e.to_string())?;

            conn.execute(
                "UPDATE nodes SET preferred_schemas_json = ?1, updated_at = ?2 WHERE id = ?3",
                params![schemas_json, Utc::now().to_rfc3339(), id.to_string()],
            ).map_err(|e| e.to_string())?;

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
}


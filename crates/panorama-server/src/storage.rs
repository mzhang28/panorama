//! SQLite-based node storage.
//! Nodes are stored in a SQLite database with JSON fields for flexible querying.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use panorama_core::types::{FieldValue, Node};
use rusqlite::{params, Connection};
use uuid::Uuid;

#[derive(Clone)]
pub struct NodeStorage {
    conn: Arc<std::sync::Mutex<Connection>>,
}

impl NodeStorage {
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
        let compiled = compile(&ast).map_err(|e| format!("compile: {}", e))?;
        let conn = self.raw_conn()?;
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
}

impl NodeStorage {
    pub fn new(data_dir: PathBuf) -> Self {
        std::fs::create_dir_all(&data_dir).ok();
        let db_path = data_dir.join("panorama.db");
        let conn = Connection::open(&db_path).expect("Failed to open SQLite database");

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
        ).expect("Failed to create tables");

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

    pub fn create(&self, node: Node) -> Result<Node, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let fields_json = serde_json::to_string(&node.fields).map_err(|e| e.to_string())?;
        let schemas_json = serde_json::to_string(&node.preferred_schemas).map_err(|e| e.to_string())?;
        let app_managed_json = node.app_managed.as_ref()
            .map(|a| serde_json::to_string(a).unwrap_or_default());

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

        Ok(node)
    }

    pub fn get(&self, id: &Uuid) -> Option<Node> {
        let conn = self.conn.lock().ok()?;
        conn.query_row(
            "SELECT id, space_id, fields_json, preferred_schemas_json, app_managed_json, created_at, updated_at FROM nodes WHERE id = ?1",
            params![id.to_string()],
            Self::row_to_node,
        ).ok()
    }

    pub fn update(&self, id: &Uuid, fields: HashMap<String, FieldValue>) -> Result<Node, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;

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

        Ok(node)
    }

    pub fn delete(&self, id: &Uuid) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let affected = conn.execute(
            "DELETE FROM nodes WHERE id = ?1",
            params![id.to_string()],
        ).map_err(|e| e.to_string())?;
        if affected == 0 {
            return Err(format!("Node {} not found", id));
        }
        Ok(())
    }

}

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

    pub fn query(
        &self,
        field_filters: &HashMap<String, FieldValue>,
        space_id: Option<Uuid>,
        limit: Option<usize>,
    ) -> Vec<Node> {
        let conn = match self.conn.lock() {
            Ok(c) => c,
            Err(_) => return vec![],
        };

        let mut sql = String::from(
            "SELECT id, space_id, fields_json, preferred_schemas_json, app_managed_json, created_at, updated_at FROM nodes WHERE 1=1"
        );
        let mut conditions: Vec<String> = Vec::new();

        // Space filter
        if let Some(sid) = space_id {
            conditions.push(format!("space_id = '{}'", sid));
        }

        // Field filters using JSON extraction
        for (key, value) in field_filters {
            let val_str = match value {
                FieldValue::String(s) => s.clone(),
                FieldValue::Integer(i) => i.to_string(),
                FieldValue::Float(f) => f.to_string(),
                FieldValue::Boolean(b) => b.to_string(),
                FieldValue::DateTime(s) => s.clone(),
                FieldValue::NodeRef(id) => id.to_string(),
                _ => continue,
            };
            conditions.push(format!(
                "json_extract(fields_json, '$.{}') = '{}'",
                key, val_str.replace('\'', "''")
            ));
        }

        if !conditions.is_empty() {
            sql.push_str(" AND ");
            sql.push_str(&conditions.join(" AND "));
        }

        sql.push_str(" ORDER BY updated_at DESC");

        if let Some(lim) = limit {
            sql.push_str(&format!(" LIMIT {}", lim));
        }

        let mut stmt = match conn.prepare(&sql) {
            Ok(s) => s,
            Err(_) => return vec![],
        };

        let nodes: Vec<Node> = stmt.query_map([], Self::row_to_node)
            .ok()
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default();

        nodes
    }

    pub fn all_in_space(&self, space_id: Uuid) -> Vec<Node> {
        self.query(&HashMap::new(), Some(space_id), None)
    }
}

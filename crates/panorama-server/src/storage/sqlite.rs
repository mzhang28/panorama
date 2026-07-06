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
use crate::query::compiler::{compile, CompiledQuery, ParamValue};

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

    fn has_broken(&self, _conn: &mut Self::Connection) -> bool { false }
}

pub type PooledConn = r2d2::PooledConnection<SqliteConnManager>;

// ── SqliteBackend ───────────────────────────────────────────────────────────

pub struct SqliteBackend {
    write_pool: r2d2::Pool<SqliteConnManager>,
    read_pool: r2d2::Pool<SqliteConnManager>,
}

impl SqliteBackend {
    pub fn new(data_dir: PathBuf) -> Self {
        std::fs::create_dir_all(&data_dir).ok();
        let db_path = data_dir.join("panorama.db");

        let write_mgr = SqliteConnManager {
            path: db_path.clone(),
            flags: OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
        };
        let read_mgr = SqliteConnManager {
            path: db_path,
            flags: OpenFlags::SQLITE_OPEN_READ_ONLY,
        };

        let write_pool = r2d2::Pool::builder().max_size(1).build(write_mgr).unwrap();
        let read_pool = r2d2::Pool::builder().max_size(8).build(read_mgr).unwrap();

        let conn = write_pool.get().unwrap();
        conn.execute_batch("PRAGMA journal_mode=WAL;").unwrap();
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
        ).unwrap();
        MetaStore::initialize(&conn).unwrap();
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;").ok();

        Self { write_pool, read_pool }
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

    fn execute_compiled(&self, compiled: &CompiledQuery) -> Result<Vec<serde_json::Value>, String> {
        let conn = self.read_conn()?;
        let mut stmt = conn.prepare(&compiled.sql).map_err(|e| format!("prepare: {}", e))?;
        let cols: Vec<String> = stmt.column_names().iter().map(|c| c.to_string()).collect();
        let params_refs: Vec<&dyn rusqlite::types::ToSql> =
            compiled.params.iter().map(|p| p as &dyn rusqlite::types::ToSql).collect();

        let rows = stmt.query_map(params_refs.as_slice(), |row| {
            let mut obj = serde_json::Map::new();
            for (i, col) in cols.iter().enumerate() {
                let val: Result<String, _> = row.get(i);
                obj.insert(col.clone(), match val {
                    Ok(s) => serde_json::from_str(&s).unwrap_or(serde_json::Value::String(s)),
                    Err(_) => serde_json::Value::Null,
                });
            }
            if let Some(v) = obj.get("fields_json").cloned() { obj.insert("fields".to_string(), v); }
            if let Some(v) = obj.get("preferred_schemas_json").cloned() { obj.insert("preferred_schemas".to_string(), v); }
            if let Some(v) = obj.get("app_managed_json").cloned() { obj.insert("app_managed".to_string(), v); }
            if cols.len() == 1 && cols[0] == "n" {
                if let Some(val) = obj.remove("n") { return Ok(val); }
            }
            Ok(serde_json::Value::Object(obj))
        }).map_err(|e| format!("exec: {}", e))?;

        Ok(rows.flatten().collect())
    }
}

// ── StorageBackend impl ─────────────────────────────────────────────────────

impl StorageBackend for SqliteBackend {
    fn query(&self, pql: &str) -> Result<Vec<serde_json::Value>, String> {
        let ast = panorama_core::query::parse_query(pql).map_err(|e| format!("parse: {}", e))?;
        let conn = self.read_conn()?;
        let compiled = compile(&ast, &conn).map_err(|e| format!("compile: {}", e))?;
        drop(conn);
        self.execute_compiled(&compiled)
    }

    fn create_node(&self, node: Node) -> Result<Node, String> {
        let mut results = self.create_batch(vec![node])?;
        Ok(results.remove(0))
    }

    fn create_batch(&self, nodes: Vec<Node>) -> Result<Vec<Node>, String> {
        if nodes.is_empty() { return Ok(Vec::new()); }
        let conn = self.write_conn()?;
        conn.execute_batch("BEGIN").map_err(|e| e.to_string())?;

        let result = (|| -> Result<(), String> {
            for node in &nodes {
                let fields_json = serde_json::to_string(&node.fields).map_err(|e| e.to_string())?;
                let schemas_json = serde_json::to_string(&node.preferred_schemas).map_err(|e| e.to_string())?;
                let app_managed_json = node.app_managed.as_ref()
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
            Ok(()) => { conn.execute_batch("COMMIT").map_err(|e| e.to_string())?; Ok(nodes) }
            Err(e) => { conn.execute_batch("ROLLBACK").ok(); Err(e) }
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

            for (key, value) in fields { node.set_field(&key, value); }
            let fields_json = serde_json::to_string(&node.fields).map_err(|e| e.to_string())?;
            conn.execute(
                "UPDATE nodes SET fields_json = ?1, updated_at = ?2 WHERE id = ?3",
                params![fields_json, node.updated_at.to_rfc3339(), id.to_string()],
            ).map_err(|e| e.to_string())?;

            MetaStore::sync_field_presence(&conn, &node.id, &node.fields)
                .map_err(|e| format!("field_presence sync: {}", e))?;
            MetaStore::sync_schema_conformance(&conn, &node.id, &node.preferred_schemas)
                .map_err(|e| format!("schema_conformance sync: {}", e))?;
            Ok(node)
        })();

        match result {
            Ok(node) => { conn.execute_batch("COMMIT").map_err(|e| e.to_string())?; Ok(node) }
            Err(e) => { conn.execute_batch("ROLLBACK").ok(); Err(e) }
        }
    }

    fn delete_node(&self, id: Uuid) -> Result<(), String> {
        let conn = self.write_conn()?;
        conn.execute_batch("BEGIN").map_err(|e| e.to_string())?;
        let result = (|| -> Result<(), String> {
            let affected = conn.execute("DELETE FROM nodes WHERE id = ?1", params![id.to_string()])
                .map_err(|e| e.to_string())?;
            if affected == 0 { return Err(format!("Node {} not found", id)); }
            MetaStore::remove_field_presence(&conn, &id).map_err(|e| format!("fp cleanup: {}", e))?;
            MetaStore::remove_schema_conformance(&conn, &id).map_err(|e| format!("sc cleanup: {}", e))?;
            Ok(())
        })();
        match result {
            Ok(()) => { conn.execute_batch("COMMIT").map_err(|e| e.to_string())?; Ok(()) }
            Err(e) => { conn.execute_batch("ROLLBACK").ok(); Err(e) }
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
            let schemas_json = serde_json::to_string(&node.preferred_schemas).map_err(|e| e.to_string())?;
            conn.execute(
                "UPDATE nodes SET preferred_schemas_json = ?1, updated_at = ?2 WHERE id = ?3",
                params![schemas_json, Utc::now().to_rfc3339(), id.to_string()],
            ).map_err(|e| e.to_string())?;
            MetaStore::sync_schema_conformance(&conn, &node.id, &node.preferred_schemas)
                .map_err(|e| format!("sc sync: {}", e))?;
            Ok(node)
        })();
        match result {
            Ok(node) => { conn.execute_batch("COMMIT").map_err(|e| e.to_string())?; Ok(node) }
            Err(e) => { conn.execute_batch("ROLLBACK").ok(); Err(e) }
        }
    }

    fn initialize(&self) -> Result<(), String> { Ok(()) }

    fn resolve_ns(&self, identifier: &str) -> Result<i64, String> {
        let conn = self.read_conn()?;
        MetaStore::resolve_ns_id(&conn, identifier).map_err(|e| e.to_string())
    }

    fn has_ready_index(&self, _schema_id: &str, _field: &str) -> Result<bool, String> {
        Ok(false)
    }
}

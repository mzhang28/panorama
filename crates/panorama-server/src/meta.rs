//! Meta table management protocol — implements QUERY_DESIGN.md §6.
//!
//! Six meta tables support the two-phase compilation described in §7:
//! 1. `namespaces`           — namespace registry (system, user, app)
//! 2. `schema_tables`        — logical schema → physical storage mapping
//! 3. `managed_indexes`      — index lifecycle tracking
//! 4. `field_presence`       — (ns_id, field_name, node_id) presence tracking
//! 5. `node_schema_conformance` — node → schema conformance with versioning
//! 6. `field_stats`          — batched field usage counters
//!
//! Write invariants (§6.2):
//! - Every field write updates `field_presence` in the same transaction.
//! - Every schema conformance change updates `node_schema_conformance` in the
//!   same transaction.
//! - `field_stats` is updated periodically (batched), not per-op.
//! - `managed_indexes.status` transitions govern whether the planner uses an index.

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

// ── Enum types ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum StorageMode {
    #[serde(rename = "promoted")]
    Promoted,
    #[serde(rename = "jsonb")]
    Jsonb,
    #[serde(rename = "hybrid")]
    Hybrid,
}

impl StorageMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            StorageMode::Promoted => "promoted",
            StorageMode::Jsonb => "jsonb",
            StorageMode::Hybrid => "hybrid",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "promoted" => StorageMode::Promoted,
            "hybrid" => StorageMode::Hybrid,
            _ => StorageMode::Jsonb,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MigrationState {
    #[serde(rename = "stable")]
    Stable,
    #[serde(rename = "migrating")]
    Migrating,
    #[serde(rename = "deprecated")]
    Deprecated,
}

impl MigrationState {
    pub fn as_str(&self) -> &'static str {
        match self {
            MigrationState::Stable => "stable",
            MigrationState::Migrating => "migrating",
            MigrationState::Deprecated => "deprecated",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "migrating" => MigrationState::Migrating,
            "deprecated" => MigrationState::Deprecated,
            _ => MigrationState::Stable,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum IndexStatus {
    #[serde(rename = "building")]
    Building,
    #[serde(rename = "ready")]
    Ready,
    #[serde(rename = "stale")]
    Stale,
    #[serde(rename = "dropped")]
    Dropped,
}

impl IndexStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            IndexStatus::Building => "building",
            IndexStatus::Ready => "ready",
            IndexStatus::Stale => "stale",
            IndexStatus::Dropped => "dropped",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "ready" => IndexStatus::Ready,
            "stale" => IndexStatus::Stale,
            "dropped" => IndexStatus::Dropped,
            _ => IndexStatus::Building,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum NamespaceKind {
    #[serde(rename = "system")]
    System,
    #[serde(rename = "user")]
    User,
    #[serde(rename = "app")]
    App,
}

impl NamespaceKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            NamespaceKind::System => "system",
            NamespaceKind::User => "user",
            NamespaceKind::App => "app",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "system" => NamespaceKind::System,
            "user" => NamespaceKind::User,
            _ => NamespaceKind::App,
        }
    }
}

// ── Row types ─────────────────────────────────────────────────────────────────

/// A row in the `namespaces` table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Namespace {
    pub ns_id: i64,
    pub kind: NamespaceKind,
    pub app_id: Option<String>,
    pub stable_identifier: String,
}

/// A row in the `schema_tables` table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaTable {
    pub schema_id: Uuid,
    pub physical_table_name: String,
    /// JSON-encoded field mappings: { logical_field → {column, type, indexed?} }
    pub field_mappings: serde_json::Value,
    pub storage_mode: StorageMode,
    pub migration_state: MigrationState,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A row in the `managed_indexes` table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagedIndex {
    pub index_id: Uuid,
    pub target_schema_id: Option<Uuid>,
    pub target_field: String,
    pub index_type: String,
    pub physical_index_name: String,
    pub status: IndexStatus,
    pub created_at: DateTime<Utc>,
}

/// A row in the `field_presence` table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldPresence {
    pub ns_id: i64,
    pub field_name: String,
    pub node_id: Uuid,
    pub value_type: Option<String>,
}

/// A row in the `node_schema_conformance` table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeSchemaConformance {
    pub node_id: Uuid,
    pub schema_id: Uuid,
    pub version_major: u32,
    pub version_minor: u32,
}

/// A row in the `field_stats` table (batched updates).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldStat {
    pub ns_id: i64,
    pub field_name: String,
    pub read_count: i64,
    pub write_count: i64,
    pub scan_count: i64,
    pub order_by_count: i64,
    pub updated_at: DateTime<Utc>,
}

// ── Predefined system namespace IDs ───────────────────────────────────────────

pub const SYSTEM_NS_ID: i64 = 1;
pub const USER_NS_ID: i64 = 2;

// ── MetaStore ─────────────────────────────────────────────────────────────────

/// Stateless namespace for meta-table operations.
///
/// All methods take a `&Connection` so they can be called within whatever
/// transaction `NodeStorage` has open.  The `MetaStore` itself holds no
/// connection of its own — it is purely a set of SQL templates.
pub struct MetaStore;

impl MetaStore {
    // ── Initialization ────────────────────────────────────────────────────────

    /// Create all six meta tables (idempotent — `IF NOT EXISTS`).
    pub fn initialize(conn: &Connection) -> Result<(), rusqlite::Error> {
        conn.execute_batch(
            "
            -- Namespace registry: maps stable identifiers to ns_id
            CREATE TABLE IF NOT EXISTS namespaces (
                ns_id            INTEGER PRIMARY KEY AUTOINCREMENT,
                kind             TEXT NOT NULL CHECK(kind IN ('system', 'user', 'app')),
                app_id           TEXT,
                stable_identifier TEXT NOT NULL UNIQUE
            );

            -- Logical schema → physical storage table mapping
            CREATE TABLE IF NOT EXISTS schema_tables (
                schema_id            TEXT PRIMARY KEY,
                physical_table_name  TEXT NOT NULL,
                field_mappings       TEXT NOT NULL DEFAULT '{}',
                storage_mode         TEXT NOT NULL DEFAULT 'jsonb'
                                      CHECK(storage_mode IN ('promoted', 'jsonb', 'hybrid')),
                migration_state      TEXT NOT NULL DEFAULT 'stable'
                                      CHECK(migration_state IN ('stable', 'migrating', 'deprecated')),
                created_at           TEXT NOT NULL,
                updated_at           TEXT NOT NULL
            );

            -- Index lifecycle tracking
            CREATE TABLE IF NOT EXISTS managed_indexes (
                index_id             TEXT PRIMARY KEY,
                target_schema_id     TEXT,
                target_field         TEXT NOT NULL,
                index_type           TEXT NOT NULL DEFAULT 'btree'
                                      CHECK(index_type IN ('btree', 'unique', 'fts', 'expr_jsonb')),
                physical_index_name  TEXT NOT NULL,
                status               TEXT NOT NULL DEFAULT 'building'
                                      CHECK(status IN ('building', 'ready', 'stale', 'dropped')),
                created_at           TEXT NOT NULL
            );

            -- Field presence: which nodes have which fields
            CREATE TABLE IF NOT EXISTS field_presence (
                ns_id        INTEGER NOT NULL,
                field_name   TEXT NOT NULL,
                node_id      TEXT NOT NULL,
                value_type   TEXT,
                PRIMARY KEY (ns_id, field_name, node_id)
            );
            CREATE INDEX IF NOT EXISTS idx_field_presence_node
                ON field_presence(node_id);
            CREATE INDEX IF NOT EXISTS idx_field_presence_ns_field
                ON field_presence(ns_id, field_name);

            -- Node → schema conformance with version info
            CREATE TABLE IF NOT EXISTS node_schema_conformance (
                node_id        TEXT NOT NULL,
                schema_id      TEXT NOT NULL,
                version_major  INTEGER NOT NULL DEFAULT 1,
                version_minor  INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (node_id, schema_id)
            );
            CREATE INDEX IF NOT EXISTS idx_nsc_schema_version
                ON node_schema_conformance(schema_id, version_major, node_id);

            -- Batched field usage statistics
            CREATE TABLE IF NOT EXISTS field_stats (
                ns_id          INTEGER NOT NULL,
                field_name     TEXT NOT NULL,
                read_count     INTEGER NOT NULL DEFAULT 0,
                write_count    INTEGER NOT NULL DEFAULT 0,
                scan_count     INTEGER NOT NULL DEFAULT 0,
                order_by_count INTEGER NOT NULL DEFAULT 0,
                updated_at     TEXT NOT NULL,
                PRIMARY KEY (ns_id, field_name)
            );
            ",
        )?;

        // Register system namespaces if not already present
        Self::ensure_system_namespaces(conn)?;

        Ok(())
    }

    fn ensure_system_namespaces(conn: &Connection) -> Result<(), rusqlite::Error> {
        // Use INSERT OR IGNORE so this is idempotent across restarts.
        // SQLite AUTOINCREMENT means ns_id 1 and 2 will be assigned on first
        // insert; OR IGNORE skips them on subsequent runs.
        conn.execute(
            "INSERT OR IGNORE INTO namespaces (ns_id, kind, stable_identifier) VALUES (?1, 'system', 'system')",
            params![SYSTEM_NS_ID],
        )?;
        conn.execute(
            "INSERT OR IGNORE INTO namespaces (ns_id, kind, stable_identifier) VALUES (?1, 'user', 'user')",
            params![USER_NS_ID],
        )?;
        Ok(())
    }

    // ── Namespace management ──────────────────────────────────────────────────

    /// Register a new namespace, returning its assigned `ns_id`.
    /// Returns the existing `ns_id` if the stable identifier is already known.
    pub fn register_namespace(
        conn: &Connection,
        kind: NamespaceKind,
        app_id: Option<&str>,
        stable_identifier: &str,
    ) -> Result<i64, rusqlite::Error> {
        // Try insert; if conflict on stable_identifier, return existing row id.
        let result = conn.execute(
            "INSERT OR IGNORE INTO namespaces (kind, app_id, stable_identifier) VALUES (?1, ?2, ?3)",
            params![kind.as_str(), app_id, stable_identifier],
        );
        match result {
            Ok(_) => {
                // If a row was inserted, last_insert_rowid gives us the new ns_id.
                // If ignored (duplicate), last_insert_rowid might be 0 — fall back to SELECT.
                let rowid = conn.last_insert_rowid();
                if rowid > 0 {
                    Ok(rowid)
                } else {
                    conn.query_row(
                        "SELECT ns_id FROM namespaces WHERE stable_identifier = ?1",
                        params![stable_identifier],
                        |row| row.get(0),
                    )
                }
            }
            Err(e) => Err(e),
        }
    }

    /// Look up a namespace by its integer ID.
    pub fn get_namespace(conn: &Connection, ns_id: i64) -> Result<Option<Namespace>, rusqlite::Error> {
        let mut stmt = conn.prepare(
            "SELECT ns_id, kind, app_id, stable_identifier FROM namespaces WHERE ns_id = ?1",
        )?;
        let mut rows = stmt.query_map(params![ns_id], |row| {
            let kind_str: String = row.get(1)?;
            Ok(Namespace {
                ns_id: row.get(0)?,
                kind: NamespaceKind::from_str(&kind_str),
                app_id: row.get(2)?,
                stable_identifier: row.get(3)?,
            })
        })?;
        match rows.next() {
            Some(Ok(ns)) => Ok(Some(ns)),
            Some(Err(e)) => Err(e),
            None => Ok(None),
        }
    }

    /// Look up a namespace by its stable identifier string.
    pub fn get_namespace_by_identifier(
        conn: &Connection,
        identifier: &str,
    ) -> Result<Option<Namespace>, rusqlite::Error> {
        let mut stmt = conn.prepare(
            "SELECT ns_id, kind, app_id, stable_identifier FROM namespaces WHERE stable_identifier = ?1",
        )?;
        let mut rows = stmt.query_map(params![identifier], |row| {
            let kind_str: String = row.get(1)?;
            Ok(Namespace {
                ns_id: row.get(0)?,
                kind: NamespaceKind::from_str(&kind_str),
                app_id: row.get(2)?,
                stable_identifier: row.get(3)?,
            })
        })?;
        match rows.next() {
            Some(Ok(ns)) => Ok(Some(ns)),
            Some(Err(e)) => Err(e),
            None => Ok(None),
        }
    }

    /// List all registered namespaces.
    pub fn list_namespaces(conn: &Connection) -> Result<Vec<Namespace>, rusqlite::Error> {
        let mut stmt = conn.prepare(
            "SELECT ns_id, kind, app_id, stable_identifier FROM namespaces ORDER BY ns_id",
        )?;
        let rows = stmt.query_map([], |row| {
            let kind_str: String = row.get(1)?;
            Ok(Namespace {
                ns_id: row.get(0)?,
                kind: NamespaceKind::from_str(&kind_str),
                app_id: row.get(2)?,
                stable_identifier: row.get(3)?,
            })
        })?;
        rows.collect()
    }

    /// Resolve a namespace string (e.g. "system", "user", or "com.example.app")
    /// to its `ns_id`.  Auto-registers unknown namespaces as `kind=app`.
    pub fn resolve_ns_id(conn: &Connection, namespace_str: &str) -> Result<i64, rusqlite::Error> {
        // Fast path for well-known system namespaces
        if namespace_str == "system" {
            return Ok(SYSTEM_NS_ID);
        }
        if namespace_str == "user" {
            return Ok(USER_NS_ID);
        }

        if let Some(ns) = Self::get_namespace_by_identifier(conn, namespace_str)? {
            return Ok(ns.ns_id);
        }

        // Auto-register as an app namespace
        Self::register_namespace(conn, NamespaceKind::App, None, namespace_str)
    }

    // ── Schema table management ───────────────────────────────────────────────

    /// Insert or update a schema → physical table mapping.
    pub fn upsert_schema_table(
        conn: &Connection,
        schema_id: &Uuid,
        physical_table_name: &str,
        field_mappings: &serde_json::Value,
        storage_mode: StorageMode,
        migration_state: MigrationState,
    ) -> Result<(), rusqlite::Error> {
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO schema_tables (schema_id, physical_table_name, field_mappings, storage_mode, migration_state, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)
             ON CONFLICT(schema_id) DO UPDATE SET
               physical_table_name = excluded.physical_table_name,
               field_mappings = excluded.field_mappings,
               storage_mode = excluded.storage_mode,
               migration_state = excluded.migration_state,
               updated_at = excluded.updated_at",
            params![
                schema_id.to_string(),
                physical_table_name,
                serde_json::to_string(field_mappings).unwrap_or_default(),
                storage_mode.as_str(),
                migration_state.as_str(),
                now,
            ],
        )?;
        Ok(())
    }

    /// Look up a schema table entry by schema ID.
    pub fn get_schema_table(conn: &Connection, schema_id: &Uuid) -> Result<Option<SchemaTable>, rusqlite::Error> {
        let mut stmt = conn.prepare(
            "SELECT schema_id, physical_table_name, field_mappings, storage_mode, migration_state, created_at, updated_at
             FROM schema_tables WHERE schema_id = ?1",
        )?;
        let mut rows = stmt.query_map(params![schema_id.to_string()], |row| {
            let fm_str: String = row.get(2)?;
            let sm_str: String = row.get(3)?;
            let ms_str: String = row.get(4)?;
            let ca_str: String = row.get(5)?;
            let ua_str: String = row.get(6)?;
            Ok(SchemaTable {
                schema_id: Uuid::parse_str(&row.get::<_, String>(0)?).unwrap_or_else(|_| Uuid::nil()),
                physical_table_name: row.get(1)?,
                field_mappings: serde_json::from_str(&fm_str).unwrap_or_default(),
                storage_mode: StorageMode::from_str(&sm_str),
                migration_state: MigrationState::from_str(&ms_str),
                created_at: DateTime::parse_from_rfc3339(&ca_str)
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                updated_at: DateTime::parse_from_rfc3339(&ua_str)
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
            })
        })?;
        match rows.next() {
            Some(Ok(st)) => Ok(Some(st)),
            Some(Err(e)) => Err(e),
            None => Ok(None),
        }
    }

    /// List all schema table entries.
    pub fn list_schema_tables(conn: &Connection) -> Result<Vec<SchemaTable>, rusqlite::Error> {
        let mut stmt = conn.prepare(
            "SELECT schema_id, physical_table_name, field_mappings, storage_mode, migration_state, created_at, updated_at
             FROM schema_tables ORDER BY schema_id",
        )?;
        let rows = stmt.query_map([], |row| {
            let fm_str: String = row.get(2)?;
            let sm_str: String = row.get(3)?;
            let ms_str: String = row.get(4)?;
            let ca_str: String = row.get(5)?;
            let ua_str: String = row.get(6)?;
            Ok(SchemaTable {
                schema_id: Uuid::parse_str(&row.get::<_, String>(0)?).unwrap_or_else(|_| Uuid::nil()),
                physical_table_name: row.get(1)?,
                field_mappings: serde_json::from_str(&fm_str).unwrap_or_default(),
                storage_mode: StorageMode::from_str(&sm_str),
                migration_state: MigrationState::from_str(&ms_str),
                created_at: DateTime::parse_from_rfc3339(&ca_str)
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                updated_at: DateTime::parse_from_rfc3339(&ua_str)
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
            })
        })?;
        rows.collect()
    }

    /// Get the physical table name for a schema (compiler helper).
    pub fn physical_table_for_schema(conn: &Connection, schema_id: &Uuid) -> Result<Option<String>, rusqlite::Error> {
        let mut stmt = conn.prepare(
            "SELECT physical_table_name FROM schema_tables WHERE schema_id = ?1",
        )?;
        let mut rows = stmt.query_map(params![schema_id.to_string()], |row| {
            row.get::<_, String>(0)
        })?;
        match rows.next() {
            Some(Ok(name)) => Ok(Some(name)),
            Some(Err(e)) => Err(e),
            None => Ok(None),
        }
    }

    // ── Managed indexes ───────────────────────────────────────────────────────

    /// Register a new index.  Initial status is `building` (not used by planner).
    pub fn create_index(
        conn: &Connection,
        index_id: &Uuid,
        target_schema_id: Option<&Uuid>,
        target_field: &str,
        index_type: &str,
        physical_index_name: &str,
    ) -> Result<(), rusqlite::Error> {
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO managed_indexes (index_id, target_schema_id, target_field, index_type, physical_index_name, status, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 'building', ?6)",
            params![
                index_id.to_string(),
                target_schema_id.map(|id| id.to_string()),
                target_field,
                index_type,
                physical_index_name,
                now,
            ],
        )?;
        Ok(())
    }

    /// Transition an index to a new status.
    ///
    /// Valid transitions (governed by this method, not a CHECK constraint
    /// in case we want to relax them later):
    /// - building → ready  (index build completed)
    /// - building → dropped (build failed or aborted)
    /// - ready    → stale  (underlying schema changed)
    /// - stale    → ready  (index rebuilt)
    /// - stale    → dropped (index removed)
    /// - ready    → dropped (manual removal)
    pub fn transition_index_status(
        conn: &Connection,
        index_id: &Uuid,
        new_status: IndexStatus,
    ) -> Result<(), rusqlite::Error> {
        let id = index_id.to_string();
        let current: String = conn.query_row(
            "SELECT status FROM managed_indexes WHERE index_id = ?1",
            params![id],
            |row| row.get(0),
        )?;

        // Validate the transition
        let valid = match (current.as_str(), &new_status) {
            ("building", IndexStatus::Ready) => true,
            ("building", IndexStatus::Dropped) => true,
            ("ready", IndexStatus::Stale) => true,
            ("ready", IndexStatus::Dropped) => true,
            ("stale", IndexStatus::Ready) => true,
            ("stale", IndexStatus::Dropped) => true,
            // Idempotent: allow re-setting the same status
            (cur, new) if cur == new.as_str() => true,
            _ => false,
        };

        if !valid {
            return Err(rusqlite::Error::ToSqlConversionFailure(
                Box::new(std::fmt::Error),
            ));
        }

        conn.execute(
            "UPDATE managed_indexes SET status = ?1 WHERE index_id = ?2",
            params![new_status.as_str(), id],
        )?;
        Ok(())
    }

    /// Get all indexes in `ready` state, optionally filtered by schema.
    /// The planner only uses indexes that are `ready`.
    pub fn get_ready_indexes(
        conn: &Connection,
        schema_id: Option<&Uuid>,
    ) -> Result<Vec<ManagedIndex>, rusqlite::Error> {
        let mut sql = String::from(
            "SELECT index_id, target_schema_id, target_field, index_type, physical_index_name, status, created_at
             FROM managed_indexes WHERE status = 'ready'",
        );
        let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if let Some(sid) = schema_id {
            sql.push_str(" AND target_schema_id = ?1");
            params_vec.push(Box::new(sid.to_string()));
        }

        let mut stmt = conn.prepare(&sql)?;
        let param_refs: Vec<&dyn rusqlite::types::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();
        let rows = stmt.query_map(param_refs.as_slice(), |row| {
            let sid_str: Option<String> = row.get(1)?;
            let status_str: String = row.get(5)?;
            let ca_str: String = row.get(6)?;
            Ok(ManagedIndex {
                index_id: Uuid::parse_str(&row.get::<_, String>(0)?).unwrap_or_else(|_| Uuid::nil()),
                target_schema_id: sid_str.and_then(|s| Uuid::parse_str(&s).ok()),
                target_field: row.get(2)?,
                index_type: row.get(3)?,
                physical_index_name: row.get(4)?,
                status: IndexStatus::from_str(&status_str),
                created_at: DateTime::parse_from_rfc3339(&ca_str)
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
            })
        })?;
        rows.collect()
    }

    /// Look up a single managed index by ID.
    pub fn get_index(conn: &Connection, index_id: &Uuid) -> Result<Option<ManagedIndex>, rusqlite::Error> {
        let mut stmt = conn.prepare(
            "SELECT index_id, target_schema_id, target_field, index_type, physical_index_name, status, created_at
             FROM managed_indexes WHERE index_id = ?1",
        )?;
        let mut rows = stmt.query_map(params![index_id.to_string()], |row| {
            let sid_str: Option<String> = row.get(1)?;
            let status_str: String = row.get(5)?;
            let ca_str: String = row.get(6)?;
            Ok(ManagedIndex {
                index_id: Uuid::parse_str(&row.get::<_, String>(0)?).unwrap_or_else(|_| Uuid::nil()),
                target_schema_id: sid_str.and_then(|s| Uuid::parse_str(&s).ok()),
                target_field: row.get(2)?,
                index_type: row.get(3)?,
                physical_index_name: row.get(4)?,
                status: IndexStatus::from_str(&status_str),
                created_at: DateTime::parse_from_rfc3339(&ca_str)
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
            })
        })?;
        match rows.next() {
            Some(Ok(idx)) => Ok(Some(idx)),
            Some(Err(e)) => Err(e),
            None => Ok(None),
        }
    }

    // ── Field presence ────────────────────────────────────────────────────────

    /// Synchronise `field_presence` for a single node.
    ///
    /// Called **within the same transaction** as the node INSERT/UPDATE.
    /// Deletes all existing presence rows for this node, then inserts the
    /// current set of fields.
    ///
    /// Each field key is expected to be in `"namespace:field_name"` format.
    /// Unknown namespaces are auto-registered as `kind=app`.
    pub fn sync_field_presence(
        conn: &Connection,
        node_id: &Uuid,
        fields: &HashMap<String, panorama_core::types::FieldValue>,
    ) -> Result<(), rusqlite::Error> {
        let nid = node_id.to_string();

        // Delete old presence for this node
        conn.execute("DELETE FROM field_presence WHERE node_id = ?1", params![nid])?;

        // Insert current fields
        for (key, value) in fields {
            // Parse "namespace:field_name"
            if let Some((ns_str, field_name)) = key.split_once(':') {
                let ns_id = Self::resolve_ns_id(conn, ns_str)?;
                let value_type = Self::value_type_tag(value);
                conn.execute(
                    "INSERT INTO field_presence (ns_id, field_name, node_id, value_type) VALUES (?1, ?2, ?3, ?4)",
                    params![ns_id, field_name, nid, value_type],
                )?;
            }
        }

        Ok(())
    }

    /// Check whether a specific node has a given field.
    pub fn has_field(
        conn: &Connection,
        ns_id: i64,
        field_name: &str,
        node_id: &Uuid,
    ) -> Result<bool, rusqlite::Error> {
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM field_presence WHERE ns_id = ?1 AND field_name = ?2 AND node_id = ?3",
            params![ns_id, field_name, node_id.to_string()],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    /// Get all node IDs that have a given field in any namespace.
    pub fn get_nodes_with_field_any_ns(
        conn: &Connection,
        field_name: &str,
    ) -> Result<Vec<Uuid>, rusqlite::Error> {
        let mut stmt = conn.prepare(
            "SELECT DISTINCT node_id FROM field_presence WHERE field_name = ?1",
        )?;
        let rows = stmt.query_map(params![field_name], |row| {
            let s: String = row.get(0)?;
            Ok(Uuid::parse_str(&s).unwrap_or_else(|_| Uuid::nil()))
        })?;
        rows.collect()
    }

    /// Get all node IDs that have a given (ns_id, field_name) pair.
    pub fn get_nodes_with_field(
        conn: &Connection,
        ns_id: i64,
        field_name: &str,
    ) -> Result<Vec<Uuid>, rusqlite::Error> {
        let mut stmt = conn.prepare(
            "SELECT node_id FROM field_presence WHERE ns_id = ?1 AND field_name = ?2",
        )?;
        let rows = stmt.query_map(params![ns_id, field_name], |row| {
            let s: String = row.get(0)?;
            Ok(Uuid::parse_str(&s).unwrap_or_else(|_| Uuid::nil()))
        })?;
        rows.collect()
    }

    /// Remove all field_presence records for a node (called on node delete).
    pub fn remove_field_presence(conn: &Connection, node_id: &Uuid) -> Result<(), rusqlite::Error> {
        conn.execute(
            "DELETE FROM field_presence WHERE node_id = ?1",
            params![node_id.to_string()],
        )?;
        Ok(())
    }

    /// Return the `FieldValue` serde tag string for a given value.
    fn value_type_tag(value: &panorama_core::types::FieldValue) -> &'static str {
        match value {
            panorama_core::types::FieldValue::String(_) => "String",
            panorama_core::types::FieldValue::Integer(_) => "Integer",
            panorama_core::types::FieldValue::Float(_) => "Float",
            panorama_core::types::FieldValue::Boolean(_) => "Boolean",
            panorama_core::types::FieldValue::DateTime(_) => "DateTime",
            panorama_core::types::FieldValue::Array(_) => "Array",
            panorama_core::types::FieldValue::NodeRef(_) => "NodeRef",
            panorama_core::types::FieldValue::Json(_) => "Json",
            panorama_core::types::FieldValue::ObjectRef(_) => "ObjectRef",
            panorama_core::types::FieldValue::Binary(_) => "Binary",
        }
    }

    // ── Node schema conformance ───────────────────────────────────────────────

    /// Synchronise `node_schema_conformance` for a single node.
    ///
    /// Called **within the same transaction** as the node INSERT/UPDATE.
    /// Deletes all existing conformance rows for this node, then inserts
    /// the current set from `preferred_schemas`.
    pub fn sync_schema_conformance(
        conn: &Connection,
        node_id: &Uuid,
        schemas: &[panorama_core::types::SchemaRef],
    ) -> Result<(), rusqlite::Error> {
        let nid = node_id.to_string();

        // Delete old conformance for this node
        conn.execute(
            "DELETE FROM node_schema_conformance WHERE node_id = ?1",
            params![nid],
        )?;

        // Insert current
        for sref in schemas {
            conn.execute(
                "INSERT INTO node_schema_conformance (node_id, schema_id, version_major, version_minor)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    nid,
                    sref.schema_node_id.to_string(),
                    sref.version.major,
                    sref.version.minor,
                ],
            )?;
        }

        Ok(())
    }

    /// Remove all conformance records for a node (called on node delete).
    pub fn remove_schema_conformance(conn: &Connection, node_id: &Uuid) -> Result<(), rusqlite::Error> {
        conn.execute(
            "DELETE FROM node_schema_conformance WHERE node_id = ?1",
            params![node_id.to_string()],
        )?;
        Ok(())
    }

    /// Get all node IDs that conform to a given schema, optionally with a
    /// minimum major version.
    pub fn get_conforming_nodes(
        conn: &Connection,
        schema_id: &Uuid,
        version_min: Option<u32>,
    ) -> Result<Vec<Uuid>, rusqlite::Error> {
        let sid = schema_id.to_string();
        if let Some(vmin) = version_min {
            let mut stmt = conn.prepare(
                "SELECT node_id FROM node_schema_conformance
                 WHERE schema_id = ?1 AND version_major >= ?2",
            )?;
            let rows = stmt.query_map(params![sid, vmin], |row| {
                let s: String = row.get(0)?;
                Ok(Uuid::parse_str(&s).unwrap_or_else(|_| Uuid::nil()))
            })?;
            rows.collect()
        } else {
            let mut stmt = conn.prepare(
                "SELECT node_id FROM node_schema_conformance WHERE schema_id = ?1",
            )?;
            let rows = stmt.query_map(params![sid], |row| {
                let s: String = row.get(0)?;
                Ok(Uuid::parse_str(&s).unwrap_or_else(|_| Uuid::nil()))
            })?;
            rows.collect()
        }
    }

    /// Check if a specific node conforms to a schema.
    pub fn node_conforms_to(
        conn: &Connection,
        node_id: &Uuid,
        schema_id: &Uuid,
    ) -> Result<bool, rusqlite::Error> {
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM node_schema_conformance WHERE node_id = ?1 AND schema_id = ?2",
            params![node_id.to_string(), schema_id.to_string()],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    /// Get all schema IDs that a node conforms to.
    pub fn get_node_schemas(
        conn: &Connection,
        node_id: &Uuid,
    ) -> Result<Vec<(Uuid, u32, u32)>, rusqlite::Error> {
        let mut stmt = conn.prepare(
            "SELECT schema_id, version_major, version_minor FROM node_schema_conformance WHERE node_id = ?1",
        )?;
        let rows = stmt.query_map(params![node_id.to_string()], |row| {
            let s: String = row.get(0)?;
            Ok((
                Uuid::parse_str(&s).unwrap_or_else(|_| Uuid::nil()),
                row.get(1)?,
                row.get(2)?,
            ))
        })?;
        rows.collect()
    }

    // ── Field stats (batched) ─────────────────────────────────────────────────

    /// Record field reads in bulk.  Uses INSERT ON CONFLICT UPSERT.
    /// Each entry is (ns_id, field_name, read_inc, scan_inc, order_by_inc).
    pub fn record_field_stats_batch(
        conn: &Connection,
        stats: &[(i64, &str, i64, i64, i64)],
    ) -> Result<(), rusqlite::Error> {
        let now = Utc::now().to_rfc3339();
        for (ns_id, field_name, read_inc, scan_inc, order_by_inc) in stats {
            conn.execute(
                "INSERT INTO field_stats (ns_id, field_name, read_count, write_count, scan_count, order_by_count, updated_at)
                 VALUES (?1, ?2, ?3, 0, ?4, ?5, ?6)
                 ON CONFLICT(ns_id, field_name) DO UPDATE SET
                   read_count = read_count + excluded.read_count,
                   scan_count = scan_count + excluded.scan_count,
                   order_by_count = order_by_count + excluded.order_by_count,
                   updated_at = excluded.updated_at",
                params![ns_id, field_name, read_inc, scan_inc, order_by_inc, now],
            )?;
        }
        Ok(())
    }

    /// Record a single field read.
    pub fn record_field_read(
        conn: &Connection,
        ns_id: i64,
        field_name: &str,
    ) -> Result<(), rusqlite::Error> {
        Self::record_field_stats_batch(conn, &[(ns_id, field_name, 1, 0, 0)])
    }

    /// Record a single field scan (unindexed access).
    pub fn record_field_scan(
        conn: &Connection,
        ns_id: i64,
        field_name: &str,
    ) -> Result<(), rusqlite::Error> {
        Self::record_field_stats_batch(conn, &[(ns_id, field_name, 0, 1, 0)])
    }

    /// Record a single ORDER BY on a field.
    pub fn record_field_order_by(
        conn: &Connection,
        ns_id: i64,
        field_name: &str,
    ) -> Result<(), rusqlite::Error> {
        Self::record_field_stats_batch(conn, &[(ns_id, field_name, 0, 0, 1)])
    }

    /// Record a field write.
    pub fn record_field_write(
        conn: &Connection,
        ns_id: i64,
        field_name: &str,
    ) -> Result<(), rusqlite::Error> {
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO field_stats (ns_id, field_name, read_count, write_count, scan_count, order_by_count, updated_at)
             VALUES (?1, ?2, 0, 1, 0, 0, ?3)
             ON CONFLICT(ns_id, field_name) DO UPDATE SET
               write_count = write_count + 1,
               updated_at = excluded.updated_at",
            params![ns_id, field_name, now],
        )?;
        Ok(())
    }

    /// Get stats for a specific field.
    pub fn get_field_stats(
        conn: &Connection,
        ns_id: i64,
        field_name: &str,
    ) -> Result<Option<FieldStat>, rusqlite::Error> {
        let mut stmt = conn.prepare(
            "SELECT ns_id, field_name, read_count, write_count, scan_count, order_by_count, updated_at
             FROM field_stats WHERE ns_id = ?1 AND field_name = ?2",
        )?;
        let mut rows = stmt.query_map(params![ns_id, field_name], |row| {
            let ua_str: String = row.get(6)?;
            Ok(FieldStat {
                ns_id: row.get(0)?,
                field_name: row.get(1)?,
                read_count: row.get(2)?,
                write_count: row.get(3)?,
                scan_count: row.get(4)?,
                order_by_count: row.get(5)?,
                updated_at: DateTime::parse_from_rfc3339(&ua_str)
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
            })
        })?;
        match rows.next() {
            Some(Ok(fs)) => Ok(Some(fs)),
            Some(Err(e)) => Err(e),
            None => Ok(None),
        }
    }

    /// Get all field stats, ordered by read_count descending.
    pub fn list_field_stats(conn: &Connection) -> Result<Vec<FieldStat>, rusqlite::Error> {
        let mut stmt = conn.prepare(
            "SELECT ns_id, field_name, read_count, write_count, scan_count, order_by_count, updated_at
             FROM field_stats ORDER BY read_count DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            let ua_str: String = row.get(6)?;
            Ok(FieldStat {
                ns_id: row.get(0)?,
                field_name: row.get(1)?,
                read_count: row.get(2)?,
                write_count: row.get(3)?,
                scan_count: row.get(4)?,
                order_by_count: row.get(5)?,
                updated_at: DateTime::parse_from_rfc3339(&ua_str)
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
            })
        })?;
        rows.collect()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn test_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        MetaStore::initialize(&conn).unwrap();
        conn
    }

    #[test]
    fn test_initialize_creates_tables() {
        let conn = test_conn();
        // Verify each table exists by querying sqlite_master
        let tables: Vec<String> = {
            let mut stmt = conn
                .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
                .unwrap();
            stmt.query_map([], |row| row.get(0))
                .unwrap()
                .filter_map(|r| r.ok())
                .collect()
        };
        assert!(tables.contains(&"namespaces".to_string()));
        assert!(tables.contains(&"schema_tables".to_string()));
        assert!(tables.contains(&"managed_indexes".to_string()));
        assert!(tables.contains(&"field_presence".to_string()));
        assert!(tables.contains(&"node_schema_conformance".to_string()));
        assert!(tables.contains(&"field_stats".to_string()));
    }

    #[test]
    fn test_system_namespaces_present() {
        let conn = test_conn();
        let sys = MetaStore::get_namespace(&conn, SYSTEM_NS_ID).unwrap().unwrap();
        assert_eq!(sys.kind, NamespaceKind::System);
        assert_eq!(sys.stable_identifier, "system");

        let usr = MetaStore::get_namespace(&conn, USER_NS_ID).unwrap().unwrap();
        assert_eq!(usr.kind, NamespaceKind::User);
        assert_eq!(usr.stable_identifier, "user");
    }

    #[test]
    fn test_resolve_ns_id_system() {
        let conn = test_conn();
        assert_eq!(MetaStore::resolve_ns_id(&conn, "system").unwrap(), 1);
        assert_eq!(MetaStore::resolve_ns_id(&conn, "user").unwrap(), 2);
    }

    #[test]
    fn test_resolve_ns_id_auto_register() {
        let conn = test_conn();
        let ns_id = MetaStore::resolve_ns_id(&conn, "com.example.app").unwrap();
        assert!(ns_id > 2);
        let ns = MetaStore::get_namespace(&conn, ns_id).unwrap().unwrap();
        assert_eq!(ns.kind, NamespaceKind::App);
        assert_eq!(ns.stable_identifier, "com.example.app");
    }

    #[test]
    fn test_resolve_ns_id_idempotent() {
        let conn = test_conn();
        let a = MetaStore::resolve_ns_id(&conn, "com.example.app").unwrap();
        let b = MetaStore::resolve_ns_id(&conn, "com.example.app").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn test_field_presence_sync() {
        let conn = test_conn();
        let node_id = Uuid::new_v4();
        let mut fields = HashMap::new();
        fields.insert(
            "system:node_title".to_string(),
            panorama_core::types::FieldValue::String("Hello".into()),
        );
        fields.insert(
            "com.example.app:score".to_string(),
            panorama_core::types::FieldValue::Integer(42),
        );

        MetaStore::sync_field_presence(&conn, &node_id, &fields).unwrap();

        // Verify presence
        let sys_ns = MetaStore::resolve_ns_id(&conn, "system").unwrap();
        assert!(MetaStore::has_field(&conn, sys_ns, "node_title", &node_id).unwrap());

        let app_ns = MetaStore::resolve_ns_id(&conn, "com.example.app").unwrap();
        assert!(MetaStore::has_field(&conn, app_ns, "score", &node_id).unwrap());
    }

    #[test]
    fn test_field_presence_sync_replaces_old() {
        let conn = test_conn();
        let node_id = Uuid::new_v4();

        // First sync: set title
        let mut fields1 = HashMap::new();
        fields1.insert(
            "system:node_title".to_string(),
            panorama_core::types::FieldValue::String("First".into()),
        );
        MetaStore::sync_field_presence(&conn, &node_id, &fields1).unwrap();

        // Second sync: replace with description only
        let mut fields2 = HashMap::new();
        fields2.insert(
            "system:node_description".to_string(),
            panorama_core::types::FieldValue::String("Desc".into()),
        );
        MetaStore::sync_field_presence(&conn, &node_id, &fields2).unwrap();

        let sys_ns = MetaStore::resolve_ns_id(&conn, "system").unwrap();
        assert!(!MetaStore::has_field(&conn, sys_ns, "node_title", &node_id).unwrap());
        assert!(MetaStore::has_field(&conn, sys_ns, "node_description", &node_id).unwrap());
    }

    #[test]
    fn test_schema_conformance_sync() {
        let conn = test_conn();
        let node_id = Uuid::new_v4();
        let schema_id = Uuid::new_v4();
        let schemas = vec![panorama_core::types::SchemaRef {
            schema_node_id: schema_id,
            version: panorama_core::types::SchemaVersion::new(1, 2),
        }];

        MetaStore::sync_schema_conformance(&conn, &node_id, &schemas).unwrap();

        let conforming = MetaStore::get_conforming_nodes(&conn, &schema_id, Some(1)).unwrap();
        assert!(conforming.contains(&node_id));
    }

    #[test]
    fn test_upsert_schema_table() {
        let conn = test_conn();
        let schema_id = Uuid::new_v4();
        let field_mappings = serde_json::json!({"title": {"column": "title", "type": "String", "indexed": false}});

        MetaStore::upsert_schema_table(
            &conn,
            &schema_id,
            "schema_data_abc",
            &field_mappings,
            StorageMode::Jsonb,
            MigrationState::Stable,
        )
        .unwrap();

        let st = MetaStore::get_schema_table(&conn, &schema_id).unwrap().unwrap();
        assert_eq!(st.physical_table_name, "schema_data_abc");
        assert_eq!(st.storage_mode, StorageMode::Jsonb);
    }

    #[test]
    fn test_managed_index_lifecycle() {
        let conn = test_conn();
        let index_id = Uuid::new_v4();
        let schema_id = Uuid::new_v4();

        MetaStore::create_index(
            &conn,
            &index_id,
            Some(&schema_id),
            "title",
            "btree",
            "idx_schema_abc_title",
        )
        .unwrap();

        let idx = MetaStore::get_index(&conn, &index_id).unwrap().unwrap();
        assert_eq!(idx.status, IndexStatus::Building);

        // Transition to ready
        MetaStore::transition_index_status(&conn, &index_id, IndexStatus::Ready).unwrap();
        let idx = MetaStore::get_index(&conn, &index_id).unwrap().unwrap();
        assert_eq!(idx.status, IndexStatus::Ready);

        // Should appear in ready indexes
        let ready = MetaStore::get_ready_indexes(&conn, Some(&schema_id)).unwrap();
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].index_id, index_id);
    }

    #[test]
    fn test_field_stats_batch() {
        let conn = test_conn();
        MetaStore::record_field_stats_batch(
            &conn,
            &[(1, "node_title", 5, 1, 2)],
        )
        .unwrap();

        let stats = MetaStore::get_field_stats(&conn, 1, "node_title").unwrap().unwrap();
        assert_eq!(stats.read_count, 5);
        assert_eq!(stats.scan_count, 1);
        assert_eq!(stats.order_by_count, 2);
    }

    #[test]
    fn test_initialize_is_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        MetaStore::initialize(&conn).unwrap();
        MetaStore::initialize(&conn).unwrap(); // second call should not error
    }
}

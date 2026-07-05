//! Property tests: the in-memory evaluator and the SQL compiler must agree.
//!
//! Strategy: generate random nodes, pick one of their fields to build a query
//! around, then assert `eval_query(nodes)` == `compile → SQLite`.

use panorama_core::query::{eval_query, parse_query};
use panorama_core::types::{FieldValue, Node};
use panorama_server::query::compiler::{compile, ParamValue};
use proptest::prelude::*;
use proptest::test_runner::Config as ProptestConfig;
use rusqlite::{Connection, params};
use std::collections::HashMap;
use uuid::Uuid;

// ── Generators ───────────────────────────────────────────────────────────

fn field_val() -> impl Strategy<Value = FieldValue> {
    prop_oneof![
        any::<String>().prop_filter("ascii, no quote/backslash", |s| {
            s.is_ascii() && !s.contains('"') && !s.contains('\\')
        }).prop_map(FieldValue::String),
        any::<i64>().prop_map(FieldValue::Integer),
        (0.01f64..1_000_000.0f64).prop_map(FieldValue::Float),
        any::<bool>().prop_map(FieldValue::Boolean),
    ]
}

fn gen_node() -> impl Strategy<Value = Node> {
    // Each node has: title (string), app:count (int), app:score (float), app:active (bool)
    // Same types per field name avoids ORDER BY mixed-type divergence.
    (
        any::<i64>(),
        (0.01f64..1_000_000.0f64),
        any::<bool>(),
    ).prop_map(|(count, score, active)| {
        let mut map: HashMap<String, FieldValue> = HashMap::new();
        map.insert("title".into(), FieldValue::String("hello".into()));
        map.insert("app:count".into(), FieldValue::Integer(count));
        map.insert("app:score".into(), FieldValue::Float(score));
        map.insert("app:active".into(), FieldValue::Boolean(active));
        Node {
            id: Uuid::new_v4(),
            fields: map,
            space_id: Uuid::nil(),
            preferred_schemas: vec![],
            app_managed: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    })
}

/// Generates nodes and picks one (key, value) from them.
fn nodes_and_field_ref() -> impl Strategy<Value = (Vec<Node>, String, FieldValue)> {
    proptest::collection::vec(gen_node(), 2..8)
        .prop_flat_map(|nodes| {
            let mut choices: Vec<(String, FieldValue)> = Vec::new();
            for n in &nodes {
                for (k, v) in &n.fields {
                    choices.push((k.clone(), v.clone()));
                }
            }
            choices.sort_by(|a, b| a.0.cmp(&b.0));
            (Just(nodes), proptest::sample::select(choices))
        })
        .prop_map(|(nodes, (key, val))| (nodes, key, val))
}

/// Generates nodes and picks two (key, value) pairs for AND/OR queries.
fn nodes_and_two_field_refs(
) -> impl Strategy<Value = (Vec<Node>, String, FieldValue, String, FieldValue)> {
    proptest::collection::vec(gen_node(), 3..10)
        .prop_flat_map(|nodes| {
            let mut choices: Vec<(String, FieldValue)> = Vec::new();
            for n in &nodes {
                for (k, v) in &n.fields {
                    choices.push((k.clone(), v.clone()));
                }
            }
            choices.sort_by(|a, b| a.0.cmp(&b.0));
            (Just(nodes), proptest::sample::select(choices.clone()), proptest::sample::select(choices))
        })
        .prop_map(|(nodes, (k1, v1), (k2, v2))| (nodes, k1, v1, k2, v2))
}

// ── Helpers ──────────────────────────────────────────────────────────────

/// Create an in-memory SQLite connection with the same schema as SqliteBackend.
fn setup_conn() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
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
        CREATE INDEX IF NOT EXISTS idx_nodes_space ON nodes(space_id);"
    ).unwrap();
    // Meta tables the compiler queries
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS namespaces (
            ns_id INTEGER PRIMARY KEY AUTOINCREMENT,
            kind TEXT NOT NULL DEFAULT 'user',
            app_id TEXT,
            stable_identifier TEXT NOT NULL UNIQUE
        );
        CREATE TABLE IF NOT EXISTS field_presence (
            node_id TEXT NOT NULL,
            ns_id INTEGER NOT NULL,
            field_name TEXT NOT NULL,
            value_type TEXT NOT NULL,
            PRIMARY KEY (node_id, ns_id, field_name)
        );
        CREATE TABLE IF NOT EXISTS node_schema_conformance (
            node_id TEXT NOT NULL,
            schema_id TEXT NOT NULL,
            version_major INTEGER NOT NULL,
            version_minor INTEGER NOT NULL,
            PRIMARY KEY (node_id, schema_id)
        );"
    ).unwrap();
    conn
}

fn insert_nodes(conn: &Connection, nodes: &[Node]) {
    // Ensure namespaces exist and field_presence is populated (compiler needs them for HAS_FIELD)
    let mut ns_ids: HashMap<String, i64> = HashMap::new();
    for node in nodes {
        for key in node.fields.keys() {
            if let Some(idx) = key.find(':') {
                let ns = &key[..idx];
                let field_name = &key[idx+1..];
                let ns_id = if let Some(id) = ns_ids.get(ns) {
                    *id
                } else {
                    conn.execute(
                        "INSERT OR IGNORE INTO namespaces (stable_identifier, kind) VALUES (?1, 'app')",
                        params![ns],
                    ).unwrap();
                    let id: i64 = conn.query_row(
                        "SELECT ns_id FROM namespaces WHERE stable_identifier = ?1",
                        params![ns],
                        |row| row.get(0),
                    ).unwrap();
                    ns_ids.insert(ns.to_string(), id);
                    id
                };
                conn.execute(
                    "INSERT OR REPLACE INTO field_presence (node_id, ns_id, field_name, value_type) VALUES (?1, ?2, ?3, ?4)",
                    params![node.id.to_string(), ns_id, field_name, "any"],
                ).unwrap();
            }
        }
    }
    for node in nodes {
        let fields_json = serde_json::to_string(&node.fields).unwrap();
        let schemas_json = serde_json::to_string(&node.preferred_schemas).unwrap();
        let app_mgmt = node.app_managed.as_ref()
            .map(|a| serde_json::to_string(a).unwrap());
        conn.execute(
            "INSERT INTO nodes (id, space_id, fields_json, preferred_schemas_json, app_managed_json, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                node.id.to_string(),
                node.space_id.to_string(),
                fields_json,
                schemas_json,
                app_mgmt,
                node.created_at.to_rfc3339(),
                node.updated_at.to_rfc3339(),
            ],
        ).unwrap();
    }
}

fn execute_sql(conn: &Connection, sql: &str, params: &[ParamValue]) -> Vec<serde_json::Value> {
    let mut stmt = conn.prepare(sql).unwrap();
    let cols: Vec<String> = stmt.column_names().iter().map(|c| c.to_string()).collect();
    let param_refs: Vec<&dyn rusqlite::types::ToSql> =
        params.iter().map(|p| p as &dyn rusqlite::types::ToSql).collect();

    stmt.query_map(param_refs.as_slice(), |row| {
        let mut obj = serde_json::Map::new();
        for (i, col) in cols.iter().enumerate() {
            let val: Result<rusqlite::types::Value, _> = row.get(i);
            obj.insert(col.clone(), match val {
                Ok(rusqlite::types::Value::Null) => serde_json::Value::Null,
                Ok(rusqlite::types::Value::Integer(n)) => serde_json::Value::Number(n.into()),
                Ok(rusqlite::types::Value::Real(f)) => {
                    serde_json::Number::from_f64(f)
                        .map(serde_json::Value::Number)
                        .unwrap_or(serde_json::Value::Null)
                }
                Ok(rusqlite::types::Value::Text(s)) => {
                    serde_json::from_str(&s).unwrap_or(serde_json::Value::String(s))
                }
                _ => serde_json::Value::Null,
            });
        }
        Ok(serde_json::Value::Object(obj))
    }).unwrap().flatten().collect()
}

fn ids_sorted(rows: &[serde_json::Value]) -> Vec<String> {
    let mut ids: Vec<String> = rows
        .iter()
        .filter_map(|r| r.get("id").and_then(|v| v.as_str()).map(|s| s.to_string()))
        .collect();
    ids.sort();
    ids
}

/// Convert a Node.fields key like `"app:count"` to PQL suffix `app.count`.
fn key_to_pql(key: &str) -> String {
    key.replace(':', ".")
}

/// Normalize Bool/Number mismatch: SQLite returns Number(0/1) for booleans,
/// but the evaluator returns Bool(false/true). Convert both to i64 for comparison.
fn normalize(v: Option<&serde_json::Value>) -> Option<serde_json::Value> {
    match v {
        Some(serde_json::Value::Bool(b)) => Some(serde_json::Value::Number((*b as i64).into())),
        other => other.cloned(),
    }
}

fn field_to_pql(v: &FieldValue) -> String {
    match v {
        FieldValue::String(s) | FieldValue::DateTime(s) => format!("\"{}\"", s),
        FieldValue::Integer(i) => i.to_string(),
        FieldValue::Float(f) => f.to_string(),
        FieldValue::Boolean(b) => b.to_string(),
        _ => "\"\"".into(),
    }
}

fn check(nodes: &[Node], pql: &str, conn: &Connection) {
    let ast = parse_query(pql).unwrap();
    let compiled = compile(&ast, conn).unwrap();
    let sql_rows = execute_sql(conn, &compiled.sql, &compiled.params);
    let mem_rows = eval_query(&ast, nodes);
    assert_eq!(
        ids_sorted(&sql_rows),
        ids_sorted(&mem_rows),
        "\nPQL: {}\nsql:  {:?}\nmem:  {:?}\nnodes: {:?}",
        pql,
        ids_sorted(&sql_rows),
        ids_sorted(&mem_rows),
        nodes.iter().map(|n| (n.id, &n.fields)).collect::<Vec<_>>(),
    );
}

// ── Property tests ───────────────────────────────────────────────────────

proptest! {
    #![proptest_config(ProptestConfig { fork: false, cases: 256, ..ProptestConfig::default() })]

    #[test]
    fn insert_then_return_all(nodes in proptest::collection::vec(gen_node(), 1..10)) {
        let conn = setup_conn();
        insert_nodes(&conn, &nodes);
        check(&nodes, r#"MATCH (n) IN space("default") RETURN n"#, &conn);
    }

    #[test]
    fn where_field_compare(
        (nodes, key, val) in nodes_and_field_ref(),
    ) {
        let conn = setup_conn();
        insert_nodes(&conn, &nodes);
        let field = key_to_pql(&key);
        for op in &["=", "!=", "<", "<=", ">", ">="] {
            let pql = format!(
                r#"MATCH (n) IN space("default") WHERE n.{} {} {} RETURN n"#,
                field, op, field_to_pql(&val)
            );
            check(&nodes, &pql, &conn);
        }
    }

    #[test]
    fn where_and(
        (nodes, k1, v1, k2, v2) in nodes_and_two_field_refs(),
    ) {
        let conn = setup_conn();
        insert_nodes(&conn, &nodes);
        let f1 = key_to_pql(&k1);
        let f2 = key_to_pql(&k2);
        let pql = format!(
            r#"MATCH (n) IN space("default") WHERE n.{} = {} AND n.{} = {} RETURN n"#,
            f1, field_to_pql(&v1), f2, field_to_pql(&v2)
        );
        check(&nodes, &pql, &conn);
    }

    #[test]
    fn where_or(
        (nodes, k1, v1, k2, v2) in nodes_and_two_field_refs(),
    ) {
        let conn = setup_conn();
        insert_nodes(&conn, &nodes);
        let f1 = key_to_pql(&k1);
        let f2 = key_to_pql(&k2);
        let pql = format!(
            r#"MATCH (n) IN space("default") WHERE n.{} = {} OR n.{} = {} RETURN n"#,
            f1, field_to_pql(&v1), f2, field_to_pql(&v2)
        );
        check(&nodes, &pql, &conn);
    }

    #[test]
    fn where_is_null(
        (nodes, key, _val) in nodes_and_field_ref(),
    ) {
        let conn = setup_conn();
        insert_nodes(&conn, &nodes);
        let field = key_to_pql(&key);
        for not in &["IS NULL", "IS NOT NULL"] {
            let pql = format!(
                r#"MATCH (n) IN space("default") WHERE n.{} {} RETURN n"#,
                field, not
            );
            check(&nodes, &pql, &conn);
        }
    }

    #[test]
    fn where_has_field(
        (nodes, key, _val) in nodes_and_field_ref(),
    ) {
        let conn = setup_conn();
        insert_nodes(&conn, &nodes);
        let (ns, name) = if let Some(idx) = key.find(':') {
            (key[..idx].to_string(), key[idx+1..].to_string())
        } else {
            ("app".to_string(), key.clone())
        };
        for ns_str in &[&ns, &"*".to_string()] {
            let pql = format!(
                r#"MATCH (n) IN space("default") WHERE HAS_FIELD(n, "{}", "{}") RETURN n"#,
                ns_str, name
            );
            check(&nodes, &pql, &conn);
        }
    }

    #[test]
    fn where_in(
        (nodes, key, val) in nodes_and_field_ref(),
    ) {
        let conn = setup_conn();
        insert_nodes(&conn, &nodes);
        let field = key_to_pql(&key);
        let pql = format!(
            r#"MATCH (n) IN space("default") WHERE n.{} IN [{}] RETURN n"#,
            field, field_to_pql(&val)
        );
        check(&nodes, &pql, &conn);
    }

    #[test]
    fn where_like(
        (nodes, key, val) in nodes_and_field_ref(),
    ) {
        let conn = setup_conn();
        insert_nodes(&conn, &nodes);
        let s = match &val {
            FieldValue::String(s) | FieldValue::DateTime(s) => s.clone(),
            _ => return Ok(()),
        };
        let pattern = if s.len() >= 3 {
            format!("%{}%", &s[1..s.len()-1])
        } else {
            format!("%{}%", s)
        };
        let field = key_to_pql(&key);
        let pql = format!(
            r#"MATCH (n) IN space("default") WHERE n.{} LIKE "{}" RETURN n"#,
            field, pattern
        );
        check(&nodes, &pql, &conn);
    }

    #[test]
    fn order_by_and_limit(
        (nodes, key, _val) in nodes_and_field_ref(),
    ) {
        let conn = setup_conn();
        insert_nodes(&conn, &nodes);
        let field = key_to_pql(&key);
        for dir in &["ASC", "DESC"] {
            let pql = format!(
                r#"MATCH (n) IN space("default") RETURN n.{} AS val ORDER BY n.{} {} LIMIT 3"#,
                field, field, dir
            );
            let ast = parse_query(&pql).unwrap();
            let compiled = compile(&ast, &conn).unwrap();
            let sql_rows = execute_sql(&conn, &compiled.sql, &compiled.params);
            let mem_rows = eval_query(&ast, &nodes);
            let sql_vals: Vec<_> = sql_rows.iter().map(|r| normalize(r.get("val"))).collect();
            let mem_vals: Vec<_> = mem_rows.iter().map(|r| normalize(r.get("val"))).collect();
            assert_eq!(sql_vals, mem_vals,
                "\nPQL: {}\nsql:  {:?}\nmem:  {:?}", pql, sql_vals, mem_vals);
        }
    }
}

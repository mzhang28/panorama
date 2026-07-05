//! Property tests: the in-memory evaluator and the SQL compiler must agree.
//!
//! Strategy: generate random nodes, pick one of their fields to build a query
//! around, then assert `eval_query(nodes)` == `compile → SQLite`.

use panorama_core::query::{eval_query, parse_query};
use panorama_core::types::{FieldValue, Node};
use panorama_server::storage::sqlite::SqliteBackend;
use panorama_server::storage::StorageBackend;
use proptest::prelude::*;
use std::collections::HashMap;
use uuid::Uuid;

// ── Generators ───────────────────────────────────────────────────────────

fn field_val() -> impl Strategy<Value = FieldValue> {
    prop_oneof![
        any::<String>().prop_map(FieldValue::String),
        any::<i64>().prop_map(FieldValue::Integer),
        any::<f64>().prop_map(FieldValue::Float),
        any::<bool>().prop_map(FieldValue::Boolean),
    ]
}

/// Generate a node with 2–5 random `"app:X"` fields plus one always-present
/// `"title"` field so queries have something stable to target.
fn gen_node() -> impl Strategy<Value = Node> {
    let names = vec!["count", "score", "active", "label"];
    (2..=5usize).prop_flat_map(move |n| {
        proptest::collection::vec(
            (proptest::sample::select(names.clone()), field_val()),
            n..=n,
        )
    }).prop_map(|fields| {
        let mut map: HashMap<String, FieldValue> = HashMap::new();
        map.insert("title".into(), FieldValue::String("hello".into()));
        for (name, val) in fields {
            map.insert(format!("app:{}", name), val);
        }
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

fn setup_backend() -> (SqliteBackend, tempfile::TempDir) {
    let tmp = tempfile::tempdir().unwrap();
    let backend = SqliteBackend::new(tmp.path().join("db"));
    (backend, tmp)
}

fn ids_sorted(rows: &[serde_json::Value]) -> Vec<String> {
    let mut ids: Vec<String> = rows
        .iter()
        .filter_map(|r| r.get("id").and_then(|v| v.as_str()).map(|s| s.to_string()))
        .collect();
    ids.sort();
    ids
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

fn check(nodes: &[Node], pql: &str, backend: &SqliteBackend) {
    let sql_rows = backend.query(pql).unwrap();
    let mem_rows = eval_query(&parse_query(pql).unwrap(), nodes);
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
    #[test]
    fn insert_then_return_all(nodes in proptest::collection::vec(gen_node(), 1..10)) {
        let (backend, _tmp) = setup_backend();
        backend.initialize().unwrap();
        backend.create_batch(nodes.clone()).unwrap();
        check(&nodes, r#"MATCH (n) IN space("default") RETURN n"#, &backend);
    }

    #[test]
    fn where_field_compare(
        (nodes, key, val) in nodes_and_field_ref(),
    ) {
        let (backend, _tmp) = setup_backend();
        backend.initialize().unwrap();
        backend.create_batch(nodes.clone()).unwrap();
        for op in &["=", "!=", "<", "<=", ">", ">="] {
            let pql = format!(
                r#"MATCH (n) IN space("default") WHERE n.{} {} {} RETURN n"#,
                key, op, field_to_pql(&val)
            );
            check(&nodes, &pql, &backend);
        }
    }

    #[test]
    fn where_and(
        (nodes, k1, v1, k2, v2) in nodes_and_two_field_refs(),
    ) {
        let (backend, _tmp) = setup_backend();
        backend.initialize().unwrap();
        backend.create_batch(nodes.clone()).unwrap();
        let pql = format!(
            r#"MATCH (n) IN space("default") WHERE n.{} = {} AND n.{} = {} RETURN n"#,
            k1, field_to_pql(&v1), k2, field_to_pql(&v2)
        );
        check(&nodes, &pql, &backend);
    }

    #[test]
    fn where_or(
        (nodes, k1, v1, k2, v2) in nodes_and_two_field_refs(),
    ) {
        let (backend, _tmp) = setup_backend();
        backend.initialize().unwrap();
        backend.create_batch(nodes.clone()).unwrap();
        let pql = format!(
            r#"MATCH (n) IN space("default") WHERE n.{} = {} OR n.{} = {} RETURN n"#,
            k1, field_to_pql(&v1), k2, field_to_pql(&v2)
        );
        check(&nodes, &pql, &backend);
    }

    #[test]
    fn where_is_null(
        (nodes, key, _val) in nodes_and_field_ref(),
    ) {
        let (backend, _tmp) = setup_backend();
        backend.initialize().unwrap();
        backend.create_batch(nodes.clone()).unwrap();
        for not in &["IS NULL", "IS NOT NULL"] {
            let pql = format!(
                r#"MATCH (n) IN space("default") WHERE n.{} {} RETURN n"#,
                key, not
            );
            check(&nodes, &pql, &backend);
        }
    }

    #[test]
    fn where_has_field(
        (nodes, key, _val) in nodes_and_field_ref(),
    ) {
        let (backend, _tmp) = setup_backend();
        backend.initialize().unwrap();
        backend.create_batch(nodes.clone()).unwrap();
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
            check(&nodes, &pql, &backend);
        }
    }

    #[test]
    fn where_in(
        (nodes, key, val) in nodes_and_field_ref(),
    ) {
        let (backend, _tmp) = setup_backend();
        backend.initialize().unwrap();
        backend.create_batch(nodes.clone()).unwrap();
        let pql = format!(
            r#"MATCH (n) IN space("default") WHERE n.{} IN [{}] RETURN n"#,
            key, field_to_pql(&val)
        );
        check(&nodes, &pql, &backend);
    }

    #[test]
    fn where_like(
        (nodes, key, val) in nodes_and_field_ref(),
    ) {
        let (backend, _tmp) = setup_backend();
        backend.initialize().unwrap();
        backend.create_batch(nodes.clone()).unwrap();
        let s = match &val {
            FieldValue::String(s) | FieldValue::DateTime(s) => s.clone(),
            _ => return Ok(()),
        };
        let pattern = if s.len() >= 3 {
            format!("%{}%", &s[1..s.len()-1])
        } else {
            format!("%{}%", s)
        };
        let pql = format!(
            r#"MATCH (n) IN space("default") WHERE n.{} LIKE "{}" RETURN n"#,
            key, pattern
        );
        check(&nodes, &pql, &backend);
    }

    #[test]
    fn order_by_and_limit(
        (nodes, key, _val) in nodes_and_field_ref(),
    ) {
        let (backend, _tmp) = setup_backend();
        backend.initialize().unwrap();
        backend.create_batch(nodes.clone()).unwrap();
        for dir in &["ASC", "DESC"] {
            let pql = format!(
                r#"MATCH (n) IN space("default") RETURN n.{} AS val ORDER BY n.{} {} LIMIT 3"#,
                key, key, dir
            );
            let sql_rows = backend.query(&pql).unwrap();
            let mem_rows = eval_query(&parse_query(&pql).unwrap(), &nodes);
            let sql_vals: Vec<_> = sql_rows.iter().map(|r| r.get("val").cloned()).collect();
            let mem_vals: Vec<_> = mem_rows.iter().map(|r| r.get("val").cloned()).collect();
            assert_eq!(sql_vals, mem_vals,
                "\nPQL: {}\nsql:  {:?}\nmem:  {:?}", pql, sql_vals, mem_vals);
        }
    }
}

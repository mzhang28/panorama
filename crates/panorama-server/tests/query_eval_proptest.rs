//! Property tests: the in-memory evaluator and the SQL compiler must agree.
//!
//! Two modes:
//! 1. **No schema** — all fields are JSONB; queries use SCAN() for every field
//!    predicate. The compiler must agree with the evaluator.
//! 2. **With schema** — a schema table with promoted columns is set up; queries
//!    with CONFORMS TO can use promoted columns without SCAN.
//!
//! Strategy: generate random nodes, build queries from their fields, then
//! assert `eval_query(nodes)` == `compile → SQLite execution`.

use panorama_core::query::{eval_query, parse_query};
use panorama_core::types::{FieldValue, Node, SchemaRef, SchemaVersion};
use panorama_server::meta::MetaStore;
use panorama_server::query::compiler::{compile, ParamValue};
use proptest::prelude::*;
use proptest::test_runner::Config as ProptestConfig;
use rusqlite::{params, Connection};
use std::collections::HashMap;
use uuid::Uuid;

// ── Generators ──────────────────────────────────────────────────────────────────

fn field_val() -> impl Strategy<Value = FieldValue> {
  prop_oneof![
    any::<String>()
      .prop_filter("ascii, no quote/backslash", |s| {
        s.is_ascii() && !s.contains('"') && !s.contains('\\')
      })
      .prop_map(FieldValue::String),
    any::<i64>().prop_map(FieldValue::Integer),
    (0.01f64..1_000_000.0f64).prop_map(FieldValue::Float),
    any::<bool>().prop_map(FieldValue::Boolean),
  ]
}

/// Generate a node with fixed field names for stable property tests.
fn gen_node() -> impl Strategy<Value = Node> {
  (any::<i64>(), (0.01f64..1_000_000.0f64), any::<bool>()).prop_map(|(count, score, active)| {
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
      (
        Just(nodes),
        proptest::sample::select(choices.clone()),
        proptest::sample::select(choices),
      )
    })
    .prop_map(|(nodes, (k1, v1), (k2, v2))| (nodes, k1, v1, k2, v2))
}

// ── Test helpers ────────────────────────────────────────────────────────────────

/// Create an in-memory SQLite connection with the standard schema + meta tables.
fn setup_conn() -> Connection {
  let conn = Connection::open_in_memory().unwrap();
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
      CREATE INDEX IF NOT EXISTS idx_nodes_space ON nodes(space_id);",
    )
    .unwrap();
  MetaStore::initialize(&conn).unwrap();
  conn
}

/// Create a connection with a promoted schema table registered.
/// Returns (conn, schema_id, schema_table_name).
fn setup_conn_with_schema(field_mappings: serde_json::Value) -> (Connection, Uuid, String) {
  let conn = setup_conn();
  let schema_id = Uuid::new_v4();
  let table_name = format!("schema_data_{}", schema_id.to_string().replace("-", ""));
  let table_name_short = table_name[..40].to_string(); // safe length

  // Create the physical schema data table with the promoted columns
  let mut col_defs = Vec::new();
  col_defs.push("node_id TEXT PRIMARY KEY".to_string());
  if let Some(obj) = field_mappings.as_object() {
    for (_, entry) in obj {
      if let Some(col) = entry.get("column").and_then(|c| c.as_str()) {
        let col_type = entry
          .get("type")
          .and_then(|t| t.as_str())
          .map(sqlite_type)
          .unwrap_or("TEXT");
        col_defs.push(format!("{} {}", col, col_type));
      }
    }
  }

  conn
    .execute_batch(&format!(
      "CREATE TABLE IF NOT EXISTS {} ({});",
      table_name_short,
      col_defs.join(", ")
    ))
    .unwrap();

  MetaStore::upsert_schema_table(
    &conn,
    &schema_id,
    &table_name_short,
    &field_mappings,
    panorama_server::meta::StorageMode::Hybrid,
    panorama_server::meta::MigrationState::Stable,
  )
  .unwrap();

  (conn, schema_id, table_name_short)
}

fn sqlite_type(t: &str) -> &str {
  match t {
    "String" | "DateTime" => "TEXT",
    "Integer" => "INTEGER",
    "Float" => "REAL",
    "Boolean" => "INTEGER",
    _ => "TEXT",
  }
}

fn insert_nodes(conn: &Connection, nodes: &[Node]) {
  let mut ns_ids: HashMap<String, i64> = HashMap::new();
  for node in nodes {
    for key in node.fields.keys() {
      if let Some(idx) = key.find(':') {
        let ns = &key[..idx];
        let field_name = &key[idx + 1..];
        let ns_id = if let Some(id) = ns_ids.get(ns) {
          *id
        } else {
          conn
            .execute(
              "INSERT OR IGNORE INTO namespaces (stable_identifier, kind) VALUES (?1, 'app')",
              params![ns],
            )
            .unwrap();
          let id: i64 = conn
            .query_row(
              "SELECT ns_id FROM namespaces WHERE stable_identifier = ?1",
              params![ns],
              |row| row.get(0),
            )
            .unwrap();
          ns_ids.insert(ns.to_string(), id);
          id
        };
        conn
          .execute(
            "INSERT OR REPLACE INTO field_presence (node_id, ns_id, field_name, value_type) VALUES (?1, ?2, ?3, ?4)",
            params![node.id.to_string(), ns_id, field_name, "any"],
          )
          .unwrap();
      }
    }
  }
  for node in nodes {
    let fields_json = serde_json::to_string(&node.fields).unwrap();
    let schemas_json = serde_json::to_string(&node.preferred_schemas).unwrap();
    let app_mgmt = node
      .app_managed
      .as_ref()
      .map(|a| serde_json::to_string(a).unwrap());
    conn
      .execute(
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
      )
      .unwrap();
  }
}

fn execute_sql(conn: &Connection, sql: &str, params: &[ParamValue]) -> Vec<serde_json::Value> {
  let mut stmt = conn.prepare(sql).unwrap();
  let cols: Vec<String> = stmt.column_names().iter().map(|c| c.to_string()).collect();
  let param_refs: Vec<&dyn rusqlite::types::ToSql> = params
    .iter()
    .map(|p| p as &dyn rusqlite::types::ToSql)
    .collect();

  stmt
    .query_map(param_refs.as_slice(), |row| {
      let mut obj = serde_json::Map::new();
      for (i, col) in cols.iter().enumerate() {
        let val: Result<rusqlite::types::Value, _> = row.get(i);
        obj.insert(
          col.clone(),
          match val {
            Ok(rusqlite::types::Value::Null) => serde_json::Value::Null,
            Ok(rusqlite::types::Value::Integer(n)) => serde_json::Value::Number(n.into()),
            Ok(rusqlite::types::Value::Real(f)) => serde_json::Number::from_f64(f)
              .map(serde_json::Value::Number)
              .unwrap_or(serde_json::Value::Null),
            Ok(rusqlite::types::Value::Text(s)) => {
              serde_json::from_str(&s).unwrap_or(serde_json::Value::String(s))
            }
            _ => serde_json::Value::Null,
          },
        );
      }
      Ok(serde_json::Value::Object(obj))
    })
    .unwrap()
    .flatten()
    .collect()
}

fn ids_sorted(rows: &[serde_json::Value]) -> Vec<String> {
  let mut ids: Vec<String> = rows
    .iter()
    .filter_map(|r| r.get("id").and_then(|v| v.as_str()).map(|s| s.to_string()))
    .collect();
  ids.sort();
  ids
}

fn key_to_pql(key: &str) -> String {
  if let Some(idx) = key.find(':') {
    format!("\"{}\".{}", &key[..idx], &key[idx + 1..])
  } else {
    key.to_string()
  }
}

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

/// Compile a query, execute against SQLite, compare with in-memory evaluator.
fn check(nodes: &[Node], pql: &str, conn: &Connection) {
  let ast = parse_query(pql).unwrap();
  let compiled = compile(&ast, conn).unwrap();
  let sql_rows = execute_sql(conn, &compiled.sql, &compiled.params);
  let mem_rows = eval_query(&ast, nodes);
  assert_eq!(
    ids_sorted(&sql_rows),
    ids_sorted(&mem_rows),
    "\nPQL: {}\nSQL: {}\nsql:  {:?}\nmem:  {:?}\nnodes: {:?}",
    pql,
    compiled.sql,
    ids_sorted(&sql_rows),
    ids_sorted(&mem_rows),
    nodes.iter().map(|n| (n.id, &n.fields)).collect::<Vec<_>>(),
  );
}

// ── Mode 1: No-schema tests (all fields JSONB, use SCAN) ────────────────────────

proptest! {
  #![proptest_config(ProptestConfig { fork: false, cases: 256, ..ProptestConfig::default() })]

  #[test]
  fn insert_then_return_all(nodes in proptest::collection::vec(gen_node(), 1..10)) {
    let conn = setup_conn();
    insert_nodes(&conn, &nodes);
    check(&nodes, r#"MATCH (n) IN space("default") RETURN n"#, &conn);
  }

  #[test]
  fn where_field_compare_scanned(
    (nodes, key, val) in nodes_and_field_ref(),
  ) {
    let conn = setup_conn();
    insert_nodes(&conn, &nodes);
    let field = key_to_pql(&key);
    // For boolean fields, skip ordering operators (§3.7: "No ordering")
    let is_bool = matches!(val, FieldValue::Boolean(_));
    for op in &["=", "!=", "<", "<=", ">", ">="] {
      if is_bool && (*op == "<" || *op == "<=" || *op == ">" || *op == ">=") {
        continue;
      }
      let pql = format!(
        r#"MATCH (n) IN space("default") WHERE SCAN(n.{} {} {}) RETURN n"#,
        field, op, field_to_pql(&val)
      );
      check(&nodes, &pql, &conn);
    }
  }

  #[test]
  fn where_and_scanned(
    (nodes, k1, v1, k2, v2) in nodes_and_two_field_refs(),
  ) {
    let conn = setup_conn();
    insert_nodes(&conn, &nodes);
    let f1 = key_to_pql(&k1);
    let f2 = key_to_pql(&k2);
    let pql = format!(
      r#"MATCH (n) IN space("default") WHERE SCAN(n.{} = {}) AND SCAN(n.{} = {}) RETURN n"#,
      f1, field_to_pql(&v1), f2, field_to_pql(&v2)
    );
    check(&nodes, &pql, &conn);
  }

  #[test]
  fn where_or_scanned(
    (nodes, k1, v1, k2, v2) in nodes_and_two_field_refs(),
  ) {
    let conn = setup_conn();
    insert_nodes(&conn, &nodes);
    let f1 = key_to_pql(&k1);
    let f2 = key_to_pql(&k2);
    let pql = format!(
      r#"MATCH (n) IN space("default") WHERE SCAN(n.{} = {}) OR SCAN(n.{} = {}) RETURN n"#,
      f1, field_to_pql(&v1), f2, field_to_pql(&v2)
    );
    check(&nodes, &pql, &conn);
  }

  #[test]
  fn where_is_null_scanned(
    (nodes, key, _val) in nodes_and_field_ref(),
  ) {
    let conn = setup_conn();
    insert_nodes(&conn, &nodes);
    let field = key_to_pql(&key);
    for pair in &[("IS NULL", "IS NULL"), ("IS NOT NULL", "IS NOT NULL")] {
      let pql = format!(
        r#"MATCH (n) IN space("default") WHERE SCAN(n.{} {}) RETURN n"#,
        field, pair.0
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
  fn where_in_scanned(
    (nodes, key, val) in nodes_and_field_ref(),
  ) {
    let conn = setup_conn();
    insert_nodes(&conn, &nodes);
    let field = key_to_pql(&key);
    let pql = format!(
      r#"MATCH (n) IN space("default") WHERE SCAN(n.{} IN [{}]) RETURN n"#,
      field, field_to_pql(&val)
    );
    check(&nodes, &pql, &conn);
  }

  #[test]
  fn where_like_scanned(
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
      r#"MATCH (n) IN space("default") WHERE SCAN(n.{} LIKE "{}") RETURN n"#,
      field, pattern
    );
    check(&nodes, &pql, &conn);
  }

  #[test]
  fn order_by_and_limit_scanned(
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

// ── Mode 2: Schema-based tests (promoted columns + CONFORMS TO) ─────────────────

#[cfg(test)]
mod schema_tests {
  use super::*;

  /// Create a promoted schema that maps `app:count` to a column.
  fn setup_schema_for_app_fields() -> (Connection, Uuid) {
    let field_mappings = serde_json::json!({
      "count": {"column": "count_col", "type": "Integer", "indexed": false},
      "score": {"column": "score_col", "type": "Float", "indexed": true},
    });
    let (conn, sid, _table) = setup_conn_with_schema(field_mappings);
    (conn, sid)
  }

  #[test]
  fn test_promoted_field_no_scan_needed() {
    let (conn, sid) = setup_schema_for_app_fields();

    // Insert a node with schema conformance
    let node = Node {
      id: Uuid::new_v4(),
      fields: {
        let mut m = HashMap::new();
        m.insert("app:count".into(), FieldValue::Integer(42));
        m.insert("app:score".into(), FieldValue::Float(3.14));
        m.insert("app:active".into(), FieldValue::Boolean(true));
        m
      },
      space_id: Uuid::nil(),
      preferred_schemas: vec![SchemaRef {
        schema_node_id: sid,
        version: SchemaVersion::new(1, 0),
      }],
      app_managed: None,
      created_at: chrono::Utc::now(),
      updated_at: chrono::Utc::now(),
    };

    // Insert into node_schema_conformance
    conn
      .execute(
        "INSERT INTO node_schema_conformance (node_id, schema_id, version_major, version_minor) VALUES (?1, ?2, 1, 0)",
        params![node.id.to_string(), sid.to_string()],
      )
      .unwrap();

    // Insert the node data into the schema data table
    let table_name = format!("schema_data_{}", sid.to_string().replace("-", ""));
    let table_name_short = &table_name[..40];
    conn
      .execute(
        &format!(
          "INSERT INTO {} (node_id, count_col, score_col) VALUES (?1, 42, 3.14)",
          table_name_short
        ),
        params![node.id.to_string()],
      )
      .unwrap();

    let nodes = vec![node.clone()];
    insert_nodes(&conn, &nodes);

    // Query with CONFORMS TO, no SCAN needed for promoted fields.
    // Use explicit namespace path for now; CONFORMS TO shorthand resolution
    // (bare field → schema namespace) is a compiler feature tracked separately.
    let pql = format!(
      r#"MATCH (n) IN space("default") WHERE n CONFORMS TO schema("{}") AND n."app".count = 42 RETURN n."app".count AS c"#,
      sid
    );

    let ast = parse_query(&pql).unwrap();
    let compiled = compile(&ast, &conn).unwrap();
    let sql_rows = execute_sql(&conn, &compiled.sql, &compiled.params);
    let mem_rows = eval_query(&ast, &nodes);

    assert_eq!(sql_rows.len(), 1, "SQL: {}", compiled.sql);
    assert_eq!(mem_rows.len(), 1);
  }

  #[test]
  fn test_unpromoted_field_still_needs_scan() {
    let (conn, sid) = setup_schema_for_app_fields();

    let node = Node {
      id: Uuid::new_v4(),
      fields: {
        let mut m = HashMap::new();
        m.insert("app:active".into(), FieldValue::Boolean(true));
        m
      },
      space_id: Uuid::nil(),
      preferred_schemas: vec![SchemaRef {
        schema_node_id: sid,
        version: SchemaVersion::new(1, 0),
      }],
      app_managed: None,
      created_at: chrono::Utc::now(),
      updated_at: chrono::Utc::now(),
    };

    conn
      .execute(
        "INSERT INTO node_schema_conformance (node_id, schema_id, version_major, version_minor) VALUES (?1, ?2, 1, 0)",
        params![node.id.to_string(), sid.to_string()],
      )
      .unwrap();

    // Also insert into schema data table
    let table_name = format!("schema_data_{}", sid.to_string().replace("-", ""));
    let table_name_short = &table_name[..40];
    conn
      .execute(
        &format!("INSERT INTO {} (node_id) VALUES (?1)", table_name_short),
        params![node.id.to_string()],
      )
      .unwrap();

    let nodes = vec![node];
    insert_nodes(&conn, &nodes);

    // `active` is not a promoted column → needs SCAN
    // But within CONFORMS TO, the field access falls back to Unpromoted JSONB:
    // the compiler should still reject without SCAN
    let pql = format!(
      r#"MATCH (n) IN space("default") WHERE n CONFORMS TO schema("{}") AND n.active = true RETURN n"#,
      sid
    );

    let ast = parse_query(&pql).unwrap();
    let result = compile(&ast, &conn);
    assert!(
      result.is_err(),
      "unpromoted field without SCAN should error"
    );
    assert!(result.unwrap_err().contains("SCAN"));
  }

  #[test]
  fn test_scan_enforcement_no_conforms_to() {
    let conn = setup_conn();
    // No CONFORMS TO → all fields unpromoted → must use SCAN
    let q = parse_query(r#"MATCH (n) IN space("default") WHERE n.foo = "bar" RETURN n"#).unwrap();
    let err = compile(&q, &conn).unwrap_err();
    assert!(err.contains("SCAN"), "error should mention SCAN: {}", err);
  }

  #[test]
  fn test_scan_allows_no_conforms_to() {
    let conn = setup_conn();
    let q =
      parse_query(r#"MATCH (n) IN space("default") WHERE SCAN(n.foo = "bar") RETURN n"#).unwrap();
    // Should compile successfully
    let _compiled = compile(&q, &conn).unwrap();
  }
}

// ── Mode 3: Aggregate queries (GROUP BY, COUNT, SUM, AVG, MIN, MAX) ──────────

/// Compare SQL compiler results against in-memory evaluator for aggregate queries.
fn check_aggregate(nodes: &[Node], pql: &str, conn: &Connection) {
  let ast = parse_query(pql).unwrap();
  let compiled = compile(&ast, conn).unwrap();
  let sql_rows = execute_sql(conn, &compiled.sql, &compiled.params);
  let mem_rows = eval_query(&ast, nodes);

  assert_eq!(
    sql_rows.len(),
    mem_rows.len(),
    "\nPQL: {}\nSQL: {}\nSQL rows: {:?}\nMem rows: {:?}",
    pql, compiled.sql, sql_rows, mem_rows,
  );

  // Row count matches — now compare each row, sorted by all column values.
  fn sort_key(row: &serde_json::Value) -> Vec<String> {
    let mut keys: Vec<String> = row
      .as_object()
      .iter()
      .flat_map(|o| o.keys().cloned())
      .collect();
    keys.sort();
    keys
      .into_iter()
      .map(|k| {
        row
          .get(&k)
          .map(|v| match v {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Number(n) => format!("{:.6}", n.as_f64().unwrap_or(0.0)),
            serde_json::Value::Bool(b) => b.to_string(),
            serde_json::Value::Null => "<null>".into(),
            _ => format!("{:?}", v),
          })
          .unwrap_or_else(|| "<missing>".into())
      })
      .collect()
  }

  let mut sql_sorted: Vec<&serde_json::Value> = sql_rows.iter().collect();
  let mut mem_sorted: Vec<&serde_json::Value> = mem_rows.iter().collect();
  sql_sorted.sort_by_key(|r| sort_key(r));
  mem_sorted.sort_by_key(|r| sort_key(r));

  for (i, (sr, mr)) in sql_sorted.iter().zip(mem_sorted.iter()).enumerate() {
    let s_obj = sr.as_object();
    let m_obj = mr.as_object();
    let mut all_keys: Vec<&str> = s_obj
      .iter()
      .flat_map(|o| o.keys().map(|k| k.as_str()))
      .chain(m_obj.iter().flat_map(|o| o.keys().map(|k| k.as_str())))
      .collect();
    all_keys.sort();
    all_keys.dedup();

    for key in &all_keys {
      let sv = s_obj.and_then(|o| o.get(*key));
      let mv = m_obj.and_then(|o| o.get(*key));
      // Compare values, with epsilon tolerance for floats (JSON round-trip
      // through SQLite text encoding can cause sub-ULP differences).
      match (sv, mv) {
        (Some(serde_json::Value::Number(sn)), Some(serde_json::Value::Number(mn))) => {
          let sf = sn.as_f64().unwrap_or(0.0);
          let mf = mn.as_f64().unwrap_or(0.0);
          let diff = (sf - mf).abs();
          // Allow tiny relative or absolute error from float serialization
          let tol = (sf.abs() + mf.abs()) * 1e-12 + 1e-9;
          if diff > tol && !(sf.is_nan() && mf.is_nan()) {
            panic!(
              "\nRow {i} key '{key}' numeric mismatch (diff={diff}, tol={tol}):\n\
               PQL: {pql}\nSQL: {}\n\
               SQL val: {sv:?}\nMem val: {mv:?}\n\
               SQL rows: {sql_rows:?}\nMem rows: {mem_rows:?}",
              compiled.sql,
            );
          }
        }
        (Some(serde_json::Value::Bool(sb)), Some(serde_json::Value::Bool(mb))) => {
          assert_eq!(sb, mb,
            "\nRow {i} key '{key}' bool mismatch:\nPQL: {pql}\nSQL: {}\nSQL rows: {sql_rows:?}\nMem rows: {mem_rows:?}",
            compiled.sql,
          );
        }
        (sv, mv) => {
          assert_eq!(sv, mv,
            "\nRow {i} key '{key}' mismatch:\nPQL: {pql}\nSQL: {}\nSQL rows: {sql_rows:?}\nMem rows: {mem_rows:?}",
            compiled.sql,
          );
        }
      }
    }
  }
}

/// Pick one field from the node to use as a grouping dimension and another
/// (numeric) field to aggregate.
// ── Deterministic aggregate sanity checks ─────────────────────────────

#[test]
fn aggregate_deterministic_sum_grouped() {
  let conn = setup_conn();
  let mut nodes = Vec::new();
  for i in 0..5i64 {
    let mut fields = HashMap::new();
    let cat = if i < 3 { "a" } else { "b" };
    fields.insert("app:cat".into(), FieldValue::String(cat.into()));
    fields.insert("app:val".into(), FieldValue::Integer(i * 10));
    fields.insert("app:score".into(), FieldValue::Float(i as f64 * 1.5));
    nodes.push(Node {
      id: Uuid::new_v4(),
      fields,
      space_id: Uuid::nil(),
      preferred_schemas: vec![],
      app_managed: None,
      created_at: chrono::Utc::now(),
      updated_at: chrono::Utc::now(),
    });
  }
  insert_nodes(&conn, &nodes);

  check_aggregate(&nodes, r#"MATCH (n) IN space("default") RETURN n."app".cat AS key, SUM(n."app".val) AS total"#, &conn);
  check_aggregate(&nodes, r#"MATCH (n) IN space("default") RETURN n."app".cat AS key, SUM(n."app".score) AS total"#, &conn);
  check_aggregate(&nodes, r#"MATCH (n) IN space("default") RETURN n."app".cat AS key, COUNT(n) AS cnt"#, &conn);
  check_aggregate(&nodes, r#"MATCH (n) IN space("default") RETURN n."app".cat AS key, AVG(n."app".val) AS avg_val"#, &conn);
  check_aggregate(&nodes, r#"MATCH (n) IN space("default") RETURN MIN(n."app".val) AS min_val, MAX(n."app".val) AS max_val"#, &conn);
  check_aggregate(&nodes, r#"MATCH (n) IN space("default") RETURN COUNT(n) AS cnt, SUM(n."app".val) AS total"#, &conn);
}

/// Aggregate-specific node generator — uses bounded integer range to avoid
/// i64 overflow in SUM and float precision edge cases.
fn gen_agg_node() -> impl Strategy<Value = Node> {
  (-100_000i64..100_000i64, (0.01f64..1_000_000.0f64), any::<bool>()).prop_map(
    |(count, score, active)| {
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
    },
  )
}

/// Pick one field from the node as grouping dimension and another to aggregate.
fn agg_nodes_and_two_fields(
) -> impl Strategy<Value = (Vec<Node>, String, String)> {
  proptest::collection::vec(gen_agg_node(), 2..10)
    .prop_flat_map(|nodes| {
      let mut keys: Vec<String> = Vec::new();
      for n in &nodes {
        for k in n.fields.keys() {
          keys.push(k.clone());
        }
      }
      keys.sort();
      keys.dedup();
      (Just(nodes), proptest::sample::select(keys.clone()), proptest::sample::select(keys))
    })
    .prop_map(|(nodes, gk, ak)| (nodes, gk, ak))
}

proptest! {
  #![proptest_config(ProptestConfig { fork: false, cases: 64, ..ProptestConfig::default() })]

  #[test]
  fn aggregate_count_all(
    nodes in proptest::collection::vec(gen_agg_node(), 1..10),
  ) {
    let conn = setup_conn();
    insert_nodes(&conn, &nodes);
    check_aggregate(&nodes, r#"MATCH (n) IN space("default") RETURN COUNT(n) AS cnt"#, &conn);
  }

  #[test]
  fn aggregate_count_field(
    (nodes, _gk, ak) in agg_nodes_and_two_fields(),
  ) {
    let conn = setup_conn();
    insert_nodes(&conn, &nodes);
    let agg_field = key_to_pql(&ak);
    let pql = format!(
      r#"MATCH (n) IN space("default") RETURN COUNT(n.{}) AS cnt"#,
      agg_field,
    );
    check_aggregate(&nodes, &pql, &conn);
  }

  #[test]
  fn aggregate_sum_grouped(
    (nodes, gk, ak) in agg_nodes_and_two_fields(),
  ) {
    let conn = setup_conn();
    insert_nodes(&conn, &nodes);
    let group_field = key_to_pql(&gk);
    let agg_field = key_to_pql(&ak);
    let pql = format!(
      r#"MATCH (n) IN space("default") RETURN n.{} AS key, SUM(n.{}) AS val"#,
      group_field, agg_field,
    );
    check_aggregate(&nodes, &pql, &conn);
  }

  #[test]
  fn aggregate_avg_single(
    (nodes, _gk, ak) in agg_nodes_and_two_fields(),
  ) {
    let conn = setup_conn();
    insert_nodes(&conn, &nodes);
    let agg_field = key_to_pql(&ak);
    let pql = format!(
      r#"MATCH (n) IN space("default") RETURN AVG(n.{}) AS val"#,
      agg_field,
    );
    check_aggregate(&nodes, &pql, &conn);
  }

  #[test]
  fn aggregate_min_max_single(
    (nodes, _gk, ak) in agg_nodes_and_two_fields(),
  ) {
    let conn = setup_conn();
    insert_nodes(&conn, &nodes);
    let agg_field = key_to_pql(&ak);
    let pql = format!(
      r#"MATCH (n) IN space("default") RETURN MIN(n.{}) AS min_val, MAX(n.{}) AS max_val"#,
      agg_field, agg_field,
    );
    check_aggregate(&nodes, &pql, &conn);
  }

  #[test]
  fn aggregate_count_grouped(
    (nodes, gk, _ak) in agg_nodes_and_two_fields(),
  ) {
    let conn = setup_conn();
    insert_nodes(&conn, &nodes);
    let group_field = key_to_pql(&gk);
    let pql = format!(
      r#"MATCH (n) IN space("default") RETURN n.{} AS key, COUNT(n) AS cnt"#,
      group_field,
    );
    check_aggregate(&nodes, &pql, &conn);
  }

  #[test]
  fn aggregate_multi_global(
    (nodes, _gk, ak) in agg_nodes_and_two_fields(),
  ) {
    let conn = setup_conn();
    insert_nodes(&conn, &nodes);
    let agg_field = key_to_pql(&ak);
    let pql = format!(
      r#"MATCH (n) IN space("default") RETURN COUNT(n) AS cnt, SUM(n.{}) AS total, AVG(n.{}) AS avg"#,
      agg_field, agg_field,
    );
    check_aggregate(&nodes, &pql, &conn);
  }

  #[test]
  fn aggregate_grouped_multi_agg(
    (nodes, gk, ak) in agg_nodes_and_two_fields(),
  ) {
    let conn = setup_conn();
    insert_nodes(&conn, &nodes);
    let group_field = key_to_pql(&gk);
    let agg_field = key_to_pql(&ak);
    let pql = format!(
      r#"MATCH (n) IN space("default") RETURN n.{} AS key, COUNT(n) AS cnt, SUM(n.{}) AS val"#,
      group_field, agg_field,
    );
    check_aggregate(&nodes, &pql, &conn);
  }
}

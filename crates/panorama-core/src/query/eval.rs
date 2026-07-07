//! Reference evaluator — executes a Query against an in-memory set of Nodes.
//!
//! Serves as the differential oracle: the SQL compiler must produce identical
//! results on the same data. Supports aggregated queries (GROUP BY, COUNT,
//! SUM, AVG, MIN, MAX) with Cypher-style implicit grouping.

use crate::query::ast::*;
use crate::types::Node;
use std::collections::HashMap;

// ── Top-level entry point ─────────────────────────────────────────────────

/// Evaluate a query against an in-memory collection of nodes.
/// Returns rows in the same shape as the SQL storage layer:
/// `Vec<serde_json::Value>` where each row has keys matching RETURN columns.
///
/// When aggregate functions are present, non-aggregate columns become
/// implicit GROUP BY keys and one row is returned per group.
pub fn eval_query(query: &Query, nodes: &[Node]) -> Vec<serde_json::Value> {
  // Check if any RETURN column is an aggregate
  let has_aggregate = query
    .return_clause
    .columns
    .iter()
    .any(|c| matches!(c.expression, ReturnExpr::Aggregate { .. }));

  // Collect all matching rows (variable → matched nodes)
  let mut var_matches: HashMap<String, Vec<&Node>> = HashMap::new();
  let mut all_matched: Vec<(&str, &Node)> = Vec::new();

  for mc in &query.matches {
    let candidates = filter_by_source(&mc.source, nodes, &var_matches);

    let matched: Vec<&Node> = if let Some(wc) = &mc.where_clause {
      candidates
        .into_iter()
        .filter(|n| eval_predicate(&wc.predicate, n))
        .collect()
    } else {
      candidates
    };

    var_matches.insert(mc.variable.clone(), matched.clone());
    for node in &matched {
      all_matched.push((mc.variable.as_str(), node));
    }
  }

  if has_aggregate {
    return eval_aggregated(query, &all_matched);
  }

  // Non-aggregate path: one row per matched node
  let mut rows: Vec<serde_json::Value> = Vec::new();
  for (var, node) in &all_matched {
    rows.push(project_row(&query.return_clause, var, node));
  }

  // ORDER BY
  eval_order_by(query, &mut rows);

  // LIMIT / SKIP
  rows = eval_limit_skip(query, rows);

  rows
}

/// Evaluate an aggregated query: group by non-aggregate columns, compute
/// aggregate functions per group, return one row per group.
fn eval_aggregated(
  query: &Query,
  matched: &[(&str, &Node)],
) -> Vec<serde_json::Value> {
  let rc = &query.return_clause;

  // Separate aggregate and non-aggregate (grouping) columns
  let group_cols: Vec<usize> = rc
    .columns
    .iter()
    .enumerate()
    .filter(|(_, c)| !matches!(c.expression, ReturnExpr::Aggregate { .. }))
    .map(|(i, _)| i)
    .collect();

  let agg_cols: Vec<usize> = rc
    .columns
    .iter()
    .enumerate()
    .filter(|(_, c)| matches!(c.expression, ReturnExpr::Aggregate { .. }))
    .map(|(i, _)| i)
    .collect();

  // Build groups keyed by Vec<serde_json::Value> (preserving types for
  // comparison against SQLite output).
  let mut groups: HashMap<Vec<serde_json::Value>, Vec<(&str, &Node)>> = HashMap::new();
  for (var, node) in matched {
    let key: Vec<serde_json::Value> = if group_cols.is_empty() {
      vec![serde_json::Value::Null] // global aggregate sentinel
    } else {
      group_cols
        .iter()
        .map(|&i| {
          let col = &rc.columns[i];
          eval_return_expr(&col.expression, var, node)
        })
        .collect()
    };
    groups.entry(key).or_default().push((var, node));
  }

  // If no groups and no rows → return empty
  if groups.is_empty() && matched.is_empty() {
    return Vec::new();
  }

  // If all columns are aggregates and no rows matched → single row
  // with null aggregates (matching SQLite behavior).
  if groups.is_empty() && !agg_cols.is_empty() {
    let row = eval_aggregate_row(rc, &agg_cols, &group_cols, &[], &[]);
    return vec![row];
  }

  let mut rows: Vec<serde_json::Value> = Vec::new();
  for (key, group_nodes) in &groups {
    rows.push(eval_aggregate_row(rc, &agg_cols, &group_cols, group_nodes, key));
  }

  // ORDER BY (on aggregate results)
  eval_order_by(query, &mut rows);

  // LIMIT / SKIP
  eval_limit_skip(query, rows)
}

/// Compute one aggregated row for a group.
fn eval_aggregate_row(
  rc: &ReturnClause,
  agg_cols: &[usize],
  group_cols: &[usize],
  group_nodes: &[(&str, &Node)],
  group_key: &[serde_json::Value],
) -> serde_json::Value {
  let mut map = serde_json::Map::new();

  // Emit grouping columns — take value from the group key directly
  // (which preserves the original JSON type matching SQLite output).
  for (idx, &col_idx) in group_cols.iter().enumerate() {
    let col = &rc.columns[col_idx];
    let alias = col
      .alias
      .clone()
      .unwrap_or_else(|| expr_default_alias(&col.expression));
    let val = group_key
      .get(idx)
      .cloned()
      .unwrap_or(serde_json::Value::Null);
    map.insert(alias, val);
  }

  // Compute aggregate columns
  for &col_idx in agg_cols {
    let col = &rc.columns[col_idx];
    let alias = col
      .alias
      .clone()
      .unwrap_or_else(|| expr_default_alias(&col.expression));
    let val = compute_aggregate(&col.expression, group_nodes);
    map.insert(alias, val);
  }

  serde_json::Value::Object(map)
}

/// Compute an aggregate value over a group of nodes.
fn compute_aggregate(expr: &ReturnExpr, nodes: &[(&str, &Node)]) -> serde_json::Value {
  match expr {
    ReturnExpr::Aggregate { func, expr: inner } => {
      let values: Vec<serde_json::Value> = nodes
        .iter()
        .map(|(var, node)| eval_return_expr(inner, var, node))
        .collect();

      match func {
        AggregateFunc::Count => {
          // COUNT(*) counts all rows; COUNT(field) counts non-null values
          match inner.as_ref() {
            ReturnExpr::Node(_) => serde_json::json!(nodes.len() as f64),
            _ => serde_json::json!(values.iter().filter(|v| !v.is_null()).count() as f64),
          }
        }
        AggregateFunc::Sum => {
          // Use i128 for exact integer summing (avoids i64 overflow)
          let all_ints = values.iter().all(|v| v.as_i64().is_some());
          if all_ints {
            let sum: i128 = values.iter().filter_map(|v| v.as_i64()).map(|v| v as i128).sum();
            serde_json::json!(sum as f64)
          } else {
            let sum: f64 = values.iter().filter_map(|v| v.as_f64()).sum();
            let sum = if sum == 0.0 { 0.0 } else { sum };
            serde_json::json!(sum)
          }
        }
        AggregateFunc::Avg => {
          let non_null: Vec<&serde_json::Value> =
            values.iter().filter(|v| !v.is_null()).collect();
          if non_null.is_empty() {
            serde_json::Value::Null
          } else if non_null.iter().all(|v| v.as_i64().is_some()) {
            // Integer path: sum as i128 then cast to f64 for division
            let sum: i128 = non_null.iter().filter_map(|v| v.as_i64()).map(|v| v as i128).sum();
            serde_json::json!(sum as f64 / non_null.len() as f64)
          } else {
            let sum: f64 = non_null.iter().filter_map(|v| v.as_f64()).sum();
            serde_json::json!(sum / non_null.len() as f64)
          }
        }
        AggregateFunc::Min => {
          let min_val = values
            .iter()
            .filter(|v| !v.is_null())
            .min_by(|a, b| cmp_json_safe(a, b));
          min_val.cloned().unwrap_or(serde_json::Value::Null)
        }
        AggregateFunc::Max => {
          let max_val = values
            .iter()
            .filter(|v| !v.is_null())
            .max_by(|a, b| cmp_json_safe(a, b));
          max_val.cloned().unwrap_or(serde_json::Value::Null)
        }
      }
    }
    _ => serde_json::Value::Null,
  }
}

/// Evaluate a return expression against a single node (no aggregation).
fn eval_return_expr(expr: &ReturnExpr, _var: &str, node: &Node) -> serde_json::Value {
  match expr {
    ReturnExpr::Node(_) => {
      let mut map = serde_json::Map::new();
      map.insert("id".into(), serde_json::Value::String(node.id.to_string()));
      let mut fields_map = serde_json::Map::new();
      for (k, v) in &node.fields {
        fields_map.insert(k.clone(), field_value_to_json(v));
      }
      map.insert("fields_json".into(), serde_json::Value::Object(fields_map));
      serde_json::Value::Object(map)
    }
    ReturnExpr::Field(fp) => {
      let key = field_key(fp);
      node
        .fields
        .get(&key)
        .map(field_value_to_json)
        .unwrap_or(serde_json::Value::Null)
    }
    ReturnExpr::Aggregate { .. } => {
      // Nested aggregates — not valid, return null.
      serde_json::Value::Null
    }
  }
}

/// ORDER BY on result rows.
fn eval_order_by(query: &Query, rows: &mut Vec<serde_json::Value>) {
  if let Some(ob) = &query.order_by {
    let field_key_str = field_key(&ob.field);
    let sort_key = query
      .return_clause
      .columns
      .iter()
      .find_map(|col| match &col.expression {
        ReturnExpr::Field(fp) if field_key(fp) == field_key_str => {
          col.alias.clone().or_else(|| Some(fp.field.clone()))
        }
        _ => None,
      })
      .unwrap_or(field_key_str);
    let desc = matches!(ob.direction, OrderDir::Desc);
    rows.sort_by(|a, b| {
      let va = column_value(a, &sort_key);
      let vb = column_value(b, &sort_key);
      let ord = cmp_json(&va, &vb);
      if desc {
        ord.reverse()
      } else {
        ord
      }
    });
  }
}

/// Apply LIMIT / SKIP.
fn eval_limit_skip(query: &Query, rows: Vec<serde_json::Value>) -> Vec<serde_json::Value> {
  let skip = query.skip.unwrap_or(0) as usize;
  let limit = query.limit.map(|l| l as usize).unwrap_or(rows.len());
  if skip < rows.len() {
    rows.into_iter().skip(skip).take(limit).collect()
  } else {
    Vec::new()
  }
}

/// Default alias for a return expression.
fn expr_default_alias(expr: &ReturnExpr) -> String {
  match expr {
    ReturnExpr::Node(v) => v.clone(),
    ReturnExpr::Field(fp) => fp.field.clone(),
    ReturnExpr::Aggregate { func, .. } => match func {
      AggregateFunc::Count => "count".into(),
      AggregateFunc::Sum => "sum".into(),
      AggregateFunc::Avg => "avg".into(),
      AggregateFunc::Min => "min".into(),
      AggregateFunc::Max => "max".into(),
    },
  }
}

// ── Source filtering ─────────────────────────────────────────────────────

fn filter_by_source<'a>(
  source: &MatchSource,
  nodes: &'a [Node],
  var_matches: &std::collections::HashMap<String, Vec<&'a Node>>,
) -> Vec<&'a Node> {
  match source {
    MatchSource::Space(space_name) => {
      if space_name == "default" {
        nodes.iter().collect()
      } else {
        nodes.iter().collect()
      }
    }
    MatchSource::RefTraverse {
      edge_type,
      target_var: _,
      target_source: _,
      ..
    } => {
      // For RefTraverse, the source variable's matched nodes are in
      // var_matches. Follow the edge field from each source node to
      // find the target nodes.
      let mut target_nodes: Vec<&'a Node> = Vec::new();
      // The edge field key follows the same format as other fields:
      // "ns:field" or bare "field". For now, edge_type is a bare field
      // name that maps to a NodeRef value.
      for source_node in nodes {
        let edge_key = edge_type.as_str();
        if let Some(fv) = source_node.fields.get(edge_key) {
          if let crate::types::FieldValue::NodeRef(target_id) = fv {
            // Find the target node by ID
            if let Some(target) = nodes.iter().find(|n| n.id == *target_id) {
              target_nodes.push(target);
            }
          }
        }
      }
      target_nodes
    }
  }
}

/// Project all RETURN columns for a single matched node (non-aggregate path).
fn project_row(rc: &ReturnClause, var: &str, node: &Node) -> serde_json::Value {
  let mut map = serde_json::Map::new();
  for col in &rc.columns {
    match &col.expression {
      ReturnExpr::Node(_) => {
        // Whole-node return: flatten id and fields_json into top level
        map.insert("id".into(), serde_json::Value::String(node.id.to_string()));
        let mut fields_map = serde_json::Map::new();
        for (k, v) in &node.fields {
          fields_map.insert(k.clone(), field_value_to_json(v));
        }
        map.insert("fields_json".into(), serde_json::Value::Object(fields_map));
      }
      _ => {
        let val = eval_return_expr(&col.expression, var, node);
        let alias = col
          .alias
          .clone()
          .unwrap_or_else(|| expr_default_alias(&col.expression));
        map.insert(alias, val);
      }
    }
  }
  serde_json::Value::Object(map)
}

fn column_value(row: &serde_json::Value, key: &str) -> serde_json::Value {
  // key may be "ns:field" — check direct key first, then check
  // inside fields_json for whole-node returns
  row
    .get(key)
    .cloned()
    .or_else(|| row.get("fields_json").and_then(|fj| fj.get(key)).cloned())
    .unwrap_or(serde_json::Value::Null)
}

/// Compare two JSON values for ordering. Nulls sort first.
fn cmp_json_safe(a: &serde_json::Value, b: &serde_json::Value) -> std::cmp::Ordering {
  cmp_json(a, b)
}

fn cmp_json(a: &serde_json::Value, b: &serde_json::Value) -> std::cmp::Ordering {
  use std::cmp::Ordering;
  match (a, b) {
    (serde_json::Value::Null, serde_json::Value::Null) => Ordering::Equal,
    (serde_json::Value::Null, _) => Ordering::Less,
    (_, serde_json::Value::Null) => Ordering::Greater,
    (serde_json::Value::String(sa), serde_json::Value::String(sb)) => sa.cmp(sb),
    (serde_json::Value::Number(na), serde_json::Value::Number(nb)) => na
      .as_f64()
      .partial_cmp(&nb.as_f64())
      .unwrap_or(Ordering::Equal),
    (serde_json::Value::Bool(ba), serde_json::Value::Bool(bb)) => ba.cmp(bb),
    // Mixed types: evaluator treats as equal; compiler must match
    _ => Ordering::Equal,
  }
}

// ── Predicate evaluation ─────────────────────────────────────────────────

/// Evaluate a single predicate against a node's fields.
/// Public so the reactor subsystem can reuse filter predicates
/// per HOOK_DESIGN §3.1 ("filter predicates reuse the query language's
/// WHERE grammar exactly").
pub fn eval_predicate(pred: &Predicate, node: &Node) -> bool {
  match pred {
    Predicate::FieldCompare {
      field_path,
      op,
      value,
    } => {
      let key = field_key(field_path);
      match node.fields.get(&key) {
        Some(fv) => field_matches_cmp(fv, op, value),
        None => false,
      }
    }
    // `SCAN(inner)` — evaluates the inner predicate; the SCAN marker is
    // a source-level directive that doesn't change evaluation semantics.
    Predicate::Scan(inner) => eval_predicate(inner, node),
    Predicate::IsNull { field_path, not } => {
      let key = field_key(field_path);
      let missing = !node.fields.contains_key(&key);
      if *not {
        !missing
      } else {
        missing
      }
    }
    Predicate::In {
      field_path, values, ..
    } => {
      let key = field_key(field_path);
      match node.fields.get(&key) {
        Some(fv) => values.iter().any(|v| field_matches_cmp(fv, &CmpOp::Eq, v)),
        None => false,
      }
    }
    Predicate::Like {
      field_path,
      pattern,
      ..
    } => {
      let key = field_key(field_path);
      match node.fields.get(&key) {
        Some(fv) => like_match(fv, pattern),
        None => false,
      }
    }
    Predicate::Contains {
      field_path, value, ..
    } => {
      let key = field_key(field_path);
      match node.fields.get(&key) {
        Some(fv) => contains_match(fv, value),
        None => false,
      }
    }
    Predicate::HasField {
      namespace,
      field_name,
      ..
    } => {
      if namespace == "*" {
        node
          .fields
          .keys()
          .any(|k| k.ends_with(&format!(":{}", field_name)))
      } else {
        node
          .fields
          .contains_key(&format!("{}:{}", namespace, field_name))
      }
    }
    Predicate::ConformsTo {
      schema_id,
      version_min,
      version_max,
      ..
    } => {
      let sid = uuid::Uuid::parse_str(schema_id);
      match sid {
        Ok(parsed) => node.preferred_schemas.iter().any(|sref| {
          sref.schema_node_id == parsed
            && version_min.map_or(true, |vmin| sref.version.major >= vmin)
            && version_max.map_or(true, |vmax| sref.version.major <= vmax)
        }),
        Err(_) => {
          // Non-UUID schema identifier — match by string representation
          let sid_str = schema_id.as_str();
          node.preferred_schemas.iter().any(|sref| {
            sref.schema_node_id.to_string() == sid_str
              && version_min.map_or(true, |vmin| sref.version.major >= vmin)
              && version_max.map_or(true, |vmax| sref.version.major <= vmax)
          })
        }
      }
    }
    Predicate::And(a, b) => eval_predicate(a, node) && eval_predicate(b, node),
    Predicate::Or(a, b) => eval_predicate(a, node) || eval_predicate(b, node),
    Predicate::Not(inner) => !eval_predicate(inner, node),
  }
}

// ── Value helpers ────────────────────────────────────────────────────────

fn field_key(fp: &FieldPath) -> String {
  match &fp.namespace {
    Some(ns) => format!("{}:{}", ns, fp.field),
    None => fp.field.clone(),
  }
}

fn field_matches_cmp(fv: &crate::types::FieldValue, op: &CmpOp, qv: &Value) -> bool {
  use crate::types::FieldValue;
  match (fv, qv) {
    (FieldValue::String(a), Value::String(b)) | (FieldValue::DateTime(a), Value::String(b)) => {
      cmp_str(a, b, op)
    }
    (FieldValue::Integer(a), Value::Integer(b)) => cmp_i64(*a, *b, op),
    (FieldValue::Integer(a), Value::Float(b)) => cmp_f64(*a as f64, *b, op),
    (FieldValue::Float(a), Value::Float(b)) => cmp_f64(*a, *b, op),
    (FieldValue::Float(a), Value::Integer(b)) => cmp_f64(*a, *b as f64, op),
    (FieldValue::Boolean(a), Value::Boolean(b)) => match op {
      CmpOp::Eq => a == b,
      CmpOp::Neq => a != b,
      // Boolean has no ordering (§3.7): ordering ops return false at
      // runtime when the type is known (compile-time error when schema
      // is available).
      CmpOp::Lt | CmpOp::Lte | CmpOp::Gt | CmpOp::Gte => false,
    },
    _ => false,
  }
}

fn cmp_str(a: &str, b: &str, op: &CmpOp) -> bool {
  match op {
    CmpOp::Eq => a == b,
    CmpOp::Neq => a != b,
    CmpOp::Lt => a < b,
    CmpOp::Lte => a <= b,
    CmpOp::Gt => a > b,
    CmpOp::Gte => a >= b,
  }
}

fn cmp_i64(a: i64, b: i64, op: &CmpOp) -> bool {
  match op {
    CmpOp::Eq => a == b,
    CmpOp::Neq => a != b,
    CmpOp::Lt => a < b,
    CmpOp::Lte => a <= b,
    CmpOp::Gt => a > b,
    CmpOp::Gte => a >= b,
  }
}

fn cmp_f64(a: f64, b: f64, op: &CmpOp) -> bool {
  match op {
    CmpOp::Eq => a == b,
    CmpOp::Neq => a != b,
    CmpOp::Lt => a < b,
    CmpOp::Lte => a <= b,
    CmpOp::Gt => a > b,
    CmpOp::Gte => a >= b,
  }
}

fn contains_match(fv: &crate::types::FieldValue, qv: &Value) -> bool {
  match fv {
    crate::types::FieldValue::Array(arr) => arr
      .iter()
      .any(|elem| field_matches_cmp(elem, &CmpOp::Eq, qv)),
    crate::types::FieldValue::Json(v) => {
      if let Some(arr) = v.as_array() {
        arr.iter().any(|elem| {
          // Convert serde_json::Value to Value for comparison
          let elem_val = json_to_ast_value(elem);
          field_matches_cmp(fv, &CmpOp::Eq, &elem_val)
        })
      } else {
        false
      }
    }
    _ => false,
  }
}

fn json_to_ast_value(v: &serde_json::Value) -> Value {
  match v {
    serde_json::Value::String(s) => Value::String(s.clone()),
    serde_json::Value::Number(n) => {
      if let Some(i) = n.as_i64() {
        Value::Integer(i)
      } else {
        Value::Float(n.as_f64().unwrap_or(0.0))
      }
    }
    serde_json::Value::Bool(b) => Value::Boolean(*b),
    _ => Value::Null,
  }
}

fn like_match(fv: &crate::types::FieldValue, pattern: &str) -> bool {
  match fv {
    crate::types::FieldValue::String(s) | crate::types::FieldValue::DateTime(s) => {
      like_pattern(s, pattern)
    }
    _ => false,
  }
}

fn like_pattern(s: &str, pattern: &str) -> bool {
  let chars: Vec<char> = s.chars().collect();
  let pat: Vec<char> = pattern.chars().collect();
  let (mut si, mut pi) = (0, 0);
  let mut star: Option<(usize, usize)> = None;

  loop {
    // Pattern exhausted — succeed if we consumed all chars, otherwise
    // backtrack the last % to consume more.
    if pi >= pat.len() {
      if si >= chars.len() {
        return true;
      }
      match star {
        Some((ss, ps)) => {
          si = ss + 1;
          pi = ps;
          star = Some((si, pi));
          continue;
        }
        None => return false,
      }
    }
    if pat[pi] == '%' {
      pi += 1;
      star = Some((si, pi));
      continue;
    }
    if si >= chars.len() {
      return false;
    }
    if pat[pi] == '_' || pat[pi] == chars[si] {
      si += 1;
      pi += 1;
      continue;
    }
    // Mismatch — backtrack
    match star {
      Some((ss, ps)) => {
        si = ss + 1;
        pi = ps;
        star = Some((si, pi));
        continue;
      }
      None => return false,
    }
  }
}

fn field_value_to_json(fv: &crate::types::FieldValue) -> serde_json::Value {
  use crate::types::FieldValue;
  match fv {
    FieldValue::String(s) | FieldValue::DateTime(s) => serde_json::Value::String(s.clone()),
    FieldValue::Integer(i) => serde_json::Value::Number((*i).into()),
    FieldValue::Float(f) => serde_json::Number::from_f64(*f)
      .map(serde_json::Value::Number)
      .unwrap_or(serde_json::Value::Null),
    // SQLite stores booleans as integers 0/1 — match that representation
    // so that GROUP BY keys and aggregate values are consistent.
    FieldValue::Boolean(b) => serde_json::Value::Number((*b as i64).into()),
    FieldValue::Array(arr) => {
      serde_json::Value::Array(arr.iter().map(field_value_to_json).collect())
    }
    FieldValue::NodeRef(id) => serde_json::Value::String(id.to_string()),
    FieldValue::Json(v) => v.clone(),
    FieldValue::ObjectRef(r) => serde_json::Value::String(r.object_id.to_string()),
    FieldValue::Binary(_) => serde_json::Value::Null,
  }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
  use super::*;
  use std::collections::HashMap;

  fn mk_node(fields: &[(&str, crate::types::FieldValue)]) -> Node {
    use crate::types::FieldValue;
    Node {
      id: uuid::Uuid::new_v4(),
      fields: fields
        .iter()
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect(),
      space_id: uuid::Uuid::nil(),
      preferred_schemas: vec![],
      app_managed: None,
      created_at: chrono::Utc::now(),
      updated_at: chrono::Utc::now(),
    }
  }

  /// Parse a query string, evaluate it against nodes, return row count.
  fn run(src: &str, nodes: &[Node]) -> Vec<serde_json::Value> {
    let q = crate::query::parser::parse_query(src).unwrap();
    eval_query(&q, nodes)
  }

  #[test]
  fn simple_match_return() {
    let nodes = vec![mk_node(&[(
      "app:title",
      crate::types::FieldValue::String("hello".into()),
    )])];
    let rows = run(r#"MATCH (n) IN space("personal") RETURN n"#, &nodes);
    assert_eq!(rows.len(), 1);
    // Whole-node return includes id
    assert!(rows[0].get("id").is_some());
  }

  #[test]
  fn where_field_compare_filters() {
    let nodes = vec![
      mk_node(&[("status", crate::types::FieldValue::String("active".into()))]),
      mk_node(&[("status", crate::types::FieldValue::String("deleted".into()))]),
    ];
    let rows = run(
      r#"MATCH (n) IN space("default") WHERE n.status = "active" RETURN n.status AS status"#,
      &nodes,
    );
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["status"], "active");
  }

  #[test]
  fn where_is_null() {
    let nodes = vec![
      mk_node(&[("x", crate::types::FieldValue::String("a".into()))]),
      mk_node(&[]),
    ];
    let rows = run(
      r#"MATCH (n) IN space("default") WHERE n.x IS NULL RETURN n"#,
      &nodes,
    );
    assert_eq!(rows.len(), 1); // only the one without "x"
  }

  #[test]
  fn order_by_and_limit() {
    let nodes = vec![
      mk_node(&[("score", crate::types::FieldValue::Integer(3))]),
      mk_node(&[("score", crate::types::FieldValue::Integer(1))]),
      mk_node(&[("score", crate::types::FieldValue::Integer(2))]),
    ];
    let rows = run(
      r#"MATCH (n) IN space("default") RETURN n.score AS score ORDER BY n.score ASC LIMIT 2"#,
      &nodes,
    );
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["score"], 1);
    assert_eq!(rows[1]["score"], 2);
  }

  #[test]
  fn has_field_wildcard() {
    let nodes = vec![
      mk_node(&[("app:title", crate::types::FieldValue::String("x".into()))]),
      mk_node(&[]),
    ];
    let rows = run(
      r#"MATCH (n) IN space("default") WHERE HAS_FIELD(n, "*", "title") RETURN n"#,
      &nodes,
    );
    assert_eq!(rows.len(), 1);
  }

  #[test]
  fn and_or_not() {
    let nodes = vec![
      mk_node(&[
        ("a", crate::types::FieldValue::Integer(1)),
        ("b", crate::types::FieldValue::Integer(2)),
      ]),
      mk_node(&[
        ("a", crate::types::FieldValue::Integer(10)),
        ("b", crate::types::FieldValue::Integer(20)),
      ]),
    ];
    // a > 5 AND b < 30
    let rows = run(
      r#"MATCH (n) IN space("default") WHERE n.a > 5 AND n.b < 30 RETURN n"#,
      &nodes,
    );
    assert_eq!(rows.len(), 1);
  }

  #[test]
  fn like_filter() {
    let nodes = vec![
      mk_node(&[(
        "title",
        crate::types::FieldValue::String("hello world".into()),
      )]),
      mk_node(&[("title", crate::types::FieldValue::String("goodbye".into()))]),
    ];
    let rows = run(
      r#"MATCH (n) IN space("default") WHERE n.title LIKE "%world%" RETURN n"#,
      &nodes,
    );
    assert_eq!(rows.len(), 1);
  }

  #[test]
  fn in_filter() {
    let nodes = vec![
      mk_node(&[("color", crate::types::FieldValue::String("red".into()))]),
      mk_node(&[("color", crate::types::FieldValue::String("blue".into()))]),
      mk_node(&[("color", crate::types::FieldValue::String("green".into()))]),
    ];
    let rows = run(
      r#"MATCH (n) IN space("default") WHERE n.color IN ["red", "blue"] RETURN n.color AS color"#,
      &nodes,
    );
    assert_eq!(rows.len(), 2);
  }
}

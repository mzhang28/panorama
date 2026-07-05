//! Reference evaluator — executes a Query against an in-memory set of Nodes.
//!
//! Kept small (~120 lines of logic) so it can be verified by inspection.
//! Serves as the differential oracle: the SQL compiler must produce identical
//! results on the same data.

use crate::query::ast::*;
use crate::types::Node;

// ── Top-level entry point ─────────────────────────────────────────────────

/// Evaluate a query against an in-memory collection of nodes.
/// Returns rows in the same shape as the SQL storage layer:
/// `Vec<serde_json::Value>` where each row has keys matching RETURN columns.
pub fn eval_query(query: &Query, nodes: &[Node]) -> Vec<serde_json::Value> {
    let mut rows: Vec<serde_json::Value> = Vec::new();

    // For now: process each MATCH clause independently.
    // Multi-MATCH with RefTraverse joins across them — not yet implemented here.
    // When added, the structure becomes: for each combination of bindings
    // across MATCH clauses, check all WHEREs, then project.
    for mc in &query.matches {
        let candidates = filter_by_source(&mc.source, nodes);

        let matched: Vec<&Node> = if let Some(wc) = &mc.where_clause {
            candidates
                .into_iter()
                .filter(|n| wc.predicates.iter().all(|p| eval_predicate(p, n)))
                .collect()
        } else {
            candidates
        };

        for node in matched {
            let row = project_return(&query.return_clause, &mc.variable, node);
            rows.push(row);
        }
    }

    // ORDER BY
    if let Some(ob) = &query.order_by {
        let key = field_key(&ob.field);
        let desc = matches!(ob.direction, OrderDir::Desc);
        rows.sort_by(|a, b| {
            let va = column_value(a, &key);
            let vb = column_value(b, &key);
            let ord = cmp_json(&va, &vb);
            if desc { ord.reverse() } else { ord }
        });
    }

    // LIMIT / SKIP
    let skip = query.skip.unwrap_or(0) as usize;
    let limit = query.limit.map(|l| l as usize).unwrap_or(rows.len());
    if skip < rows.len() {
        rows = rows.into_iter().skip(skip).take(limit).collect();
    } else {
        rows.clear();
    }

    rows
}

// ── Source filtering ─────────────────────────────────────────────────────

fn filter_by_source<'a>(source: &MatchSource, nodes: &'a [Node]) -> Vec<&'a Node> {
    match source {
        MatchSource::Space(space_name) => {
            if space_name == "default" {
                nodes.iter().collect()
            } else {
                // The SQL compiler uses space_id. Here we can't filter by
                // space name without a space registry, so return all nodes.
                // The differential test controls which nodes are in the set.
                nodes.iter().collect()
            }
        }
        MatchSource::RefTraverse { .. } => {
            // Not yet implemented — RefTraverse requires following edge fields.
            // For now, return empty. The differential test will flag this.
            Vec::new()
        }
    }
}

// ── RETURN projection ────────────────────────────────────────────────────

fn project_return(
    rc: &ReturnClause,
    _var: &str,
    node: &Node,
) -> serde_json::Value {
    let mut map = serde_json::Map::new();

    for col in &rc.columns {
        match &col.expression {
            ReturnExpr::Node(_) => {
                // Whole-node return: include id and all fields
                map.insert("id".into(), serde_json::Value::String(node.id.to_string()));
                let mut fields_map = serde_json::Map::new();
                for (k, v) in &node.fields {
                    fields_map.insert(k.clone(), field_value_to_json(v));
                }
                map.insert("fields_json".into(), serde_json::Value::Object(fields_map));
            }
            ReturnExpr::Field(fp) => {
                let key = field_key(fp);
                let alias = col.alias.clone().unwrap_or_else(|| fp.field.clone());
                let val = node.fields.get(&key)
                    .map(field_value_to_json)
                    .unwrap_or(serde_json::Value::Null);
                map.insert(alias, val);
            }
        }
    }

    serde_json::Value::Object(map)
}

fn column_value(row: &serde_json::Value, key: &str) -> serde_json::Value {
    // key may be "ns:field" — check direct key first, then check
    // inside fields_json for whole-node returns
    row.get(key)
        .cloned()
        .or_else(|| {
            row.get("fields_json")
                .and_then(|fj| fj.get(key))
                .cloned()
        })
        .unwrap_or(serde_json::Value::Null)
}

fn cmp_json(a: &serde_json::Value, b: &serde_json::Value) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    match (a, b) {
        (serde_json::Value::Null, serde_json::Value::Null) => Ordering::Equal,
        (serde_json::Value::Null, _) => Ordering::Less,
        (_, serde_json::Value::Null) => Ordering::Greater,
        (serde_json::Value::String(sa), serde_json::Value::String(sb)) => sa.cmp(sb),
        (serde_json::Value::Number(na), serde_json::Value::Number(nb)) => {
            na.as_f64().partial_cmp(&nb.as_f64()).unwrap_or(Ordering::Equal)
        }
        (serde_json::Value::Bool(ba), serde_json::Value::Bool(bb)) => ba.cmp(bb),
        _ => Ordering::Equal,
    }
}

// ── Predicate evaluation ─────────────────────────────────────────────────

fn eval_predicate(pred: &Predicate, node: &Node) -> bool {
    match pred {
        Predicate::FieldCompare { field_path, op, value, .. } => {
            let key = field_key(field_path);
            match node.fields.get(&key) {
                Some(fv) => field_matches_cmp(fv, op, value),
                None => false,
            }
        }
        Predicate::IsNull { field_path, not } => {
            let key = field_key(field_path);
            let missing = !node.fields.contains_key(&key);
            if *not { !missing } else { missing }
        }
        Predicate::In { field_path, values, .. } => {
            let key = field_key(field_path);
            match node.fields.get(&key) {
                Some(fv) => values.iter().any(|v| field_matches_cmp(fv, &CmpOp::Eq, v)),
                None => false,
            }
        }
        Predicate::Like { field_path, pattern, .. } => {
            let key = field_key(field_path);
            match node.fields.get(&key) {
                Some(fv) => like_match(fv, pattern),
                None => false,
            }
        }
        Predicate::HasField { namespace, field_name, .. } => {
            if namespace == "*" {
                node.fields.keys().any(|k| k.ends_with(&format!(":{}", field_name)))
            } else {
                node.fields.contains_key(&format!("{}:{}", namespace, field_name))
            }
        }
        Predicate::ConformsTo { .. } => {
            // Requires schema metadata not on Node. Return true —
            // the differential oracle catches drift against SQL result.
            true
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
        (FieldValue::String(a), Value::String(b))
        | (FieldValue::DateTime(a), Value::String(b)) => cmp_str(a, b, op),
        (FieldValue::Integer(a), Value::Integer(b)) => cmp_i64(*a, *b, op),
        (FieldValue::Integer(a), Value::Float(b)) => cmp_f64(*a as f64, *b, op),
        (FieldValue::Float(a), Value::Float(b)) => cmp_f64(*a, *b, op),
        (FieldValue::Float(a), Value::Integer(b)) => cmp_f64(*a, *b as f64, op),
        (FieldValue::Boolean(a), Value::Boolean(b)) => match op {
            CmpOp::Eq => a == b,
            CmpOp::Neq => a != b,
            _ => false,
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
        FieldValue::Float(f) => {
            serde_json::Number::from_f64(*f)
                .map(serde_json::Value::Number)
                .unwrap_or(serde_json::Value::Null)
        }
        FieldValue::Boolean(b) => serde_json::Value::Bool(*b),
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
            fields: fields.iter().map(|(k, v)| (k.to_string(), v.clone())).collect(),
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
        let nodes = vec![mk_node(&[
            ("app:title", crate::types::FieldValue::String("hello".into())),
        ])];
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
            mk_node(&[("title", crate::types::FieldValue::String("hello world".into()))]),
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

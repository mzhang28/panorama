//! Compiles a Panorama Query Language AST into parameterized SQL with CTEs.
//!
//! **Phase 1 (meta lookup):** resolves namespaces, schema IDs, and index
//! availability from the meta tables (§7.1).
//!
//! **Phase 2 (SQL generation):** emits a single SQLite statement with CTEs,
//! joining against `field_presence` and `node_schema_conformance` instead of
//! extracting JSON for schema/presence predicates (§7.2).
//!
//! The output is a `CompiledQuery` containing:
//! - A SQL string with `?N` placeholders for SQLite
//! - A `Vec<ParamValue>` of bound parameter values

use panorama_core::query::ast::*;
use rusqlite::Connection;
use std::collections::HashMap;

use crate::meta::MetaStore;

/// A compiled query ready for SQLite execution.
#[derive(Debug, Clone)]
pub struct CompiledQuery {
    pub sql: String,
    pub params: Vec<ParamValue>,
}

/// Simple parameter value wrapper.
#[derive(Debug, Clone)]
pub enum ParamValue {
    Text(String),
    Integer(i64),
    Real(f64),
    Null,
}

impl rusqlite::types::ToSql for ParamValue {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        match self {
            ParamValue::Text(s) => Ok(rusqlite::types::ToSqlOutput::Owned(
                rusqlite::types::Value::Text(s.clone()),
            )),
            ParamValue::Integer(i) => Ok(rusqlite::types::ToSqlOutput::Owned(
                rusqlite::types::Value::Integer(*i),
            )),
            ParamValue::Real(f) => Ok(rusqlite::types::ToSqlOutput::Owned(
                rusqlite::types::Value::Real(*f),
            )),
            ParamValue::Null => Ok(rusqlite::types::ToSqlOutput::Owned(
                rusqlite::types::Value::Null,
            )),
        }
    }
}

/// Compile an AST query into parameterized SQL, performing Phase 1 meta
/// lookups against the supplied connection.
pub fn compile(query: &Query, conn: &Connection) -> Result<CompiledQuery, String> {
    let mut ctx = CompileCtx::new();
    let mut ctes: Vec<String> = Vec::new();
    let mut params: Vec<ParamValue> = Vec::new();
    let mut param_idx = 1;

    // Collect field accesses for Phase 1 resolution
    // (We resolve ns_ids lazily as we encounter field paths.)

    // Process each MATCH clause
    for mc in &query.matches {
        match &mc.source {
            MatchSource::Space(space_name) => {
                let cte_name = format!("_match_{}", mc.variable);
                ctx.var_cte.insert(mc.variable.clone(), cte_name.clone());

                let space_id_str = if space_name == "default" {
                    uuid::Uuid::nil().to_string()
                } else {
                    space_name.clone()
                };

                let mut where_sqls = vec![format!("n.space_id = ?{}", param_idx)];
                params.push(ParamValue::Text(space_id_str));
                param_idx += 1;

                if let Some(wc) = &mc.where_clause {
                    let (pred_sql, pred_params) = compile_predicate(
                        &wc.predicate, &mc.variable, &mut ctx, conn, param_idx,
                    )?;
                    where_sqls.push(pred_sql);
                    param_idx += pred_params.len();
                    params.extend(pred_params);
                }

                // CTEs select from the `nodes` table (aliased `n`)
                ctes.push(format!(
                    "{} AS (SELECT n.* FROM nodes n WHERE {})",
                    cte_name,
                    where_sqls.join(" AND ")
                ));
            }
            MatchSource::RefTraverse { edge_type, target_var, target_source, .. } => {
                let source_cte = ctx.var_cte.get(&mc.variable)
                    .cloned()
                    .unwrap_or_else(|| format!("_source_{}", mc.variable));

                let target_space = match target_source.as_ref() {
                    MatchSource::Space(name) => {
                        if name == "default" {
                            uuid::Uuid::nil().to_string()
                        } else {
                            name.clone()
                        }
                    }
                    _ => return Err("nested RefTraverse not yet supported".into()),
                };

                let cte_name = format!("_traverse_{}", target_var);
                ctes.push(format!(
                    "{} AS (
                        SELECT n.* FROM nodes n
                        JOIN {source} s ON n.id = json_extract(s.fields_json, '$.{edge_type}')
                        WHERE n.space_id = ?{pi}
                    )",
                    cte_name, source = source_cte, edge_type = edge_type, pi = param_idx
                ));
                params.push(ParamValue::Text(target_space));
                param_idx += 1;
                ctx.var_cte.insert(target_var.clone(), cte_name);
            }
        }
    }

    // Get the final CTE name
    let final_var = &query.matches.last()
        .map(|m| m.variable.clone())
        .unwrap_or_else(|| "n".into());
    let final_cte = ctx.var_cte.get(final_var.as_str())
        .cloned()
        .unwrap_or_else(|| "nodes".into());

    // Build SELECT from RETURN clause
    let mut select_cols: Vec<String> = Vec::new();
    for col in &query.return_clause.columns {
        match &col.expression {
            ReturnExpr::Node(_) => {
                select_cols.push(format!("{cte}.*", cte = final_cte));
            }
            ReturnExpr::Field(fp) => {
                let col_alias = col.alias.clone().unwrap_or_else(|| fp.field.clone());
                let key = field_path_to_json_key(fp);
                select_cols.push(format!(
                    "json_extract({cte}.fields_json, '$.\"{key}\".value') AS {alias}",
                    cte = final_cte, key = key, alias = col_alias
                ));
            }
        }
    }

    // Assemble the full SQL with CTEs
    let select_sql = select_cols.join(", ");
    let mut sql = String::from("WITH ");
    sql.push_str(&ctes.join(",\n  "));
    sql.push_str(&format!(
        "\nSELECT {sel} FROM {cte}",
        sel = select_sql,
        cte = final_cte
    ));

    // ORDER BY
    if let Some(ob) = &query.order_by {
        let key = field_path_to_json_key(&ob.field);
        sql.push_str(&format!(
            " ORDER BY json_extract({cte}.fields_json, '$.\"{key}\".value') {dir}",
            cte = final_cte,
            key = key,
            dir = match ob.direction {
                OrderDir::Asc => "ASC",
                OrderDir::Desc => "DESC",
            }
        ));
    }

    // LIMIT
    if let Some(limit) = query.limit {
        sql.push_str(&format!(" LIMIT ?{}", param_idx));
        params.push(ParamValue::Integer(limit as i64));
        param_idx += 1;
    }

    // SKIP (OFFSET)
    if let Some(skip) = query.skip {
        sql.push_str(&format!(" OFFSET ?{}", param_idx));
        params.push(ParamValue::Integer(skip as i64));
        param_idx += 1;
    }

    Ok(CompiledQuery { sql, params })
}

// ── Compilation context ─────────────────────────────────────────────────────

struct CompileCtx {
    /// Maps variable names to their CTE names.
    var_cte: HashMap<String, String>,
    /// Cached ns_id lookups during this compilation.
    ns_id_cache: HashMap<String, i64>,
}

impl CompileCtx {
    fn new() -> Self {
        Self {
            var_cte: HashMap::new(),
            ns_id_cache: HashMap::new(),
        }
    }

    /// Resolve a namespace string to its ns_id (with caching for this compilation).
    fn resolve_ns(&mut self, conn: &Connection, ns_str: &str) -> Result<i64, String> {
        if let Some(id) = self.ns_id_cache.get(ns_str) {
            return Ok(*id);
        }
        let id = MetaStore::resolve_ns_id(conn, ns_str)
            .map_err(|e| format!("namespace resolve '{}': {}", ns_str, e))?;
        self.ns_id_cache.insert(ns_str.to_string(), id);
        Ok(id)
    }
}

// ── Predicate compilation ───────────────────────────────────────────────────

fn compile_predicate(
    pred: &Predicate,
    var: &str,
    ctx: &mut CompileCtx,
    conn: &Connection,
    start_param: usize,
) -> Result<(String, Vec<ParamValue>), String> {
    let mut pi = start_param;

    match pred {
        Predicate::ConformsTo { schema_id, version_min, version_max, .. } => {
            // Phase 1: resolve the schema_id to a stable identifier.
            // For now, treat the schema_id string as the schema identifier.
            // The meta-table approach uses `node_schema_conformance` via a
            // subquery or JOIN.

            // Strategy: use a semi-join on node_schema_conformance:
            //   n.id IN (SELECT node_id FROM node_schema_conformance WHERE schema_id = ?)
            let mut cond = format!(
                "n.id IN (SELECT node_id FROM node_schema_conformance WHERE schema_id = ?{p})",
                p = pi
            );
            let mut vals = vec![ParamValue::Text(schema_id.clone())];
            pi += 1;

            if let Some(vmin) = version_min {
                cond.push_str(&format!(" AND version_major >= ?{}", pi));
                vals.push(ParamValue::Integer(*vmin as i64));
                pi += 1;
            }
            if let Some(vmax) = version_max {
                cond.push_str(&format!(" AND version_major <= ?{}", pi));
                vals.push(ParamValue::Integer(*vmax as i64));
                pi += 1;
            }

            Ok((cond, vals))
        }
        Predicate::FieldCompare { field_path, op, value, scan } => {
            let key = field_path_to_json_key(field_path);
            let val_param = value_to_param(value);
            let sql_op = cmp_sql(op);

            if *scan {
                // SCAN marker — emit json_extract and record scan stat
                // Stats recording happens in the API layer after execution
                Ok((
                    format!(
                        "json_extract(n.fields_json, '$.\"{key}\".value') {op} ?{p}",
                        key = key, op = sql_op, p = pi
                    ),
                    vec![val_param],
                ))
            } else {
                // Check if this is a promoted column or JSONB extraction.
                // For v0 (all JSONB), always use json_extract.
                Ok((
                    format!(
                        "json_extract(n.fields_json, '$.\"{key}\".value') {op} ?{p}",
                        key = key, op = sql_op, p = pi
                    ),
                    vec![val_param],
                ))
            }
        }
        Predicate::HasField { namespace, field_name, .. } => {
            // Phase 1: resolve namespace → ns_id
            let ns_id = ctx.resolve_ns(conn, namespace)?;

            // Use field_presence table instead of JSON extraction:
            //   n.id IN (SELECT node_id FROM field_presence WHERE ns_id = ? AND field_name = ?)
            let cond = if namespace == "*" {
                // Wildcard namespace — any ns_id
                format!(
                    "n.id IN (SELECT node_id FROM field_presence WHERE field_name = ?{p})",
                    p = pi
                )
            } else {
                format!(
                    "n.id IN (SELECT node_id FROM field_presence WHERE ns_id = ?{p} AND field_name = ?{q})",
                    p = pi, q = pi + 1
                )
            };

            let mut vals = if namespace == "*" {
                vec![ParamValue::Text(field_name.clone())]
            } else {
                vec![ParamValue::Integer(ns_id), ParamValue::Text(field_name.clone())]
            };

            pi += vals.len();
            Ok((cond, vals))
        }
        Predicate::IsNull { field_path, not } => {
            let key = field_path_to_json_key(field_path);
            let op = if *not { "IS NOT NULL" } else { "IS NULL" };
            Ok((
                format!(
                    "json_type(n.fields_json, '$.\"{key}\"') {op}",
                    key = key, op = op
                ),
                vec![],
            ))
        }
        Predicate::In { field_path, values, .. } => {
            let key = field_path_to_json_key(field_path);
            let mut placeholders = Vec::new();
            let mut vals = Vec::new();
            for v in values {
                placeholders.push(format!("?{}", pi));
                vals.push(value_to_param(v));
                pi += 1;
            }
            Ok((
                format!(
                    "json_extract(n.fields_json, '$.\"{key}\".value') IN ({phs})",
                    key = key, phs = placeholders.join(", ")
                ),
                vals,
            ))
        }
        Predicate::Like { field_path, pattern, .. } => {
            let key = field_path_to_json_key(field_path);
            Ok((
                format!(
                    "json_extract(n.fields_json, '$.\"{key}\".type') = 'String' AND json_extract(n.fields_json, '$.\"{key}\".value') LIKE ?{p}",
                    key = key, p = pi
                ),
                vec![ParamValue::Text(pattern.clone())],
            ))
        }
        Predicate::And(a, b) => {
            let (sa, pa) = compile_predicate(a, var, ctx, conn, pi)?;
            pi += pa.len();
            let (sb, pb) = compile_predicate(b, var, ctx, conn, pi)?;
            let mut params = pa;
            params.extend(pb);
            Ok((format!("({} AND {})", sa, sb), params))
        }
        Predicate::Or(a, b) => {
            let (sa, pa) = compile_predicate(a, var, ctx, conn, pi)?;
            pi += pa.len();
            let (sb, pb) = compile_predicate(b, var, ctx, conn, pi)?;
            let mut params = pa;
            params.extend(pb);
            Ok((format!("({} OR {})", sa, sb), params))
        }
        Predicate::Not(inner) => {
            let (s, p) = compile_predicate(inner, var, ctx, conn, pi)?;
            Ok((format!("NOT ({})", s), p))
        }
    }
}

/// Convert a FieldPath to the JSON key format used in `fields_json`.
fn field_path_to_json_key(fp: &FieldPath) -> String {
    match &fp.namespace {
        Some(ns) => format!("{}:{}", ns, fp.field),
        None => fp.field.clone(),
    }
}

fn cmp_sql(op: &CmpOp) -> &'static str {
    match op {
        CmpOp::Eq => "=",
        CmpOp::Neq => "<>",
        CmpOp::Lt => "<",
        CmpOp::Lte => "<=",
        CmpOp::Gt => ">",
        CmpOp::Gte => ">=",
    }
}

fn value_to_param(v: &Value) -> ParamValue {
    match v {
        Value::String(s) => ParamValue::Text(s.clone()),
        Value::Integer(i) => ParamValue::Integer(*i),
        Value::Float(f) => ParamValue::Real(*f),
        Value::Boolean(b) => ParamValue::Integer(if *b { 1 } else { 0 }),
        Value::Null => ParamValue::Null,
    }
}

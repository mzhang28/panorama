//! Compiles a Panorama Query Language AST into parameterized SQL with CTEs.
//!
//! The output is a `CompiledQuery` containing:
//! - A SQL string with `?` placeholders for SQLite
//! - A `Vec<Box<dyn rusqlite::types::ToSql>>` of bound parameter values
//!
//! Works against the current simple `nodes` table with `fields_json`.

use panorama_core::query::ast::*;
use panorama_core::query::ir;
use std::collections::HashMap;

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

/// Compile an AST query into parameterized SQL.
pub fn compile(query: &Query) -> Result<CompiledQuery, String> {
    let mut ctx = CompileCtx::new();
    let mut ctes: Vec<String> = Vec::new();
    let mut params: Vec<ParamValue> = Vec::new();
    let mut param_idx = 1;

    // Process each MATCH clause
    for mc in &query.matches {
        match &mc.source {
            MatchSource::Space(space_name) => {
                // CTE: space filter
                let cte_name = format!("_match_{}", mc.variable);
                ctes.push(format!(
                    "{} AS (SELECT * FROM nodes WHERE space_id = ?{})",
                    cte_name, param_idx
                ));
                params.push(ParamValue::Text(space_name.clone()));
                param_idx += 1;
                ctx.var_cte.insert(mc.variable.clone(), cte_name);

                // WHERE predicates on this match
                if let Some(wc) = &mc.where_clause {
                    for pred in &wc.predicates {
                        let (pred_sql, pred_params) = compile_predicate(
                            pred, &mc.variable, &ctx, param_idx,
                        )?;
                        // Push the predicate down into the CTE
                        let cte_name = format!("_match_{}", mc.variable);
                        let new_cte = format!(
                            "{} AS (SELECT * FROM {cte} WHERE {pred_sql})",
                            cte_name,
                            cte = cte_name,
                            pred_sql = pred_sql
                        );
                        // Replace the previous CTE entry
                        if let Some(existing) = ctes.iter_mut().rev().find(|c| c.starts_with(&format!("{} AS", cte_name))) {
                            *existing = new_cte;
                        }
                        param_idx += pred_params.len();
                        params.extend(pred_params);
                    }
                }
            }
            MatchSource::RefTraverse { edge_type, target_var, target_source, .. } => {
                // For reference traversal, we need to join nodes via the ref field.
                // The edge is stored as a NodeRef field value in fields_json.
                let source_cte = ctx.var_cte.get(&mc.variable)
                    .cloned()
                    .unwrap_or_else(|| format!("_source_{}", mc.variable));

                // Resolve target space
                let target_space = match target_source.as_ref() {
                    MatchSource::Space(name) => name.clone(),
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

    // Get the final CTE name (last match clause's variable)
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
                let ns_key = match &fp.namespace {
                    Some(ns) => format!("{}.{}", ns, fp.field),
                    None => fp.field.clone(),
                };
                let json_path = ns_key.replace('.', "\\.");
                select_cols.push(format!(
                    "json_extract({cte}.fields_json, '$.{path}') AS {alias}",
                    cte = final_cte, path = json_path, alias = col_alias
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
        let ns_key = match &ob.field.namespace {
            Some(ns) => format!("{}.{}", ns, ob.field.field),
            None => ob.field.field.clone(),
        };
        let json_path = ns_key.replace('.', "\\.");
        sql.push_str(&format!(
            " ORDER BY json_extract({cte}.fields_json, '$.{path}') {dir}",
            cte = final_cte,
            path = json_path,
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
}

impl CompileCtx {
    fn new() -> Self {
        Self { var_cte: HashMap::new() }
    }
}

// ── Predicate compilation ───────────────────────────────────────────────────

fn compile_predicate(
    pred: &Predicate,
    var: &str,
    ctx: &CompileCtx,
    start_param: usize,
) -> Result<(String, Vec<ParamValue>), String> {
    let cte = ctx.var_cte.get(var).cloned().unwrap_or_else(|| var.into());
    let mut pi = start_param;

    match pred {
        Predicate::ConformsTo { schema_id, .. } => {
            // Check if the node has this schema in its preferred_schemas_json
            Ok((
                format!(
                    "json_extract({cte}.preferred_schemas_json, '$[*].schema_node_id') LIKE ?{p}",
                    cte = cte, p = pi
                ),
                vec![ParamValue::Text(format!("%{}%", schema_id))],
            ))
        }
        Predicate::FieldCompare { field_path, op, value, .. } => {
            let ns_key = field_path_to_json_key(field_path);
            let val_param = value_to_param(value);
            Ok((
                format!(
                    "json_extract({cte}.fields_json, '$.{key}') {op} ?{p}",
                    cte = cte, key = ns_key, op = cmp_sql(op), p = pi
                ),
                vec![val_param],
            ))
        }
        Predicate::HasField { namespace, field_name, .. } => {
            let ns_key = if namespace == "*" {
                format!("%.{}", field_name)
            } else {
                format!("{}.{}", namespace, field_name)
            };
            Ok((
                format!(
                    "json_type({cte}.fields_json, '$.{key}') IS NOT NULL",
                    cte = cte, key = ns_key.replace('.', "\\.")
                ),
                vec![],
            ))
        }
        Predicate::IsNull { field_path, not } => {
            let ns_key = field_path_to_json_key(field_path);
            let op = if *not { "IS NOT NULL" } else { "IS NULL" };
            Ok((
                format!(
                    "json_type({cte}.fields_json, '$.{key}') {op}",
                    cte = cte, key = ns_key, op = op
                ),
                vec![],
            ))
        }
        Predicate::In { field_path, values, .. } => {
            let ns_key = field_path_to_json_key(field_path);
            let mut placeholders = Vec::new();
            let mut vals = Vec::new();
            for v in values {
                placeholders.push(format!("?{}", pi));
                vals.push(value_to_param(v));
                pi += 1;
            }
            Ok((
                format!(
                    "json_extract({cte}.fields_json, '$.{key}') IN ({phs})",
                    cte = cte, key = ns_key, phs = placeholders.join(", ")
                ),
                vals,
            ))
        }
        Predicate::Like { field_path, pattern, .. } => {
            let ns_key = field_path_to_json_key(field_path);
            Ok((
                format!(
                    "json_extract({cte}.fields_json, '$.{key}') LIKE ?{p}",
                    cte = cte, key = ns_key, p = pi
                ),
                vec![ParamValue::Text(pattern.clone())],
            ))
        }
        Predicate::And(a, b) => {
            let (sa, pa) = compile_predicate(a, var, ctx, pi)?;
            pi += pa.len();
            let (sb, pb) = compile_predicate(b, var, ctx, pi)?;
            let mut params = pa;
            params.extend(pb);
            Ok((format!("({} AND {})", sa, sb), params))
        }
        Predicate::Or(a, b) => {
            let (sa, pa) = compile_predicate(a, var, ctx, pi)?;
            pi += pa.len();
            let (sb, pb) = compile_predicate(b, var, ctx, pi)?;
            let mut params = pa;
            params.extend(pb);
            Ok((format!("({} OR {})", sa, sb), params))
        }
        Predicate::Not(inner) => {
            let (s, p) = compile_predicate(inner, var, ctx, pi)?;
            Ok((format!("NOT ({})", s), p))
        }
    }
}

fn field_path_to_json_key(fp: &FieldPath) -> String {
    let key = match &fp.namespace {
        Some(ns) => format!("{}.{}", ns, fp.field),
        None => fp.field.clone(),
    };
    key.replace('.', "\\.")
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
        Value::Boolean(b) => ParamValue::Text(if *b { "true".into() } else { "false".into() }),
        Value::Null => ParamValue::Null,
    }
}

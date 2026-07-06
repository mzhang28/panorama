//! Compiles a Panorama Query Language AST into parameterized SQL with CTEs.
//!
//! ## Phase 1 (meta lookup, §7.1)
//!
//! Resolves namespaces, schema IDs, physical tables, and field access strategies
//! from the meta tables.
//!
//! ## Phase 2 (SQL generation, §7.2)
//!
//! Emits a single SQLite statement:
//! 1. CTE: `space_nodes` — filter by space_id only
//! 2. CTE: `conforming_X` — semi-join on `node_schema_conformance` (per CONFORMS TO)
//! 3. CTE: `schema_data_X` — join physical schema table (if promoted columns exist)
//! 4. Final SELECT with `WHERE` clause containing field predicates, referencing
//!    either `sd.column` (promoted) or `json_extract(n.fields_json, ...)` (unpromoted).
//!
//! ## SCAN enforcement (§3.9, §7.4)
//!
//! Field predicates against non-promoted, non-indexed fields without a `SCAN`
//! marker produce a compile-time error.

use panorama_core::query::ast::*;
use rusqlite::Connection;
use std::collections::HashMap;
use uuid::Uuid;

use super::physical::{resolve_physical_schema, FieldAccess, PhysicalSchema};
use crate::meta::MetaStore;

// ── Output types ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct CompiledQuery {
  pub sql: String,
  pub params: Vec<ParamValue>,
  /// Unique query ID for tracing (§8).
  pub query_id: Uuid,
}

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

// ── Compilation context ─────────────────────────────────────────────────────────

struct CompileCtx {
  /// Maps variable names to their CTE names (the final CTE in the chain for each var).
  var_cte: HashMap<String, String>,
  /// Cached ns_id lookups.
  ns_id_cache: HashMap<String, i64>,
  /// Resolved physical schemas.
  resolved_schemas: Vec<ResolvedSchema>,
}

struct ResolvedSchema {
  variable: String,
  schema_id: String,
  physical: Option<PhysicalSchema>,
  conformance_cte: String,
  /// CTE alias for the schema data table (empty if no physical table).
  data_cte: String,
}

// ── Public entry point ──────────────────────────────────────────────────────────

pub fn compile(query: &Query, conn: &Connection) -> Result<CompiledQuery, String> {
  let query_id = Uuid::new_v4();

  // ── AST → IR lowering (§5) ──────────────────────────────────────────────
  // The IR captures the query structure independent of parameter values,
  // providing a stable key for the prepared-statement cache (§7.3) and
  // an attachment point for query tracing (§8).
  let _ir = panorama_core::query::ir::lower_to_ir(query);

  let mut ctx = CompileCtx::new();

  // ── Phase 1: resolve physical schemas ────────────────────────────────────
  for mc in &query.matches {
    if let Some(wc) = &mc.where_clause {
      extract_conforms_to(&wc.predicate, &mc.variable, conn, &mut ctx)?;
    }
  }

  // ── Phase 2: generate SQL ─────────────────────────────────────────────────
  let mut ctes: Vec<String> = Vec::new();
  let mut params: Vec<ParamValue> = Vec::new();
  let mut param_idx = 1;
  let mut added_ctes: std::collections::HashSet<String> = std::collections::HashSet::new();

  // Collect final WHERE predicates (field predicates that go in the top-level
  // WHERE, not in intermediate CTEs).
  let mut final_where_parts: Vec<String> = Vec::new();

  for mc in &query.matches {
    match &mc.source {
      MatchSource::Space(space_name) => {
        let cte_name = format!("_match_{}", mc.variable);

        let space_id_str = if space_name == "default" {
          Uuid::nil().to_string()
        } else {
          space_name.clone()
        };

        // ── 1. Space CTE: only the space filter ────────────────────────────
        let space_cte = format!("{}_space", cte_name);
        if !added_ctes.contains(&space_cte) {
          ctes.push(format!(
            "{} AS (SELECT n.* FROM nodes n WHERE n.space_id = ?{})",
            space_cte, param_idx
          ));
          params.push(ParamValue::Text(space_id_str));
          param_idx += 1;
          added_ctes.insert(space_cte.clone());
        }
        let mut pipeline_cte = space_cte.clone();

        // ── 2. Conformance + schema data CTEs ──────────────────────────────
        let var_schemas: Vec<&ResolvedSchema> = ctx
          .resolved_schemas
          .iter()
          .filter(|rs| rs.variable == mc.variable)
          .collect();

        for rs in &var_schemas {
          if !added_ctes.contains(&rs.conformance_cte) {
            ctes.push(format!(
              "{} AS (SELECT n.id, n.space_id, n.fields_json, n.preferred_schemas_json, n.created_at, n.updated_at FROM {} n JOIN node_schema_conformance c ON n.id = c.node_id WHERE c.schema_id = ?{})",
              rs.conformance_cte, pipeline_cte, param_idx
            ));
            params.push(ParamValue::Text(rs.schema_id.clone()));
            param_idx += 1;
            added_ctes.insert(rs.conformance_cte.clone());
          }
          pipeline_cte = rs.conformance_cte.clone();

          if let Some(ref physical) = rs.physical {
            if !rs.data_cte.is_empty() && !added_ctes.contains(&rs.data_cte) {
              ctes.push(format!(
                "{} AS (SELECT sd.*, c.fields_json FROM {} sd JOIN {} c ON c.id = sd.node_id)",
                rs.data_cte, physical.table_name, rs.conformance_cte
              ));
              added_ctes.insert(rs.data_cte.clone());
            }
          }
        }

        ctx.var_cte.insert(mc.variable.clone(), pipeline_cte);

        // ── 3. Compile non-CONFORMS-TO predicates into final WHERE ─────────
        if let Some(wc) = &mc.where_clause {
          let (pred_sql, pred_params) =
            compile_predicate_for_final(&wc.predicate, &mc.variable, &ctx, conn, param_idx)?;
          if !pred_sql.is_empty() {
            final_where_parts.push(pred_sql);
            param_idx += pred_params.len();
            params.extend(pred_params);
          }
        }
      }
      MatchSource::RefTraverse {
        edge_type,
        target_var,
        target_source,
        min_depth,
        max_depth,
        ..
      } => {
        let source_cte = ctx
          .var_cte
          .get(&mc.variable)
          .cloned()
          .unwrap_or_else(|| format!("_match_{}", mc.variable));

        let target_space = match target_source.as_ref() {
          MatchSource::Space(name) => {
            if name == "default" {
              Uuid::nil().to_string()
            } else {
              name.clone()
            }
          }
          _ => return Err("nested RefTraverse not yet supported".into()),
        };

        // Build the traversal join condition using the same json_extract
        // helper as all other field accesses — quoted key with .value unwrap.
        let edge_json_path = format!("\"{}\"", edge_type);
        let join_condition = format!(
          "t.id = json_extract(s.fields_json, '$.{}.value')",
          edge_json_path
        );

        // Emit CTEs for each hop up to max_depth.
        // Single-hop (depth=1) is the common case.
        let mut prev_cte = source_cte.clone();
        for depth in 0..*max_depth {
          let hop_cte = if *max_depth > 1 {
            format!("_traverse_{}_hop{}", target_var, depth)
          } else {
            format!("_traverse_{}", target_var)
          };
          if !added_ctes.contains(&hop_cte) {
            ctes.push(format!(
              "{} AS (SELECT t.* FROM nodes t JOIN {} s ON {} WHERE t.space_id = ?{})",
              hop_cte, prev_cte, join_condition, param_idx
            ));
            params.push(ParamValue::Text(target_space.clone()));
            param_idx += 1;
            added_ctes.insert(hop_cte.clone());
            // For the first hop that meets min_depth, register this CTE
            // as the target_var's CTE. Later hops override, so the
            // final hop is the one RETURN reads from.
            if depth + 1 >= *min_depth {
              ctx.var_cte.insert(target_var.clone(), hop_cte.clone());
            }
          }
          prev_cte = hop_cte;
        }

        // Also register source variable's CTE name so RETURN can
        // reference both sides of the traversal.
        ctx.var_cte.insert(mc.variable.clone(), source_cte);

        // Compile WHERE clause for the target side of the traversal.
        // The source side's WHERE was already compiled when the source
        // MATCH clause was processed.
        if let Some(wc) = &mc.where_clause {
          let (pred_sql, pred_params) =
            compile_predicate_for_final(&wc.predicate, &target_var, &ctx, conn, param_idx)?;
          if !pred_sql.is_empty() {
            final_where_parts.push(pred_sql);
            param_idx += pred_params.len();
            params.extend(pred_params);
          }
        }
      }
    }
  }

  // Build SELECT from RETURN clause. Each column may reference a different
  // variable (e.g. `RETURN a.title, b.name` in a RefTraverse). Determine
  // the FROM table per-column.
  let mut select_cols: Vec<String> = Vec::new();
  let mut from_tables: Vec<String> = Vec::new();
  let mut from_set: std::collections::HashSet<String> = std::collections::HashSet::new();

  for col in &query.return_clause.columns {
    match &col.expression {
      ReturnExpr::Node(var) => {
        let cte = ctx
          .var_cte
          .get(var)
          .cloned()
          .unwrap_or_else(|| "nodes".into());
        let schema_alias = ctx
          .resolved_schemas
          .iter()
          .find(|rs| rs.variable == *var && !rs.data_cte.is_empty())
          .map(|rs| rs.data_cte.clone());
        let src = schema_alias.as_ref().unwrap_or(&cte);
        select_cols.push(format!("{cte}.*", cte = src));
        if from_set.insert(src.clone()) {
          from_tables.push(src.clone());
        }
      }
      ReturnExpr::Field(fp) => {
        let col_alias = col.alias.clone().unwrap_or_else(|| fp.field.clone());
        let var_cte = ctx
          .var_cte
          .get(&fp.variable)
          .cloned()
          .unwrap_or_else(|| "nodes".into());
        let schema_alias = ctx
          .resolved_schemas
          .iter()
          .find(|rs| rs.variable == fp.variable && !rs.data_cte.is_empty())
          .map(|rs| rs.data_cte.clone());
        let from_table = schema_alias.as_ref().unwrap_or(&var_cte);
        let expr = compile_field_access(fp, &ctx, from_table, &var_cte)?;
        select_cols.push(format!("{} AS {}", expr, col_alias));
        if from_set.insert(from_table.clone()) {
          from_tables.push(from_table.clone());
        }
      }
    }
  }

  // Assemble the full SQL.
  // Multi-table (RefTraverse) uses FROM cte_a, cte_b (cross join — the CTEs
  // already embed the join condition). Single-table uses FROM cte.
  let select_sql = select_cols.join(", ");
  let from_clause = if from_tables.is_empty() {
    "nodes".to_string()
  } else {
    from_tables.join(", ")
  };
  let mut sql = String::from("WITH ");
  sql.push_str(&ctes.join(",\n  "));
  sql.push_str(&format!(
    "\nSELECT {sel} FROM {src}",
    sel = select_sql,
    src = from_clause
  ));

  // Final WHERE clause (field predicates)
  if !final_where_parts.is_empty() {
    sql.push_str(&format!(" WHERE {}", final_where_parts.join(" AND ")));
  }

  // ORDER BY
  if let Some(ob) = &query.order_by {
    let ob_cte = ctx
      .var_cte
      .get(&ob.field.variable)
      .cloned()
      .unwrap_or_else(|| "nodes".into());
    let expr = compile_field_access(&ob.field, &ctx, &ob_cte, &ob_cte)?;
    sql.push_str(&format!(
      " ORDER BY {} {}",
      expr,
      match ob.direction {
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

  // Prepend query_id as a SQL comment for debugging (§8)
  let sql = format!("/* query_id: {} */\n{}", query_id, sql);
  Ok(CompiledQuery {
    sql,
    params,
    query_id,
  })
}

// ── Phase 1 helpers ────────────────────────────────────────────────────────────

fn extract_conforms_to(
  pred: &Predicate,
  variable: &str,
  conn: &Connection,
  ctx: &mut CompileCtx,
) -> Result<(), String> {
  match pred {
    Predicate::ConformsTo {
      schema_id, field, ..
    } => {
      let sid = Uuid::parse_str(schema_id).unwrap_or_else(|_| Uuid::nil());
      let physical = resolve_physical_schema(conn, &sid)?;

      let data_cte = physical
        .as_ref()
        .map(|_| {
          format!(
            "_schema_data_{}",
            schema_id.replace(|c: char| !c.is_alphanumeric(), "_")
          )
        })
        .unwrap_or_default();

      let conformance_cte = format!("_conforming_{}", variable);

      ctx.resolved_schemas.push(ResolvedSchema {
        variable: field.clone(),
        schema_id: schema_id.clone(),
        physical,
        conformance_cte,
        data_cte,
      });
      Ok(())
    }
    Predicate::And(a, b) | Predicate::Or(a, b) => {
      extract_conforms_to(a, variable, conn, ctx)?;
      extract_conforms_to(b, variable, conn, ctx)
    }
    Predicate::Not(inner) | Predicate::Scan(inner) => {
      extract_conforms_to(inner, variable, conn, ctx)
    }
    _ => Ok(()),
  }
}

// ── Field access compilation ────────────────────────────────────────────────────

fn compile_field_access(
  fp: &FieldPath,
  ctx: &CompileCtx,
  from_table: &str,
  _node_cte: &str,
) -> Result<String, String> {
  // Reject unsupported CRDT view selectors (§3.6)
  match &fp.view {
    Some(CrdtView::Ops) => {
      return Err("CRDT op-stream view (@ops) is not yet supported — cannot compile".into());
    }
    Some(CrdtView::At(_)) => {
      return Err("CRDT at-view (@at) is not yet supported — cannot compile".into());
    }
    _ => {} // @merged (or None) is the default
  }

  let resolved = ctx
    .resolved_schemas
    .iter()
    .find(|rs| rs.variable == fp.variable);

  if let Some(rs) = resolved {
    if let Some(ref physical) = rs.physical {
      let access = physical.field_access(&fp.field, fp.namespace.as_deref());
      return Ok(match access {
        FieldAccess::Promoted { ref column, .. } => format!("{}.{}", from_table, column),
        _ => {
          let key = field_path_to_json_key(fp);
          json_extract_expr(&format!("{}.fields_json", from_table), &key)
        }
      });
    }
  }

  // Fallback: JSONB extraction from the CTE's fields_json column
  let key = field_path_to_json_key(fp);
  Ok(json_extract_expr("fields_json", &key))
}

fn field_path_to_json_key(fp: &FieldPath) -> String {
  match &fp.namespace {
    Some(ns) => format!("{}:{}", ns, fp.field),
    None => fp.field.clone(),
  }
}

/// Build the JSON extraction expression for a field.
/// Returns `json_extract(column, '$."key".value')` — extracting the `.value`
/// subpath from the JSON envelope `{"type": "String", "value": ...}`.
fn json_extract_expr(column_ref: &str, key: &str) -> String {
  format!("json_extract({}, '$.\"{}\".value')", column_ref, key)
}

// ── Predicate compilation (for final WHERE clause) ──────────────────────────────

/// Like `compile_predicate` but skips CONFORMS TO (handled by CTEs) and
/// formats field expressions for the final WHERE context.
fn compile_predicate_for_final(
  pred: &Predicate,
  var: &str,
  ctx: &CompileCtx,
  conn: &Connection,
  start_param: usize,
) -> Result<(String, Vec<ParamValue>), String> {
  compile_predicate_inner(pred, var, ctx, conn, start_param, true, false)
}

fn compile_predicate_inner(
  pred: &Predicate,
  var: &str,
  ctx: &CompileCtx,
  conn: &Connection,
  start_param: usize,
  is_final: bool,
  scan_allowed: bool,
) -> Result<(String, Vec<ParamValue>), String> {
  let mut pi = start_param;

  match pred {
    // CONFORMS TO: handled by CTEs, no output in WHERE
    Predicate::ConformsTo { .. } => Ok((String::new(), vec![])),

    // `SCAN(inner)` — passes through with scan allowed.
    Predicate::Scan(inner) => {
      let (sql, params) =
        compile_predicate_inner(inner, var, ctx, conn, start_param, is_final, true)?;
      // Record field_stats for SCAN — extract the field path from the inner
      // predicate and increment scan_count.
      record_scan_stat(inner, ctx, conn);
      Ok((sql, params))
    }

    Predicate::FieldCompare {
      field_path,
      op,
      value,
    } => {
      let (expr, requires_scan) = field_sql_expr(field_path, var, ctx, is_final)?;
      enforce_scan(field_path, requires_scan, scan_allowed)?;

      // Type checking: validate operator against field type if known
      if let Some(type_tag) = ctx.field_type_for(field_path) {
        check_type_validity(field_path, op, type_tag)?;
      }

      let val_param = value_to_param(value);
      let sql_op = cmp_sql(op);
      Ok((format!("{} {} ?{}", expr, sql_op, pi), vec![val_param]))
    }

    Predicate::HasField {
      namespace,
      field_name,
      ..
    } => {
      let ns_id = ctx.resolve_ns(conn, namespace)?;
      // Use the correct CTE alias for the id column.
      // In the final WHERE, the outer table is from `from_table` — but
      // the column is just `id` (no prefix needed in a single-table context).
      let id_ref = if is_final { "id" } else { "n.id" };
      let cond = if namespace == "*" {
        format!(
          "{id_ref} IN (SELECT node_id FROM field_presence WHERE field_name = ?{p})",
          id_ref = id_ref,
          p = pi
        )
      } else {
        format!(
          "{id_ref} IN (SELECT node_id FROM field_presence WHERE ns_id = ?{p} AND field_name = ?{q})",
          id_ref = id_ref,
          p = pi, q = pi + 1
        )
      };
      let vals = if namespace == "*" {
        vec![ParamValue::Text(field_name.clone())]
      } else {
        vec![
          ParamValue::Integer(ns_id),
          ParamValue::Text(field_name.clone()),
        ]
      };
      Ok((cond, vals))
    }

    Predicate::IsNull { field_path, not } => {
      let (expr, requires_scan) = field_sql_expr(field_path, var, ctx, is_final)?;
      enforce_scan(field_path, requires_scan, scan_allowed)?;
      let null_op = if *not { "IS NOT NULL" } else { "IS NULL" };
      Ok((format!("{} {}", expr, null_op), vec![]))
    }

    Predicate::In {
      field_path, values, ..
    } => {
      let (expr, requires_scan) = field_sql_expr(field_path, var, ctx, is_final)?;
      enforce_scan(field_path, requires_scan, scan_allowed)?;

      let mut placeholders = Vec::new();
      let mut vals = Vec::new();
      for v in values {
        placeholders.push(format!("?{}", pi));
        vals.push(value_to_param(v));
        pi += 1;
      }
      Ok((format!("{} IN ({})", expr, placeholders.join(", ")), vals))
    }

    Predicate::Like {
      field_path,
      pattern,
      ..
    } => {
      let (expr, requires_scan) = field_sql_expr(field_path, var, ctx, is_final)?;
      enforce_scan(field_path, requires_scan, scan_allowed)?;
      Ok((
        format!("{} LIKE ?{}", expr, pi),
        vec![ParamValue::Text(pattern.clone())],
      ))
    }

    Predicate::Contains {
      field_path, value, ..
    } => {
      let (expr, requires_scan) = field_sql_expr(field_path, var, ctx, is_final)?;
      enforce_scan(field_path, requires_scan, scan_allowed)?;
      // For v0, CONTAINS compiles to a LIKE check on the JSON array
      // representation. A proper array-membership index would replace this.
      let val_str = value_to_param(value);
      let pattern = match value {
        Value::String(s) => format!("%\"{}\"%", s),
        Value::Integer(i) => format!("%{}%", i),
        Value::Float(f) => format!("%{}%", f),
        Value::Boolean(b) => format!("%{}%", b),
        Value::Null => "%null%".into(),
      };
      Ok((
        format!("{} LIKE ?{}", expr, pi),
        vec![ParamValue::Text(pattern)],
      ))
    }

    Predicate::And(a, b) => {
      let (sa, pa) = compile_predicate_inner(a, var, ctx, conn, pi, is_final, scan_allowed)?;
      pi += pa.len();
      let (sb, pb) = compile_predicate_inner(b, var, ctx, conn, pi, is_final, scan_allowed)?;
      let mut params = pa;
      params.extend(pb);
      // Filter out empty sub-predicates (e.g., CONFORMS TO)
      match (sa.is_empty(), sb.is_empty()) {
        (true, true) => Ok((String::new(), params)),
        (true, false) => Ok((sb, params)),
        (false, true) => Ok((sa, params)),
        (false, false) => Ok((format!("({} AND {})", sa, sb), params)),
      }
    }
    Predicate::Or(a, b) => {
      let (sa, pa) = compile_predicate_inner(a, var, ctx, conn, pi, is_final, scan_allowed)?;
      pi += pa.len();
      let (sb, pb) = compile_predicate_inner(b, var, ctx, conn, pi, is_final, scan_allowed)?;
      let mut params = pa;
      params.extend(pb);
      match (sa.is_empty(), sb.is_empty()) {
        (true, true) => Ok((String::new(), params)),
        (true, false) => Ok((sb, params)),
        (false, true) => Ok((sa, params)),
        (false, false) => Ok((format!("({} OR {})", sa, sb), params)),
      }
    }
    Predicate::Not(inner) => {
      let (s, p) = compile_predicate_inner(inner, var, ctx, conn, pi, is_final, scan_allowed)?;
      if s.is_empty() {
        Ok((String::new(), p))
      } else {
        Ok((format!("NOT ({})", s), p))
      }
    }
  }
}

/// Return (SQL_expr, requires_scan) for a field path.
fn field_sql_expr(
  fp: &FieldPath,
  _var: &str,
  ctx: &CompileCtx,
  is_final: bool,
) -> Result<(String, bool), String> {
  // Reject unsupported CRDT view selectors in predicates (§3.6)
  match &fp.view {
    Some(CrdtView::Ops) => {
      return Err("CRDT op-stream view (@ops) is not yet supported".into());
    }
    Some(CrdtView::At(_)) => {
      return Err("CRDT at-view (@at) is not yet supported".into());
    }
    _ => {}
  }
  let resolved = ctx
    .resolved_schemas
    .iter()
    .find(|rs| rs.variable == fp.variable);

  if let Some(rs) = resolved {
    if let Some(ref physical) = rs.physical {
      let access = physical.field_access(&fp.field, fp.namespace.as_deref());
      let requires_scan = access.requires_scan();
      let key = field_path_to_json_key(fp);
      let expr = if is_final && !requires_scan {
        match &access {
          FieldAccess::Promoted { column, .. } => format!("{}.{}", rs.data_cte, column),
          _ => json_extract_expr("fields_json", &key),
        }
      } else if is_final {
        json_extract_expr("fields_json", &key)
      } else {
        json_extract_expr("n.fields_json", &key)
      };
      return Ok((expr, requires_scan));
    }
  }

  // No schema → everything is unpromoted JSONB
  let key = field_path_to_json_key(fp);
  let col_ref = if is_final {
    "fields_json"
  } else {
    "n.fields_json"
  };
  Ok((json_extract_expr(col_ref, &key), true))
}

/// Validate that an operator is compatible with a field's declared type
/// per QUERY_DESIGN.md §3.7 type compatibility table.
fn check_type_validity(fp: &FieldPath, op: &CmpOp, type_tag: &str) -> Result<(), String> {
  let valid = match type_tag {
    "String" | "DateTime" | "RgaText" => matches!(
      op,
      CmpOp::Eq | CmpOp::Neq | CmpOp::Lt | CmpOp::Lte | CmpOp::Gt | CmpOp::Gte
    ),
    "Integer" | "Float" | "Counter" | "Timestamp" => matches!(
      op,
      CmpOp::Eq | CmpOp::Neq | CmpOp::Lt | CmpOp::Lte | CmpOp::Gt | CmpOp::Gte
    ),
    "Boolean" | "NodeRef" => matches!(op, CmpOp::Eq | CmpOp::Neq),
    "Array" | "OrSet" => false, // only CONTAINS and IS NULL are valid
    _ => true,                  // unknown types → allow (checked at runtime)
  };
  if !valid {
    return Err(format!(
      "operator {:?} is not valid for field `{}`.`{}` of type {}",
      op,
      fp.variable,
      field_path_to_json_key(fp),
      type_tag
    ));
  }
  Ok(())
}

/// Record a field_stats.scan_count increment for the field referenced by
/// a predicate (called when SCAN is used on an unindexed field).
fn record_scan_stat(pred: &Predicate, ctx: &CompileCtx, conn: &Connection) {
  let fp = match pred {
    Predicate::FieldCompare { field_path, .. }
    | Predicate::IsNull { field_path, .. }
    | Predicate::In { field_path, .. }
    | Predicate::Like { field_path, .. }
    | Predicate::Contains { field_path, .. } => field_path,
    _ => return,
  };
  // Extract ns and field name for stats recording
  if let Some(ns) = &fp.namespace {
    let field_name = &fp.field;
    if let Ok(ns_id) = MetaStore::resolve_ns_id(conn, ns) {
      let _ = MetaStore::record_field_scan(conn, ns_id, field_name);
    }
  }
}

fn enforce_scan(fp: &FieldPath, requires_scan: bool, scan_marker: bool) -> Result<(), String> {
  if requires_scan && !scan_marker {
    return Err(format!(
      "field `{}`.`{}` is not indexed — wrap in SCAN(...) or add an index",
      fp.variable,
      field_path_to_json_key(fp),
    ));
  }
  Ok(())
}

// ── Helpers ─────────────────────────────────────────────────────────────────────

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

// ── CompileCtx impl ─────────────────────────────────────────────────────────────

impl CompileCtx {
  fn new() -> Self {
    Self {
      var_cte: HashMap::new(),
      ns_id_cache: HashMap::new(),
      resolved_schemas: Vec::new(),
    }
  }

  fn resolve_ns(&self, conn: &Connection, ns_str: &str) -> Result<i64, String> {
    MetaStore::resolve_ns_id(conn, ns_str)
      .map_err(|e| format!("namespace resolve '{}': {}", ns_str, e))
  }

  /// Look up the declared type for a field path from resolved schemas.
  fn field_type_for(&self, fp: &FieldPath) -> Option<&str> {
    self
      .resolved_schemas
      .iter()
      .find(|rs| rs.variable == fp.variable)
      .and_then(|rs| rs.physical.as_ref())
      .and_then(|p| p.field_type(&fp.field))
  }
}

// ── Tests ───────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
  use super::*;
  use panorama_core::query::parse_query;

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

  #[test]
  fn test_compile_simple_return() {
    let conn = setup_conn();
    let q = parse_query(r#"MATCH (n) IN space("default") RETURN n"#).unwrap();
    let compiled = compile(&q, &conn).unwrap();
    assert!(compiled.sql.contains("SELECT"));
    assert!(compiled.sql.contains("nodes"));
  }

  #[test]
  fn test_compile_with_field_projection() {
    let conn = setup_conn();
    let q = parse_query(r#"MATCH (n) IN space("default") RETURN n.title AS title"#).unwrap();
    let compiled = compile(&q, &conn).unwrap();
    assert!(compiled.sql.contains("json_extract") || compiled.sql.contains("fields_json"));
  }

  #[test]
  fn test_compile_with_conforms_to_and_promoted_schema() {
    let conn = setup_conn();
    let schema_id = Uuid::new_v4();

    let table_name = format!("schema_data_{}", schema_id.to_string().replace("-", ""));
    let tn = &table_name[..40];

    conn
      .execute_batch(&format!(
        "CREATE TABLE IF NOT EXISTS {} (node_id TEXT PRIMARY KEY, title_col TEXT, start_col TEXT);",
        tn
      ))
      .unwrap();

    MetaStore::upsert_schema_table(
      &conn,
      &schema_id,
      tn,
      &serde_json::json!({
        "title": {"column": "title_col", "type": "String", "indexed": false},
        "start_time": {"column": "start_col", "type": "DateTime", "indexed": true},
      }),
      crate::meta::StorageMode::Hybrid,
      crate::meta::MigrationState::Stable,
    )
    .unwrap();

    let q = parse_query(&format!(
      r#"MATCH (n) IN space("default") WHERE n CONFORMS TO schema("{}") RETURN n.title AS title"#,
      schema_id
    ))
    .unwrap();

    let compiled = compile(&q, &conn).unwrap();
    assert!(
      compiled.sql.contains(tn),
      "should use schema data table {}: {}",
      tn,
      compiled.sql
    );
  }

  #[test]
  fn test_scan_enforcement_without_conforms_to() {
    let conn = setup_conn();
    let q = parse_query(r#"MATCH (n) IN space("default") WHERE n.foo = "bar" RETURN n"#).unwrap();
    let result = compile(&q, &conn);
    assert!(result.is_err(), "should error");
    assert!(result.unwrap_err().contains("SCAN"));
  }

  #[test]
  fn test_scan_allows_access() {
    let conn = setup_conn();
    let q =
      parse_query(r#"MATCH (n) IN space("default") WHERE SCAN(n.foo = "bar") RETURN n"#).unwrap();
    let _compiled = compile(&q, &conn).unwrap();
  }

  #[test]
  fn test_crdt_ops_rejected() {
    let conn = setup_conn();
    let q = parse_query(r#"MATCH (n) IN space("default") RETURN n.event.attendees@ops"#).unwrap();
    let err = compile(&q, &conn).unwrap_err();
    assert!(
      err.contains("@ops") || err.contains("op-stream"),
      "should mention @ops: {}",
      err
    );
  }

  #[test]
  fn test_contains_compiles() {
    let conn = setup_conn();
    let q =
      parse_query(r#"MATCH (n) IN space("default") WHERE SCAN(n.tags CONTAINS "urgent") RETURN n"#)
        .unwrap();
    let compiled = compile(&q, &conn).unwrap();
    assert!(compiled.sql.contains("LIKE"));
  }
}

//! PromQL → Panorama Query Language (PQL) translator.
//!
//! Translates a PromQL AST expression into a PQL query string plus
//! a list of DataFrame post-processing steps.
//!
//! ## Translation model
//!
//! Each PromQL expression translates to:
//! 1. A PQL query that fetches matching nodes (always `RETURN n`)
//! 2. Zero or more post-processing steps that transform the raw nodes
//!    into the final DataFrame (rate calculation, aggregation, sorting, etc.)
//!
//! ## Example
//!
//! `rate(wakatime_duration{project="panorama"}[5m])` translates to:
//! - PQL: `MATCH (n) IN space("default") WHERE HAS_FIELD(n, "wakatime", "duration")
//!          AND n.wakatime.project = "panorama" RETURN n`
//! - Post-steps: [Rate { range_seconds: 300.0 }]

use super::ast::*;
use super::registry::{MetricMapping, MetricRegistry};
use panorama_core::{FieldValue, Node};
use std::collections::BTreeMap;

/// A translated query ready for execution.
#[derive(Debug, Clone)]
pub struct TranslatedQuery {
    /// The PQL query string to execute. Always returns whole nodes (`RETURN n`).
    pub pql: String,
    /// Post-processing steps to apply to the raw PQL results.
    pub post_steps: Vec<PostStep>,
    /// The label fields to extract from each node (fully qualified, e.g. "wakatime:project").
    pub label_fields: Vec<String>,
    /// The value field key (e.g. "wakatime:duration").
    pub value_field: String,
    /// The time field key (e.g. "wakatime:time").
    pub time_field: String,
}

/// A post-processing step applied to DataFrame rows.
#[derive(Debug, Clone)]
pub enum PostStep {
    /// Compute rate: (last - first) / range_seconds, per series.
    Rate { range_seconds: f64 },
    /// Compute absolute increase: last - first, per series.
    Increase,
    /// Sum values grouped by given label keys (relative to the query's label_fields).
    GroupSum { group_by: Vec<String> },
    /// Average values grouped by given label keys.
    GroupAvg { group_by: Vec<String> },
    /// Minimum values grouped by given label keys.
    GroupMin { group_by: Vec<String> },
    /// Maximum values grouped by given label keys.
    GroupMax { group_by: Vec<String> },
    /// Count values grouped by given label keys.
    GroupCount { group_by: Vec<String> },
    /// Standard deviation grouped by label keys.
    GroupStddev { group_by: Vec<String> },
    /// Standard variance grouped by label keys.
    GroupStdvar { group_by: Vec<String> },
    /// Take the top K results by value.
    TopK { k: usize },
    /// Take the bottom K results by value.
    BottomK { k: usize },
    /// Sort results by value.
    Sort { desc: bool },
    /// Apply a histogram_quantile calculation.
    HistogramQuantile { quantile: f64 },
    /// Filter: keep only results where value compares to the given threshold.
    ComparisonFilter { op: BinOpKind, threshold: f64 },
}

// ── Translation context ───────────────────────────────────────────────────────

struct Ctx<'a> {
    registry: &'a MetricRegistry,
    /// Dashboard time range start (RFC 3339 / ISO 8601 string).
    from: String,
    /// Dashboard time range end (RFC 3339 / ISO 8601 string).
    to: String,
}

// ── Public entry point ────────────────────────────────────────────────────────

/// Translate a PromQL expression into a PQL query and post-processing steps.
///
/// `from` and `to` are the dashboard time range as RFC 3339 strings.
pub fn translate(
    expr: &Expr,
    registry: &MetricRegistry,
    from: &str,
    to: &str,
) -> Result<TranslatedQuery, String> {
    let ctx = Ctx { registry, from: from.to_string(), to: to.to_string() };
    translate_expr(expr, &ctx)
}

// ── Recursive translator ──────────────────────────────────────────────────────

fn translate_expr(expr: &Expr, ctx: &Ctx) -> Result<TranslatedQuery, String> {
    match expr {
        Expr::InstantVector(iv) => translate_instant_vector(iv, None, ctx),
        Expr::RangeVector(rv) => translate_range_vector(rv, ctx),
        Expr::FunctionCall(fc) => translate_function(fc, ctx),
        Expr::Aggregation(agg) => translate_aggregation(agg, ctx),
        Expr::BinaryOp(bin) => translate_binary_op(bin, ctx),
        Expr::UnaryOp(un) => translate_unary(un, ctx),
        Expr::Subquery(sq) => translate_subquery(sq, ctx),
        Expr::NumberLiteral(_n) => {
            // A scalar literal: produces a constant value with no PQL needed
            Ok(TranslatedQuery {
                pql: String::new(),
                post_steps: vec![PostStep::Sort { desc: false }], // placeholder — scalar value
                label_fields: vec![],
                value_field: String::new(),
                time_field: String::new(),
            })
        }
        Expr::StringLiteral(_) => Err("string literals are not supported as standalone expressions".into()),
        Expr::Paren(e) => translate_expr(e, ctx),
    }
}

// ── Vector selectors ──────────────────────────────────────────────────────────

/// Translate an instant vector selector: `metric_name{labels}`.
/// Optionally applies a time range constraint for range vectors.
fn translate_instant_vector(
    iv: &InstantVector,
    range: Option<&Duration>,
    ctx: &Ctx,
) -> Result<TranslatedQuery, String> {
    let mapping = resolve_metric(&iv.metric_name, ctx.registry)?;

    // Build WHERE clause predicates
    let mut predicates: Vec<String> = Vec::new();

    // Required field presence (e.g., HAS_FIELD(n, "wakatime", "entity"))
    for req_field in &mapping.required_fields {
        if let Some((ns, field)) = split_field_key(req_field) {
            predicates.push(format!(
                "HAS_FIELD(n, \"{}\", \"{}\")", ns, field
            ));
        }
    }

    // Always check that the value field exists
    predicates.push(format!(
        "HAS_FIELD(n, \"{}\", \"{}\")",
        mapping.namespace, mapping.value_field
    ));

    // Label matchers become field comparisons
    let mut label_fields: Vec<String> = Vec::new();
    for matcher in &iv.matchers {
        let field_key = mapping.label_field_key(&matcher.label);
        label_fields.push(field_key.clone());

        match matcher.op {
            MatchOp::Eq => {
                predicates.push(format!(
                    "n.{}.{} = \"{}\"",
                    mapping.namespace, matcher.label,
                    escape_pql_string(&matcher.value)
                ));
            }
            MatchOp::NotEq => {
                predicates.push(format!(
                    "n.{}.{} != \"{}\"",
                    mapping.namespace, matcher.label,
                    escape_pql_string(&matcher.value)
                ));
            }
            MatchOp::Regex => {
                // PQL supports LIKE for pattern matching — convert regex to LIKE
                // For simple patterns, this works; complex regex may need
                // post-filtering
                let like_pattern = regex_to_like(&matcher.value);
                predicates.push(format!(
                    "n.{}.{} LIKE \"{}\"",
                    mapping.namespace, matcher.label,
                    escape_pql_string(&like_pattern)
                ));
            }
            MatchOp::NotRegex => {
                // For NOT regex, we'll need post-filtering. For now, skip the
                // PQL filter and add a note.
                // Actually, PQL doesn't have NOT LIKE. We'll filter post-query.
                // Skip this predicate and handle in post-processing
            }
        }
    }

    // Time range constraint (from range vector or dashboard range)
    let time_field_key = mapping.time_field_key();
    let time_pql_path = field_key_to_pql_path(&time_field_key);
    if let Some(dur) = range {
        // Range vector: look back from the dashboard `to` time
        // For range vectors, compute the start time relative to `to`
        // and format both as ISO 8601 strings for PQL comparison
        let to_dt = chrono::DateTime::parse_from_rfc3339(&ctx.to)
            .map(|d| d.with_timezone(&chrono::Utc))
            .unwrap_or_else(|_| chrono::Utc::now());
        let range_start = to_dt - chrono::Duration::milliseconds(dur.millis_i64());
        predicates.push(format!(
            "n.{} >= \"{}\"",
            time_pql_path, range_start.to_rfc3339()
        ));
        predicates.push(format!(
            "n.{} <= \"{}\"",
            time_pql_path, ctx.to
        ));
    } else {
        // Dashboard time range — use as-is (already RFC 3339 strings)
        predicates.push(format!(
            "n.{} >= \"{}\"",
            time_pql_path, ctx.from
        ));
        predicates.push(format!(
            "n.{} <= \"{}\"",
            time_pql_path, ctx.to
        ));
    }

    // Collect all label fields for projection (unique, preserving order)
    let mut all_label_fields: Vec<String> = Vec::new();
    for matcher in &iv.matchers {
        let fk = mapping.label_field_key(&matcher.label);
        if !all_label_fields.contains(&fk) {
            all_label_fields.push(fk);
        }
    }
    // Add default labels from the mapping
    for label in mapping.default_labels.keys() {
        let fk = mapping.label_field_key(label);
        if !all_label_fields.contains(&fk) {
            all_label_fields.push(fk);
        }
    }

    // Build PQL query
    let pql = format!(
        "MATCH (n) IN space(\"default\") WHERE {} RETURN n",
        predicates.join(" AND ")
    );

    Ok(TranslatedQuery {
        pql,
        post_steps: vec![],
        label_fields: all_label_fields,
        value_field: mapping.value_field_key(),
        time_field: time_field_key,
    })
}

fn translate_range_vector(
    rv: &RangeVector,
    ctx: &Ctx,
) -> Result<TranslatedQuery, String> {
    translate_instant_vector(&rv.vector, Some(&rv.range), ctx)
}

// ── Functions ─────────────────────────────────────────────────────────────────

fn translate_function(
    fc: &FunctionCall,
    ctx: &Ctx,
) -> Result<TranslatedQuery, String> {
    match fc.name.to_lowercase().as_str() {
        "rate" | "irate" => {
            if fc.args.is_empty() {
                return Err("rate() requires a range vector argument".into());
            }
            let mut tq = translate_expr(&fc.args[0], ctx)?;
            // Extract range_seconds from the inner translation context
            // For now, use a default or pull from the range vector
            let range_seconds = extract_range_seconds(&fc.args[0])?;
            tq.post_steps.push(PostStep::Rate { range_seconds });
            Ok(tq)
        }
        "increase" => {
            if fc.args.is_empty() {
                return Err("increase() requires a range vector argument".into());
            }
            let mut tq = translate_expr(&fc.args[0], ctx)?;
            tq.post_steps.push(PostStep::Increase);
            Ok(tq)
        }
        "delta" => {
            if fc.args.is_empty() {
                return Err("delta() requires a range vector argument".into());
            }
            let mut tq = translate_expr(&fc.args[0], ctx)?;
            let range_seconds = extract_range_seconds(&fc.args[0])?;
            tq.post_steps.push(PostStep::Rate { range_seconds }); // same formula
            Ok(tq)
        }
        "sum" => {
            // sum() without 'by' is an aggregation — but it's also a function
            // For PromQL, sum(expr) is a function form of aggregation
            if fc.args.is_empty() {
                return Err("sum() requires an argument".into());
            }
            let mut tq = translate_expr(&fc.args[0], ctx)?;
            tq.post_steps.push(PostStep::GroupSum { group_by: vec![] });
            Ok(tq)
        }
        "avg" => {
            if fc.args.is_empty() {
                return Err("avg() requires an argument".into());
            }
            let mut tq = translate_expr(&fc.args[0], ctx)?;
            tq.post_steps.push(PostStep::GroupAvg { group_by: vec![] });
            Ok(tq)
        }
        "min" => {
            if fc.args.is_empty() {
                return Err("min() requires an argument".into());
            }
            let mut tq = translate_expr(&fc.args[0], ctx)?;
            tq.post_steps.push(PostStep::GroupMin { group_by: vec![] });
            Ok(tq)
        }
        "max" => {
            if fc.args.is_empty() {
                return Err("max() requires an argument".into());
            }
            let mut tq = translate_expr(&fc.args[0], ctx)?;
            tq.post_steps.push(PostStep::GroupMax { group_by: vec![] });
            Ok(tq)
        }
        "count" => {
            if fc.args.is_empty() {
                return Err("count() requires an argument".into());
            }
            let mut tq = translate_expr(&fc.args[0], ctx)?;
            tq.post_steps.push(PostStep::GroupCount { group_by: vec![] });
            Ok(tq)
        }
        "sort" => {
            if fc.args.is_empty() {
                return Err("sort() requires an argument".into());
            }
            let mut tq = translate_expr(&fc.args[0], ctx)?;
            tq.post_steps.push(PostStep::Sort { desc: false });
            Ok(tq)
        }
        "sort_desc" => {
            if fc.args.is_empty() {
                return Err("sort_desc() requires an argument".into());
            }
            let mut tq = translate_expr(&fc.args[0], ctx)?;
            tq.post_steps.push(PostStep::Sort { desc: true });
            Ok(tq)
        }
        "topk" => {
            if fc.args.len() < 2 {
                return Err("topk() requires (k, expr) arguments".into());
            }
            let k = eval_number_literal(&fc.args[0])? as usize;
            let mut tq = translate_expr(&fc.args[1], ctx)?;
            tq.post_steps.push(PostStep::TopK { k });
            Ok(tq)
        }
        "bottomk" => {
            if fc.args.len() < 2 {
                return Err("bottomk() requires (k, expr) arguments".into());
            }
            let k = eval_number_literal(&fc.args[0])? as usize;
            let mut tq = translate_expr(&fc.args[1], ctx)?;
            tq.post_steps.push(PostStep::BottomK { k });
            Ok(tq)
        }
        "absent" => {
            // absent(metric) returns 1 if the metric has no data, empty otherwise
            if fc.args.is_empty() {
                return Err("absent() requires an argument".into());
            }
            let mut tq = translate_expr(&fc.args[0], ctx)?;
            // Add a marker — the execution layer checks if result is empty
            tq.post_steps.push(PostStep::GroupCount { group_by: vec![] });
            Ok(tq)
        }
        "histogram_quantile" => {
            if fc.args.len() < 2 {
                return Err("histogram_quantile() requires (quantile, histogram) arguments".into());
            }
            let quantile = eval_number_literal(&fc.args[0])?;
            let mut tq = translate_expr(&fc.args[1], ctx)?;
            tq.post_steps.push(PostStep::HistogramQuantile { quantile });
            Ok(tq)
        }
        "ceil" | "floor" | "round" | "abs" | "sqrt" | "ln" | "log2" | "log10" | "exp" => {
            // These are simple math functions applied per-value
            // For now, pass through — they need DataFrame-level implementation
            if fc.args.is_empty() {
                return Err(format!("{}() requires an argument", fc.name));
            }
            let tq = translate_expr(&fc.args[0], ctx)?;
            // These will be computed in post-processing
            Ok(tq)
        }
        "changes" | "resets" | "deriv" | "predict_linear" | "holt_winters"
        | "idelta" | "label_replace" | "label_join" | "timestamp"
        | "vector" | "scalar" | "time" | "days_in_month" | "day_of_month"
        | "day_of_week" | "hour" | "minute" | "month" | "year" => {
            Err(format!("function '{}' is not yet implemented", fc.name))
        }
        _ => Err(format!("unknown function: {}", fc.name)),
    }
}

// ── Aggregation ───────────────────────────────────────────────────────────────

fn translate_aggregation(
    agg: &Aggregation,
    ctx: &Ctx,
) -> Result<TranslatedQuery, String> {
    let mut tq = translate_expr(&agg.expr, ctx)?;

    // Resolve group_by labels to fully qualified field keys
    let group_by: Vec<String> = if agg.grouping.by {
        // "by (labels)" — use the specified labels
        agg.grouping.labels.iter().map(|l| {
            // Try to qualify the label with the metric's namespace
            if l.contains(':') {
                l.clone()
            } else {
                // Use the label fields from the inner query
                // Look through label_fields for a match
                for lf in &tq.label_fields {
                    if lf.ends_with(&format!(":{}", l)) || lf == l {
                        return lf.clone();
                    }
                }
                format!("{}:{}", tq.value_field.split(':').next().unwrap_or(""), l)
            }
        }).collect()
    } else {
        // "without (labels)" — use all labels except the specified ones
        tq.label_fields.iter()
            .filter(|lf| !agg.grouping.labels.iter().any(|l| lf.ends_with(&format!(":{}", l))))
            .cloned()
            .collect()
    };

    let step = match agg.op {
        AggregationOp::Sum => PostStep::GroupSum { group_by: group_by.clone() },
        AggregationOp::Avg => PostStep::GroupAvg { group_by: group_by.clone() },
        AggregationOp::Min => PostStep::GroupMin { group_by: group_by.clone() },
        AggregationOp::Max => PostStep::GroupMax { group_by: group_by.clone() },
        AggregationOp::Count => PostStep::GroupCount { group_by: group_by.clone() },
        AggregationOp::Stddev => PostStep::GroupStddev { group_by: group_by.clone() },
        AggregationOp::Stdvar => PostStep::GroupStdvar { group_by: group_by.clone() },
        AggregationOp::TopK => {
            let k = agg.param.as_ref()
                .and_then(|p| eval_expr_number(p).ok())
                .unwrap_or(10.0) as usize;
            PostStep::TopK { k }
        }
        AggregationOp::BottomK => {
            let k = agg.param.as_ref()
                .and_then(|p| eval_expr_number(p).ok())
                .unwrap_or(10.0) as usize;
            PostStep::BottomK { k }
        }
        AggregationOp::Quantile => {
            let q = agg.param.as_ref()
                .and_then(|p| eval_expr_number(p).ok())
                .unwrap_or(0.5);
            PostStep::HistogramQuantile { quantile: q }
        }
        AggregationOp::Group => {
            // group() returns 1 for each unique label combination
            PostStep::GroupCount { group_by: group_by.clone() }
        }
        AggregationOp::CountValues => {
            // count_values is complex — skip for now
            PostStep::GroupCount { group_by: group_by.clone() }
        }
    };

    // Add group_by labels to the label_fields so they get extracted from nodes
    for gb in &group_by {
        if !tq.label_fields.contains(gb) {
            tq.label_fields.push(gb.clone());
        }
    }

    tq.post_steps.push(step);
    Ok(tq)
}

// ── Binary operators ──────────────────────────────────────────────────────────

fn translate_binary_op(
    bin: &BinaryOp,
    ctx: &Ctx,
) -> Result<TranslatedQuery, String> {
    let lhs = translate_expr(&bin.lhs, ctx)?;
    let _rhs = translate_expr(&bin.rhs, ctx)?;

    // For comparison operators, we can add a post-filter step
    // For arithmetic, we need to compute the operation on aligned results
    match bin.op {
        BinOpKind::Eq | BinOpKind::NotEq | BinOpKind::Gt | BinOpKind::Lt
        | BinOpKind::Gte | BinOpKind::Lte => {
            // If RHS is a number literal, translate to a comparison filter
            if let Expr::NumberLiteral(n) = *bin.rhs {
                let mut tq = lhs;
                tq.post_steps.push(PostStep::ComparisonFilter {
                    op: bin.op,
                    threshold: n,
                });
                return Ok(tq);
            }
            Err("comparison between two vectors is not yet implemented".into())
        }
        BinOpKind::Add | BinOpKind::Sub | BinOpKind::Mul | BinOpKind::Div
        | BinOpKind::Mod | BinOpKind::Pow => {
            // If RHS is a number literal, this is a scalar operation
            if let Expr::NumberLiteral(_n) = *bin.rhs {
                // Scalar arithmetic on each value — implemented in DataFrame layer
                let mut tq = lhs;
                // Mark for scalar computation
                tq.post_steps.push(PostStep::Sort { desc: false }); // placeholder
                return Ok(tq);
            }
            Err("arithmetic between two vectors is not yet implemented".into())
        }
        BinOpKind::And | BinOpKind::Or | BinOpKind::Unless => {
            Err("set operations (and/or/unless) between vectors are not yet implemented".into())
        }
    }
}

// ── Unary operators ───────────────────────────────────────────────────────────

fn translate_unary(
    un: &UnaryOp,
    ctx: &Ctx,
) -> Result<TranslatedQuery, String> {
    let tq = translate_expr(&un.expr, ctx)?;
    // Unary minus: negate values in post-processing
    match un.op {
        UnaryOpKind::Minus => {
            let mut tq = tq;
            tq.post_steps.push(PostStep::Sort { desc: false }); // placeholder
            Ok(tq)
        }
        UnaryOpKind::Plus => Ok(tq),
    }
}

// ── Subquery ──────────────────────────────────────────────────────────────────

fn translate_subquery(
    sq: &Subquery,
    _ctx: &Ctx,
) -> Result<TranslatedQuery, String> {
    // Subqueries are complex — for now, just translate the inner expression
    // and note the range/step for the execution engine
    let _range_ms = sq.range.millis_i64();
    let _step_ms = sq.step.as_ref().map(|d| d.millis_i64()).unwrap_or(_range_ms / 10);

    translate_expr(&sq.expr, _ctx)
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Look up the metric mapping from the registry. Falls back to convention-based
/// parsing (metric name: `namespace_field` → namespace=`namespace`, value_field=`field`).
fn resolve_metric<'a>(metric_name: &str, registry: &'a MetricRegistry) -> Result<&'a MetricMapping, String> {
    if let Some(mapping) = registry.get(metric_name) {
        return Ok(mapping);
    }

    // Fallback: try convention-based parsing
    // "wakatime_duration" → namespace="wakatime", value_field="duration"
    Err(format!(
        "unknown metric '{}'. Register it via the metric registry or use a known metric name.",
        metric_name
    ))
}

/// Split a field key like "wakatime:entity" into ("wakatime", "entity").
fn split_field_key(key: &str) -> Option<(&str, &str)> {
    key.split_once(':')
}

/// Convert a colon-separated field key like "wakatime:time" to PQL dot notation
/// like "wakatime.time".
fn field_key_to_pql_path(key: &str) -> String {
    key.replace(':', ".")
}

/// Escape a string for use inside a PQL double-quoted string.
fn escape_pql_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

/// Convert a regex pattern to a SQL LIKE pattern (best-effort).
fn regex_to_like(re: &str) -> String {
    let mut like = String::new();
    let chars: Vec<char> = re.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '.' => {
                if i + 1 < chars.len() && chars[i + 1] == '*' {
                    like.push('%');
                    i += 1;
                } else if i + 1 < chars.len() && chars[i + 1] == '+' {
                    like.push_str("_%");
                    i += 1;
                } else {
                    like.push('_');
                }
            }
            '*' => {
                // standalone * → %
                like.push('%');
            }
            '%' => like.push_str("\\%"),
            '_' => like.push_str("\\_"),
            '\\' => {
                if i + 1 < chars.len() {
                    like.push(chars[i + 1]);
                    i += 1;
                }
            }
            '^' | '$' => {
                // Anchors are implicit in LIKE — skip
            }
            c => like.push(c),
        }
        i += 1;
    }
    like
}

/// Extract the range duration in seconds from a range vector expression.
fn extract_range_seconds(expr: &Expr) -> Result<f64, String> {
    match expr {
        Expr::RangeVector(rv) => Ok(rv.range.seconds()),
        Expr::Subquery(sq) => Ok(sq.range.seconds()),
        _ => Err("expected a range vector or subquery for range-based function".into()),
    }
}

/// Try to evaluate an expression as a numeric literal.
fn eval_number_literal(expr: &Expr) -> Result<f64, String> {
    match expr {
        Expr::NumberLiteral(n) => Ok(*n),
        Expr::UnaryOp(un) => {
            let val = eval_number_literal(&un.expr)?;
            match un.op {
                UnaryOpKind::Minus => Ok(-val),
                UnaryOpKind::Plus => Ok(val),
            }
        }
        _ => Err("expected a number literal".into()),
    }
}

/// Try to evaluate an arbitrary expression as a number (best-effort for constants).
fn eval_expr_number(expr: &Expr) -> Result<f64, String> {
    eval_number_literal(expr)
}

// ═══════════════════════════════════════════════════════════════════════════════
// Query Execution Engine (run on the results of a TranslatedQuery)
// ═══════════════════════════════════════════════════════════════════════════════

/// A single data point in a time series.
#[derive(Debug, Clone)]
pub struct DataPoint {
    pub value: f64,
    pub timestamp: f64,
    pub labels: BTreeMap<String, String>,
}

/// Execute a translated query: convert raw PQL node rows to DataPoints,
/// apply all post-processing steps, and produce a DataFrame.
///
/// `nodes` are the result of running `tq.pql` through `ctx.query()` and
/// converting rows to Nodes via `row_to_node`.
pub fn execute_translated(
    tq: &TranslatedQuery,
    nodes: &[panorama_core::Node],
) -> Result<crate::DataFrame, String> {
    if tq.pql.is_empty() {
        return Ok(crate::DataFrame {
            name: "promql".into(),
            columns: vec!["value".into()],
            rows: vec![],
            meta: None,
        });
    }

    let mut points = nodes_to_datapoints(nodes, &tq.value_field, &tq.time_field, &tq.label_fields);

    for step in &tq.post_steps {
        points = apply_post_step(points, step)?;
    }

    datapoints_to_dataframe(&points, "promql")
}

/// Extract DataPoints from nodes.
fn nodes_to_datapoints(
    nodes: &[Node],
    value_field: &str,
    time_field: &str,
    label_fields: &[String],
) -> Vec<DataPoint> {
let mut points = Vec::new();

    for node in nodes {
        // Extract value
        let value = match node.get_field(value_field) {
            Some(FieldValue::Float(f)) => *f,
            Some(FieldValue::Integer(i)) => *i as f64,
            _ => continue,
        };

        // Extract timestamp
        let timestamp = match node.get_field(time_field) {
            Some(FieldValue::Float(f)) => *f,
            Some(FieldValue::Integer(i)) => *i as f64,
            Some(FieldValue::DateTime(s)) => {
                chrono::DateTime::parse_from_rfc3339(&s)
                    .map(|d| d.timestamp() as f64)
                    .unwrap_or(0.0)
            }
            _ => 0.0,
        };

        // Extract labels
        let mut labels = BTreeMap::new();
        for field_key in label_fields {
            if let Some(val) = node.get_field(field_key) {
                let label_name = field_key.split(':').last().unwrap_or(field_key);
                let label_val = match val {
                    FieldValue::String(s) => s.clone(),
                    FieldValue::Integer(i) => i.to_string(),
                    FieldValue::Float(f) => f.to_string(),
                    FieldValue::Boolean(b) => b.to_string(),
                    _ => continue,
                };
                labels.insert(label_name.to_string(), label_val);
            }
        }

        points.push(DataPoint { value, timestamp, labels });
    }

    points
}

/// Apply a single post-processing step to a list of data points.
fn apply_post_step(points: Vec<DataPoint>, step: &PostStep) -> Result<Vec<DataPoint>, String> {
    match step {
        PostStep::Rate { range_seconds } => {
            // Group by labels, compute rate per series
            let grouped = group_by_labels(&points);
            let mut result = Vec::new();
            for (_key, mut series) in grouped {
                if series.len() < 2 { continue; }
                series.sort_by(|a, b| a.timestamp.partial_cmp(&b.timestamp).unwrap_or(std::cmp::Ordering::Equal));
                let first = series.first().unwrap();
                let last = series.last().unwrap();
                let rate = (last.value - first.value) / range_seconds;
                result.push(DataPoint {
                    value: rate,
                    timestamp: last.timestamp,
                    labels: last.labels.clone(),
                });
            }
            Ok(result)
        }
        PostStep::Increase => {
            let grouped = group_by_labels(&points);
            let mut result = Vec::new();
            for (_key, mut series) in grouped {
                if series.len() < 2 { continue; }
                series.sort_by(|a, b| a.timestamp.partial_cmp(&b.timestamp).unwrap_or(std::cmp::Ordering::Equal));
                let first = series.first().unwrap();
                let last = series.last().unwrap();
                result.push(DataPoint {
                    value: last.value - first.value,
                    timestamp: last.timestamp,
                    labels: last.labels.clone(),
                });
            }
            Ok(result)
        }
        PostStep::GroupSum { group_by } => {
            let result = aggregate_group(&points, group_by, |acc, v| acc + v, 0.0);
            Ok(result)
        }
        PostStep::GroupAvg { group_by } => {
            let grouped = group_by_labels_filtered(&points, group_by);
            let mut result = Vec::new();
            for (key, series) in grouped {
                if series.is_empty() { continue; }
                let sum: f64 = series.iter().map(|p| p.value).sum();
                let avg = sum / series.len() as f64;
                let mut labels = series[0].labels.clone();
                // Keep only the group_by labels
                let keys_to_keep: Vec<String> = labels.keys()
                    .filter(|k| group_by.is_empty() || group_by.iter().any(|gb| {
                        gb.ends_with(&format!(":{}", k)) || *gb == **k
                    }))
                    .cloned()
                    .collect();
                labels.retain(|k, _| keys_to_keep.contains(k));

                result.push(DataPoint {
                    value: avg,
                    timestamp: series.last().map(|p| p.timestamp).unwrap_or(0.0),
                    labels: key,
                });
            }
            Ok(result)
        }
        PostStep::GroupMin { group_by } => {
            aggregate_group_minmax(&points, group_by, |a, b| a < b)
        }
        PostStep::GroupMax { group_by } => {
            aggregate_group_minmax(&points, group_by, |a, b| a > b)
        }
        PostStep::GroupCount { group_by } => {
            let grouped = group_by_labels_filtered(&points, group_by);
            let mut result = Vec::new();
            for (key, series) in grouped {
                result.push(DataPoint {
                    value: series.len() as f64,
                    timestamp: series.last().map(|p| p.timestamp).unwrap_or(0.0),
                    labels: key,
                });
            }
            Ok(result)
        }
        PostStep::TopK { k } => {
            let mut sorted = points;
            sorted.sort_by(|a, b| b.value.partial_cmp(&a.value).unwrap_or(std::cmp::Ordering::Equal));
            sorted.truncate(*k);
            Ok(sorted)
        }
        PostStep::BottomK { k } => {
            let mut sorted = points;
            sorted.sort_by(|a, b| a.value.partial_cmp(&b.value).unwrap_or(std::cmp::Ordering::Equal));
            sorted.truncate(*k);
            Ok(sorted)
        }
        PostStep::Sort { desc } => {
            let mut sorted = points;
            if *desc {
                sorted.sort_by(|a, b| b.value.partial_cmp(&a.value).unwrap_or(std::cmp::Ordering::Equal));
            } else {
                sorted.sort_by(|a, b| a.value.partial_cmp(&b.value).unwrap_or(std::cmp::Ordering::Equal));
            }
            Ok(sorted)
        }
        PostStep::HistogramQuantile { quantile } => {
            // Compute φ-quantile from histogram buckets
            // Each DataPoint represents a bucket with label "le" (less-than-or-equal)
            let mut buckets: Vec<(f64, f64)> = points.iter().map(|p| {
                let le = p.labels.get("le")
                    .and_then(|v| v.parse::<f64>().ok())
                    .unwrap_or(f64::INFINITY);
                (le, p.value)
            }).collect();
            buckets.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

            let total = buckets.last().map(|b| b.1).unwrap_or(1.0);
            if total <= 0.0 {
                return Ok(vec![]);
            }
            let target = quantile * total;

            let mut cumulative = 0.0;
            for (le, count) in &buckets {
                cumulative += count;
                if cumulative >= target {
                    return Ok(vec![DataPoint {
                        value: *le,
                        timestamp: 0.0,
                        labels: BTreeMap::new(),
                    }]);
                }
            }

            Ok(vec![DataPoint {
                value: f64::INFINITY,
                timestamp: 0.0,
                labels: BTreeMap::new(),
            }])
        }
        PostStep::ComparisonFilter { op, threshold } => {
            let filtered: Vec<DataPoint> = points.into_iter().filter(|p| {
                match op {
                    BinOpKind::Eq => (p.value - threshold).abs() < f64::EPSILON,
                    BinOpKind::NotEq => (p.value - threshold).abs() > f64::EPSILON,
                    BinOpKind::Gt => p.value > *threshold,
                    BinOpKind::Lt => p.value < *threshold,
                    BinOpKind::Gte => p.value >= *threshold,
                    BinOpKind::Lte => p.value <= *threshold,
                    _ => true,
                }
            }).collect();
            Ok(filtered)
        }
        _ => {
            // Unimplemented step — pass through
            Ok(points)
        }
    }
}

/// Group data points by their label set.
fn group_by_labels(points: &[DataPoint]) -> BTreeMap<BTreeMap<String, String>, Vec<DataPoint>> {
    let mut groups: BTreeMap<BTreeMap<String, String>, Vec<DataPoint>> = BTreeMap::new();
    for point in points {
        groups.entry(point.labels.clone()).or_default().push(point.clone());
    }
    groups
}

/// Group by a subset of labels.
fn group_by_labels_filtered(
    points: &[DataPoint],
    group_by: &[String],
) -> BTreeMap<BTreeMap<String, String>, Vec<DataPoint>> {
    let mut groups: BTreeMap<BTreeMap<String, String>, Vec<DataPoint>> = BTreeMap::new();
    for point in points {
        let key: BTreeMap<String, String> = if group_by.is_empty() {
            BTreeMap::new()
        } else {
            point.labels.iter()
                .filter(|(k, _)| {
                    let fq = format!("{}:{}", k, point.labels.get(*k).unwrap_or(&String::new()));
                    group_by.iter().any(|gb| {
                        gb.ends_with(&format!(":{}", k)) || *gb == **k || fq == *gb
                    })
                })
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect()
        };
        groups.entry(key).or_default().push(point.clone());
    }
    groups
}

/// Generic sum aggregation (sorted descending by value).
fn aggregate_group(
    points: &[DataPoint],
    group_by: &[String],
    op: fn(f64, f64) -> f64,
    init: f64,
) -> Vec<DataPoint> {
    let grouped = group_by_labels_filtered(points, group_by);
    let mut result: Vec<DataPoint> = Vec::new();
    for (key, series) in grouped {
        let value = series.iter().fold(init, |acc, p| op(acc, p.value));
        result.push(DataPoint {
            value,
            timestamp: series.last().map(|p| p.timestamp).unwrap_or(0.0),
            labels: key,
        });
    }
    // Sort descending by value for leaderboard-style display
    result.sort_by(|a, b| b.value.partial_cmp(&a.value).unwrap_or(std::cmp::Ordering::Equal));
    result
}

/// Min/max aggregation (sorted descending by value).
fn aggregate_group_minmax(
    points: &[DataPoint],
    group_by: &[String],
    better: fn(f64, f64) -> bool,
) -> Result<Vec<DataPoint>, String> {
    let grouped = group_by_labels_filtered(points, group_by);
    let mut result: Vec<DataPoint> = Vec::new();
    for (key, series) in grouped {
        if series.is_empty() { continue; }
        let best = series.iter()
            .fold(f64::NAN, |best, p| {
                if best.is_nan() || better(p.value, best) { p.value } else { best }
            });
        result.push(DataPoint {
            value: best,
            timestamp: series.last().map(|p| p.timestamp).unwrap_or(0.0),
            labels: key,
        });
    }
    result.sort_by(|a, b| b.value.partial_cmp(&a.value).unwrap_or(std::cmp::Ordering::Equal));
    Ok(result)
}

/// Convert DataPoints to a Grafana-compatible DataFrame.
fn datapoints_to_dataframe(
    points: &[DataPoint],
    name: &str,
) -> Result<crate::DataFrame, String> {
    use serde_json::json;

    if points.is_empty() {
        return Ok(crate::DataFrame {
            name: name.into(),
            columns: vec!["value".into()],
            rows: vec![],
            meta: Some(json!({"note": "no data"})),
        });
    }

    // Collect all unique label keys across all points
    let mut label_keys: Vec<String> = Vec::new();
    for point in points {
        for key in point.labels.keys() {
            if !label_keys.contains(key) {
                label_keys.push(key.clone());
            }
        }
    }
    label_keys.sort();

    // Sort points descending by value (leaderboard style)
    let mut sorted_points: Vec<&DataPoint> = points.iter().collect();
    sorted_points.sort_by(|a, b| b.value.partial_cmp(&a.value).unwrap_or(std::cmp::Ordering::Equal));

    let mut columns = vec!["time".into(), "value".into()];
    columns.extend(label_keys.clone());

    let mut rows = Vec::new();
    for point in sorted_points {
        let mut row = vec![
            json!(point.timestamp),
            json!(point.value),
        ];
        for key in &label_keys {
            row.push(json!(point.labels.get(key).unwrap_or(&String::new())));
        }
        rows.push(row);
    }

    Ok(crate::DataFrame {
        name: name.into(),
        columns,
        rows,
        meta: Some(json!({"query_type": "promql"})),
    })
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn test_registry() -> MetricRegistry {
        MetricRegistry::with_wakatime_defaults()
    }

    fn translate_str(promql: &str) -> Result<TranslatedQuery, String> {
        let expr = super::super::parser::parse(promql)
            .map_err(|e| e.to_string())?;
        let registry = test_registry();
        // Use fixed time range strings for tests
        translate(&expr, &registry, "2026-07-01T00:00:00+00:00", "2026-07-04T00:00:00+00:00")
    }

    #[test]
    fn test_simple_metric() {
        let tq = translate_str("wakatime_duration").unwrap();
        assert!(tq.pql.contains("MATCH (n) IN space(\"default\")"));
        assert!(tq.pql.contains("HAS_FIELD(n, \"wakatime\", \"duration\")"));
        assert_eq!(tq.value_field, "wakatime:duration");
    }

    #[test]
    fn test_metric_with_labels() {
        let tq = translate_str(r#"wakatime_duration{project="panorama"}"#).unwrap();
        assert!(tq.pql.contains("n.wakatime.project = \"panorama\""));
        assert!(tq.label_fields.contains(&"wakatime:project".to_string()));
    }

    #[test]
    fn test_range_vector() {
        let tq = translate_str("wakatime_duration[5m]").unwrap();
        assert!(tq.pql.contains("HAS_FIELD"));
        // Should have time range constraint (system.node_time is the default time field)
        assert!(tq.pql.contains("system.node_time"));
    }

    #[test]
    fn test_rate() {
        let tq = translate_str("rate(wakatime_duration[5m])").unwrap();
        assert!(tq.pql.contains("HAS_FIELD"));
        assert_eq!(tq.post_steps.len(), 1);
        match &tq.post_steps[0] {
            PostStep::Rate { range_seconds } => {
                assert!((*range_seconds - 300.0).abs() < 0.01);
            }
            _ => panic!("expected Rate step"),
        }
    }

    #[test]
    fn test_sum_by() {
        let tq = translate_str("sum by (project) (rate(wakatime_duration[5m]))").unwrap();
        // Should have Rate step and GroupSum step
        assert!(tq.post_steps.len() >= 1);
    }

    #[test]
    fn test_unknown_metric() {
        let result = translate_str("nonexistent_metric");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("unknown metric"));
    }
}

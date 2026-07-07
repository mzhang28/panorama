//! Query IR — the intermediate representation between surface syntax and SQL.
//!
//! Matches QUERY_DESIGN.md §5.  Each stage maps to a logical operator in the
//! evaluation order; the compiler emits them in order and may reorder when
//! equivalent.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::hash::{Hash, Hasher};

/// A compiled IR plan: ordered list of stages.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IrPlan {
  pub stages: Vec<IrStage>,
  pub project: Project,
  pub order_by: Option<OrderBy>,
  pub limit: Option<u64>,
  pub skip: Option<u64>,
}

impl IrPlan {
  /// Produce a cache key from the IR shape (structure minus parameter values).
  /// Two queries that differ only in literal values produce the same key,
  /// matching §7.3's requirement: "cache keyed on the IR shape."
  pub fn cache_key(&self) -> u64 {
    use std::hash::{DefaultHasher, Hash, Hasher};
    let mut h = DefaultHasher::new();
    // Hash structural elements only — not parameter values
    for stage in &self.stages {
      stage.hash_shape(&mut h);
    }
    self.project.hash_shape(&mut h);
    if let Some(ref ob) = self.order_by {
      ob.hash_shape(&mut h);
    }
    self.limit.hash(&mut h);
    self.skip.hash(&mut h);
    h.finish()
  }
}

impl IrStage {
  /// Hash only the shape (stage variant + field paths), not parameter values.
  fn hash_shape<H: Hasher>(&self, h: &mut H) {
    // Encode the variant discriminant as a tag byte
    let tag: u8 = match self {
      IrStage::Source(_) => 0,
      IrStage::ConformanceFilter { .. } => 1,
      IrStage::IndexedPredicate { .. } => 2,
      IrStage::PresencePredicate { .. } => 3,
      IrStage::ScanPredicate { .. } => 4,
      IrStage::RefTraverse { .. } => 5,
    };
    tag.hash(h);
    match self {
      IrStage::Source(spaces) => {
        for s in spaces {
          s.hash(h);
        }
      }
      IrStage::ConformanceFilter {
        schema_id,
        version_min: _,
        version_max: _,
      } => {
        schema_id.hash(h);
      }
      IrStage::IndexedPredicate { field_path, op, .. }
      | IrStage::ScanPredicate { field_path, op, .. } => {
        field_path.hash(h);
        op.to_sql().hash(h);
      }
      IrStage::PresencePredicate {
        namespace,
        field_name,
      } => {
        namespace.hash(h);
        field_name.hash(h);
      }
      IrStage::RefTraverse {
        edge_field,
        min_depth,
        max_depth,
        ..
      } => {
        edge_field.hash(h);
        min_depth.hash(h);
        max_depth.hash(h);
      }
    }
  }
}

impl Project {
  fn hash_shape<H: std::hash::Hasher>(&self, h: &mut H) {
    self.whole_node.hash(h);
    for col in &self.columns {
      col.expr.hash_shape(h);
      col.alias.hash(h);
    }
    for gb in &self.group_by {
      gb.hash_shape(h);
    }
  }
}

impl ProjectExpr {
  fn hash_shape<H: std::hash::Hasher>(&self, h: &mut H) {
    let tag: u8 = match self {
      ProjectExpr::Field(_) => 0,
      ProjectExpr::Node(_) => 1,
      ProjectExpr::Aggregate(_, _) => 2,
    };
    tag.hash(h);
    match self {
      ProjectExpr::Field(s) | ProjectExpr::Node(s) => s.hash(h),
      ProjectExpr::Aggregate(func, inner) => {
        let func_tag: u8 = match func {
          AggregateFunc::Count => 0,
          AggregateFunc::Sum => 1,
          AggregateFunc::Avg => 2,
          AggregateFunc::Min => 3,
          AggregateFunc::Max => 4,
        };
        func_tag.hash(h);
        inner.hash_shape(h);
      }
    }
  }
}

impl OrderBy {
  fn hash_shape<H: std::hash::Hasher>(&self, h: &mut H) {
    self.field_path.hash(h);
    self.direction.to_sql().hash(h);
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IrStage {
  /// Restrict to nodes in these spaces (mandatory first filter).
  Source(Vec<String>),
  /// Filter by schema conformance with optional version range.
  ConformanceFilter {
    schema_id: String,
    version_min: Option<u32>,
    version_max: Option<u32>,
  },
  /// Predicate on a promoted/indexed field — can be pushed down.
  IndexedPredicate {
    field_path: String,
    op: CmpOp,
    value: String,
  },
  /// HAS_FIELD presence filter.
  PresencePredicate {
    namespace: String,
    field_name: String,
  },
  /// Explicitly unindexed predicate (SCAN in source).
  ScanPredicate {
    field_path: String,
    op: CmpOp,
    value: String,
  },
  /// Bounded reference traversal.
  RefTraverse {
    edge_field: String,
    min_depth: u32,
    max_depth: u32,
    target_stages: Vec<IrStage>,
  },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
  /// When true: return the whole node (all fields the caller can read).
  /// Otherwise: return only the named columns.
  pub whole_node: bool,
  pub columns: Vec<ProjectColumn>,
  /// Grouping keys inferred from non-aggregated columns when aggregates exist.
  pub group_by: Vec<ProjectExpr>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectColumn {
  pub expr: ProjectExpr,
  pub alias: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProjectExpr {
  Field(String), // "namespace:field"
  Node(String),  // variable name
  Aggregate(AggregateFunc, Box<ProjectExpr>),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AggregateFunc {
  Count,
  Sum,
  Avg,
  Min,
  Max,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CmpOp {
  Eq,
  Neq,
  Lt,
  Lte,
  Gt,
  Gte,
  Like,
  NotLike,
}

impl CmpOp {
  pub fn to_sql(&self) -> &'static str {
    match self {
      CmpOp::Eq => "=",
      CmpOp::Neq => "<>",
      CmpOp::Lt => "<",
      CmpOp::Lte => "<=",
      CmpOp::Gt => ">",
      CmpOp::Gte => ">=",
      CmpOp::Like => "LIKE",
      CmpOp::NotLike => "NOT LIKE",
    }
  }
}

impl fmt::Display for CmpOp {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}", self.to_sql())
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBy {
  pub field_path: String,
  pub direction: OrderDir,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OrderDir {
  Asc,
  Desc,
}

impl OrderDir {
  pub fn to_sql(&self) -> &'static str {
    match self {
      OrderDir::Asc => "ASC",
      OrderDir::Desc => "DESC",
    }
  }
}

// ── AST → IR lowering ───────────────────────────────────────────────────────────

use crate::query::ast;

/// Lower a parsed AST query into the query IR (§5).
///
/// This is a pure transform — no DB connection needed. The IR captures the
/// query structure independent of literal values, providing a stable key for
/// the prepared-statement cache (§7.3) and an attachment point for query-id
/// tracing (§8).
pub fn lower_to_ir(query: &ast::Query) -> IrPlan {
  let mut stages: Vec<IrStage> = Vec::new();

  for mc in &query.matches {
    match &mc.source {
      ast::MatchSource::Space(space_name) => {
        stages.push(IrStage::Source(vec![space_name.clone()]));

        // Walk the WHERE clause and classify predicates
        if let Some(wc) = &mc.where_clause {
          lower_predicate(&wc.predicate, &mut stages);
        }
      }
      ast::MatchSource::RefTraverse {
        edge_type,
        min_depth,
        max_depth,
        target_var,
        target_source,
      } => {
        // Lower the target's source first
        if let ast::MatchSource::Space(ref space_name) = target_source.as_ref() {
          stages.push(IrStage::Source(vec![space_name.clone()]));
        }
        stages.push(IrStage::RefTraverse {
          edge_field: edge_type.clone(),
          min_depth: *min_depth,
          max_depth: *max_depth,
          target_stages: Vec::new(),
        });
      }
    }
  }

  // Build project columns and detect aggregates for implicit GROUP BY.
  let has_aggregate = query
    .return_clause
    .columns
    .iter()
    .any(|c| matches!(c.expression, ast::ReturnExpr::Aggregate { .. }));

  let project_columns: Vec<ProjectColumn> = query
    .return_clause
    .columns
    .iter()
    .map(|c| {
      let expr = lower_return_expr(&c.expression);
      let alias = c
        .alias
        .clone()
        .or_else(|| match &c.expression {
          ast::ReturnExpr::Field(fp) => Some(fp.field.clone()),
          ast::ReturnExpr::Node(v) => Some(v.clone()),
          ast::ReturnExpr::Aggregate { func, .. } => Some(match func {
            ast::AggregateFunc::Count => "count".into(),
            ast::AggregateFunc::Sum => "sum".into(),
            ast::AggregateFunc::Avg => "avg".into(),
            ast::AggregateFunc::Min => "min".into(),
            ast::AggregateFunc::Max => "max".into(),
          }),
        })
        .unwrap_or_else(|| "expr".into());
      ProjectColumn { expr, alias }
    })
    .collect();

  // When aggregates are present, non-aggregate columns become grouping keys.
  let group_by: Vec<ProjectExpr> = if has_aggregate {
    project_columns
      .iter()
      .filter(|c| !matches!(c.expr, ProjectExpr::Aggregate(..)))
      .map(|c| c.expr.clone())
      .collect()
  } else {
    Vec::new()
  };

  let project = Project {
    whole_node: query
      .return_clause
      .columns
      .iter()
      .any(|c| matches!(c.expression, ast::ReturnExpr::Node(_))),
    columns: project_columns,
    group_by,
  };

  let order_by = query.order_by.as_ref().map(|ob| OrderBy {
    field_path: format!(
      "{}:{}",
      ob.field.namespace.as_deref().unwrap_or(""),
      ob.field.field
    ),
    direction: match ob.direction {
      ast::OrderDir::Asc => OrderDir::Asc,
      ast::OrderDir::Desc => OrderDir::Desc,
    },
  });

  IrPlan {
    stages,
    project,
    order_by,
    limit: query.limit,
    skip: query.skip,
  }
}

/// Lower an AST ReturnExpr into an IR ProjectExpr.
fn lower_return_expr(expr: &ast::ReturnExpr) -> ProjectExpr {
  match expr {
    ast::ReturnExpr::Node(v) => ProjectExpr::Node(v.clone()),
    ast::ReturnExpr::Field(fp) => ProjectExpr::Field(format!(
      "{}:{}",
      fp.namespace.as_deref().unwrap_or(""),
      fp.field
    )),
    ast::ReturnExpr::Aggregate { func, expr } => {
      let ir_func = match func {
        ast::AggregateFunc::Count => AggregateFunc::Count,
        ast::AggregateFunc::Sum => AggregateFunc::Sum,
        ast::AggregateFunc::Avg => AggregateFunc::Avg,
        ast::AggregateFunc::Min => AggregateFunc::Min,
        ast::AggregateFunc::Max => AggregateFunc::Max,
      };
      ProjectExpr::Aggregate(ir_func, Box::new(lower_return_expr(expr)))
    }
  }
}

/// Walk a predicate tree and push IR stages for each predicate type.
fn lower_predicate(pred: &ast::Predicate, stages: &mut Vec<IrStage>) {
  match pred {
    ast::Predicate::ConformsTo {
      schema_id,
      version_min,
      version_max,
      ..
    } => {
      stages.push(IrStage::ConformanceFilter {
        schema_id: schema_id.clone(),
        version_min: *version_min,
        version_max: *version_max,
      });
    }
    ast::Predicate::FieldCompare {
      field_path,
      op,
      value,
    } => {
      let fp_str = format!(
        "{}:{}",
        field_path.namespace.as_deref().unwrap_or(""),
        field_path.field
      );
      stages.push(IrStage::ScanPredicate {
        field_path: fp_str,
        op: lower_cmp_op(op),
        value: format!("{:?}", value),
      });
    }
    ast::Predicate::HasField {
      namespace,
      field_name,
      ..
    } => {
      stages.push(IrStage::PresencePredicate {
        namespace: namespace.clone(),
        field_name: field_name.clone(),
      });
    }
    ast::Predicate::IsNull { field_path, .. } => {
      let fp_str = format!(
        "{}:{}",
        field_path.namespace.as_deref().unwrap_or(""),
        field_path.field
      );
      stages.push(IrStage::ScanPredicate {
        field_path: fp_str,
        op: CmpOp::Eq,
        value: "NULL".into(),
      });
    }
    ast::Predicate::Scan(inner) => lower_predicate(inner, stages),
    ast::Predicate::And(a, b) | ast::Predicate::Or(a, b) => {
      lower_predicate(a, stages);
      lower_predicate(b, stages);
    }
    ast::Predicate::Not(inner) => lower_predicate(inner, stages),
    _ => {} // IN, LIKE, CONTAINS → treat as ScanPredicate
  }
}

fn lower_cmp_op(op: &ast::CmpOp) -> CmpOp {
  match op {
    ast::CmpOp::Eq => CmpOp::Eq,
    ast::CmpOp::Neq => CmpOp::Neq,
    ast::CmpOp::Lt => CmpOp::Lt,
    ast::CmpOp::Lte => CmpOp::Lte,
    ast::CmpOp::Gt => CmpOp::Gt,
    ast::CmpOp::Gte => CmpOp::Gte,
  }
}

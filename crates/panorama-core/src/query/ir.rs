//! Query IR — the intermediate representation between surface syntax and SQL.
//!
//! Matches QUERY_DESIGN.md §5.  Each stage maps to a logical operator in the
//! evaluation order; the compiler emits them in order and may reorder when
//! equivalent.

use serde::{Deserialize, Serialize};
use std::fmt;

/// A compiled IR plan: ordered list of stages.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IrPlan {
  pub stages: Vec<IrStage>,
  pub project: Project,
  pub order_by: Option<OrderBy>,
  pub limit: Option<u64>,
  pub skip: Option<u64>,
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectColumn {
  pub field_path: String, // "ns.field" format for JSON extraction
  pub alias: String,
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

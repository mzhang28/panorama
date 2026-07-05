//! AST types for the Panorama Query Language (v0).
//!
//! Mirrors the surface syntax described in QUERY_DESIGN.md §3.

use serde::{Deserialize, Serialize};

/// A complete query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Query {
    pub matches: Vec<MatchClause>,
    pub return_clause: ReturnClause,
    pub order_by: Option<OrderBy>,
    pub limit: Option<u64>,
    pub skip: Option<u64>,
}

/// `MATCH (var) IN space(...)`  or  `MATCH (a)-[:REF("f")]->(b) IN space(...)`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchClause {
    pub variable: String,
    pub source: MatchSource,
    pub where_clause: Option<WhereClause>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MatchSource {
    /// `IN space("name")` — bare node match
    Space(String),
    /// `(a)-[:REF("edge")]->(b) IN space("name")` — reference traversal
    RefTraverse {
        edge_type: String,
        min_depth: u32,
        max_depth: u32,
        target_var: String,
        target_source: Box<MatchSource>,
    },
}

/// The `WHERE` clause — a single predicate tree (AND binds tighter than OR).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhereClause {
    pub predicate: Predicate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Predicate {
    /// `n CONFORMS TO schema("id")` or with version constraint
    ConformsTo {
        field: String,        // variable name (usually "n")
        schema_id: String,
        version_min: Option<u32>,
        version_max: Option<u32>,
    },
    /// Field comparison: `n.ns.field OP value`
    FieldCompare {
        field_path: FieldPath,
        op: CmpOp,
        value: Value,
        /// True when wrapped in SCAN(...)
        scan: bool,
    },
    /// `HAS_FIELD(n, "ns", "name")` or `HAS_FIELD(n, "*", "name")`
    HasField {
        variable: String,
        namespace: String,
        field_name: String,
    },
    /// Boolean composition
    And(Box<Predicate>, Box<Predicate>),
    Or(Box<Predicate>, Box<Predicate>),
    Not(Box<Predicate>),
    /// `n.foo IS NULL` / `IS NOT NULL`
    IsNull { field_path: FieldPath, not: bool },
    /// Set membership: `n.foo IN [...]`
    In { field_path: FieldPath, values: Vec<Value>, not: bool },
    /// String pattern: `n.foo LIKE "pattern"`
    Like { field_path: FieldPath, pattern: String, not: bool },
}

/// A namespaced field path: `n."ns"."field"` or bare `n.field`
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct FieldPath {
    pub variable: String,
    pub namespace: Option<String>,
    pub field: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CmpOp {
    Eq, Neq, Lt, Lte, Gt, Gte,
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
        }
    }
}

/// A literal value in the query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Value {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Null,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReturnClause {
    pub columns: Vec<ReturnColumn>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReturnColumn {
    pub expression: ReturnExpr,
    pub alias: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReturnExpr {
    /// Whole node: `RETURN n`
    Node(String),
    /// Field projection: `RETURN n.field` or `n.ns.field`
    Field(FieldPath),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBy {
    pub field: FieldPath,
    pub direction: OrderDir,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OrderDir {
    Asc,
    Desc,
}

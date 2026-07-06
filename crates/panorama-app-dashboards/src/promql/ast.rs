//! AST types for PromQL (Prometheus Query Language).
//!
//! Mirrors the PromQL grammar from the Prometheus documentation.

use std::fmt;

// ── Top-level expression ──────────────────────────────────────────────────────

/// A PromQL expression.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// A literal number: `42`, `3.14`
    NumberLiteral(f64),
    /// A literal string: `"hello"`, `'world'`
    StringLiteral(String),
    /// An instant vector selector: `metric_name{label="value"}`
    InstantVector(InstantVector),
    /// A range vector selector: `metric_name{label="value"}[5m]`
    RangeVector(RangeVector),
    /// A function call: `rate(metric[5m])`
    FunctionCall(FunctionCall),
    /// An aggregation: `sum by (label) (expr)`
    Aggregation(Aggregation),
    /// A binary operation: `a + b`
    BinaryOp(BinaryOp),
    /// A unary operation: `-expr`
    UnaryOp(UnaryOp),
    /// A subquery: `metric[5m:1m]`
    Subquery(Subquery),
    /// A parenthesized expression: `(expr)`
    Paren(Box<Expr>),
}

// ── Vector selectors ──────────────────────────────────────────────────────────

/// An instant vector selector: `metric_name{label1="val1", label2=~"regex"}`
#[derive(Debug, Clone, PartialEq)]
pub struct InstantVector {
    /// The metric name (may be empty if only label matchers are given).
    pub metric_name: String,
    /// Label matchers.
    pub matchers: Vec<LabelMatcher>,
    /// Optional offset modifier: `offset 5m`
    pub offset: Option<Duration>,
    /// Optional `@` timestamp modifier.
    pub at: Option<AtModifier>,
}

/// A range vector selector: `metric{labels}[5m]` or `metric{labels}[5m] offset 1h`
#[derive(Debug, Clone, PartialEq)]
pub struct RangeVector {
    /// The underlying instant vector.
    pub vector: Box<InstantVector>,
    /// The time range.
    pub range: Duration,
}

/// A subquery: `metric[5m:1m]`, `rate(metric[5m])[1h:5m]`
#[derive(Debug, Clone, PartialEq)]
pub struct Subquery {
    /// The inner expression.
    pub expr: Box<Expr>,
    /// The range duration.
    pub range: Duration,
    /// The resolution step (optional, defaults to global evaluation interval).
    pub step: Option<Duration>,
    /// Optional offset modifier.
    pub offset: Option<Duration>,
}

// ── Label matching ────────────────────────────────────────────────────────────

/// A label matcher inside `{...}`.
#[derive(Debug, Clone, PartialEq)]
pub struct LabelMatcher {
    /// The label name.
    pub label: String,
    /// The match operator.
    pub op: MatchOp,
    /// The match value (or regex pattern).
    pub value: String,
}

/// Label match operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchOp {
    /// `=`
    Eq,
    /// `!=`
    NotEq,
    /// `=~` (regex match)
    Regex,
    /// `!~` (regex not match)
    NotRegex,
}

impl fmt::Display for MatchOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MatchOp::Eq => write!(f, "="),
            MatchOp::NotEq => write!(f, "!="),
            MatchOp::Regex => write!(f, "=~"),
            MatchOp::NotRegex => write!(f, "!~"),
        }
    }
}

// ── Functions ─────────────────────────────────────────────────────────────────

/// A function call: `function_name(arg1, arg2, ...)`
#[derive(Debug, Clone, PartialEq)]
pub struct FunctionCall {
    /// The function name (lowercase).
    pub name: String,
    /// Arguments to the function.
    pub args: Vec<Expr>,
}

// ── Aggregation ───────────────────────────────────────────────────────────────

/// An aggregation expression: `sum by (label1, label2) (expr)` or `avg without (label) (expr)`
#[derive(Debug, Clone, PartialEq)]
pub struct Aggregation {
    /// The aggregation operator.
    pub op: AggregationOp,
    /// The inner expression to aggregate.
    pub expr: Box<Expr>,
    /// Grouping clause.
    pub grouping: Grouping,
    /// Optional parameter (for topk, bottomk, quantile).
    pub param: Option<Box<Expr>>,
}

/// Aggregation operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AggregationOp {
    Sum,
    Avg,
    Min,
    Max,
    Count,
    Stddev,
    Stdvar,
    TopK,
    BottomK,
    CountValues,
    Quantile,
    Group,
}

impl fmt::Display for AggregationOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AggregationOp::Sum => write!(f, "sum"),
            AggregationOp::Avg => write!(f, "avg"),
            AggregationOp::Min => write!(f, "min"),
            AggregationOp::Max => write!(f, "max"),
            AggregationOp::Count => write!(f, "count"),
            AggregationOp::Stddev => write!(f, "stddev"),
            AggregationOp::Stdvar => write!(f, "stdvar"),
            AggregationOp::TopK => write!(f, "topk"),
            AggregationOp::BottomK => write!(f, "bottomk"),
            AggregationOp::CountValues => write!(f, "count_values"),
            AggregationOp::Quantile => write!(f, "quantile"),
            AggregationOp::Group => write!(f, "group"),
        }
    }
}

impl AggregationOp {
    /// Parse an aggregation operator from a keyword string.
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "sum" => Some(AggregationOp::Sum),
            "avg" => Some(AggregationOp::Avg),
            "min" => Some(AggregationOp::Min),
            "max" => Some(AggregationOp::Max),
            "count" => Some(AggregationOp::Count),
            "stddev" => Some(AggregationOp::Stddev),
            "stdvar" => Some(AggregationOp::Stdvar),
            "topk" => Some(AggregationOp::TopK),
            "bottomk" => Some(AggregationOp::BottomK),
            "count_values" => Some(AggregationOp::CountValues),
            "quantile" => Some(AggregationOp::Quantile),
            "group" => Some(AggregationOp::Group),
            _ => None,
        }
    }
}

/// Grouping clause for aggregation.
#[derive(Debug, Clone, PartialEq)]
pub struct Grouping {
    /// `true` for `by (labels)`, `false` for `without (labels)`
    pub by: bool,
    /// The labels in the grouping clause.
    pub labels: Vec<String>,
}

// ── Binary operators ──────────────────────────────────────────────────────────

/// A binary operation: `lhs op rhs`
#[derive(Debug, Clone, PartialEq)]
pub struct BinaryOp {
    /// Left-hand side expression.
    pub lhs: Box<Expr>,
    /// Right-hand side expression.
    pub rhs: Box<Expr>,
    /// The binary operator.
    pub op: BinOpKind,
    /// Optional vector matching clause.
    pub matching: Option<VectorMatching>,
}

/// Binary operator kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOpKind {
    // Arithmetic
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
    // Comparison
    Eq,
    NotEq,
    Gt,
    Lt,
    Gte,
    Lte,
    // Logical / set
    And,
    Or,
    Unless,
}

impl fmt::Display for BinOpKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BinOpKind::Add => write!(f, "+"),
            BinOpKind::Sub => write!(f, "-"),
            BinOpKind::Mul => write!(f, "*"),
            BinOpKind::Div => write!(f, "/"),
            BinOpKind::Mod => write!(f, "%"),
            BinOpKind::Pow => write!(f, "^"),
            BinOpKind::Eq => write!(f, "=="),
            BinOpKind::NotEq => write!(f, "!="),
            BinOpKind::Gt => write!(f, ">"),
            BinOpKind::Lt => write!(f, "<"),
            BinOpKind::Gte => write!(f, ">="),
            BinOpKind::Lte => write!(f, "<="),
            BinOpKind::And => write!(f, "and"),
            BinOpKind::Or => write!(f, "or"),
            BinOpKind::Unless => write!(f, "unless"),
        }
    }
}

/// The precedence level of a binary operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Precedence {
    Lowest,
    /// `or`
    LogicalOr,
    /// `and`, `unless`
    LogicalAnd,
    /// `==`, `!=`, `>`, `<`, `>=`, `<=`
    Comparison,
    /// `+`, `-`
    Additive,
    /// `*`, `/`, `%`
    Multiplicative,
    /// `^`
    Power,
    /// Unary
    Unary,
}

impl BinOpKind {
    pub fn precedence(self) -> Precedence {
        match self {
            BinOpKind::Or => Precedence::LogicalOr,
            BinOpKind::And | BinOpKind::Unless => Precedence::LogicalAnd,
            BinOpKind::Eq | BinOpKind::NotEq | BinOpKind::Gt | BinOpKind::Lt
            | BinOpKind::Gte | BinOpKind::Lte => Precedence::Comparison,
            BinOpKind::Add | BinOpKind::Sub => Precedence::Additive,
            BinOpKind::Mul | BinOpKind::Div | BinOpKind::Mod => Precedence::Multiplicative,
            BinOpKind::Pow => Precedence::Power,
        }
    }
}

/// Vector matching clause for binary operators.
#[derive(Debug, Clone, PartialEq)]
pub struct VectorMatching {
    /// The matching card: `on` (use these labels) or `ignoring` (ignore these).
    pub card: VectorMatchCard,
    /// The labels in the matching clause.
    pub labels: Vec<String>,
    /// `group_left` labels (None if not specified).
    pub group_left: Option<Vec<String>>,
    /// `group_right` labels (None if not specified).
    pub group_right: Option<Vec<String>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VectorMatchCard {
    On,
    Ignoring,
}

// ── Unary operators ───────────────────────────────────────────────────────────

/// A unary operation: `-expr` or `+expr`
#[derive(Debug, Clone, PartialEq)]
pub struct UnaryOp {
    pub op: UnaryOpKind,
    pub expr: Box<Expr>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOpKind {
    Plus,
    Minus,
}

// ── Duration ──────────────────────────────────────────────────────────────────

/// A Prometheus duration: `5m`, `30s`, `1h30m`, `7d`, `1w`
#[derive(Debug, Clone, PartialEq)]
pub struct Duration {
    /// Duration in milliseconds.
    pub millis: i64,
}

impl Duration {
    pub fn new(millis: i64) -> Self {
        Self { millis }
    }

    /// Return the duration in seconds (as f64).
    pub fn seconds(&self) -> f64 {
        self.millis as f64 / 1000.0
    }

    /// Return the duration in milliseconds.
    pub fn millis_i64(&self) -> i64 {
        self.millis
    }
}

impl fmt::Display for Duration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut remaining = self.millis;
        if remaining == 0 {
            return write!(f, "0s");
        }
        let weeks = remaining / (7 * 24 * 3600 * 1000);
        remaining %= 7 * 24 * 3600 * 1000;
        let days = remaining / (24 * 3600 * 1000);
        remaining %= 24 * 3600 * 1000;
        let hours = remaining / (3600 * 1000);
        remaining %= 3600 * 1000;
        let minutes = remaining / (60 * 1000);
        remaining %= 60 * 1000;
        let seconds = remaining / 1000;
        let ms = remaining % 1000;

        let mut s = String::new();
        if weeks > 0 { s.push_str(&format!("{}w", weeks)); }
        if days > 0 { s.push_str(&format!("{}d", days)); }
        if hours > 0 { s.push_str(&format!("{}h", hours)); }
        if minutes > 0 { s.push_str(&format!("{}m", minutes)); }
        if seconds > 0 { s.push_str(&format!("{}s", seconds)); }
        if ms > 0 { s.push_str(&format!("{}ms", ms)); }
        write!(f, "{}", s)
    }
}

// ── @ modifier ────────────────────────────────────────────────────────────────

/// The `@` timestamp modifier on instant vectors.
#[derive(Debug, Clone, PartialEq)]
pub enum AtModifier {
    /// `@ 1234567890` — unix timestamp in seconds.
    UnixTimestamp(f64),
    /// `@ start()` or `@ end()` — special functions.
    Start,
    End,
}

// ── Helper display impls for debugging ────────────────────────────────────────

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Expr::NumberLiteral(n) => write!(f, "{}", n),
            Expr::StringLiteral(s) => write!(f, "\"{}\"", s),
            Expr::InstantVector(iv) => write!(f, "{}", iv),
            Expr::RangeVector(rv) => write!(f, "{}", rv),
            Expr::FunctionCall(fc) => write!(f, "{}", fc),
            Expr::Aggregation(agg) => write!(f, "{}", agg),
            Expr::BinaryOp(bin) => write!(f, "({} {} {})", bin.lhs, bin.op, bin.rhs),
            Expr::UnaryOp(un) => {
                let op_str = match un.op { UnaryOpKind::Minus => "-", UnaryOpKind::Plus => "+" };
                write!(f, "{}{}", op_str, un.expr)
            }
            Expr::Subquery(sq) => write!(f, "{}", sq),
            Expr::Paren(e) => write!(f, "({})", e),
        }
    }
}

impl fmt::Display for InstantVector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.metric_name)?;
        if !self.matchers.is_empty() {
            write!(f, "{{")?;
            for (i, m) in self.matchers.iter().enumerate() {
                if i > 0 { write!(f, ", ")?; }
                write!(f, "{}", m)?;
            }
            write!(f, "}}")?;
        }
        if let Some(ref dur) = self.offset {
            write!(f, " offset {}", dur)?;
        }
        if let Some(ref at) = self.at {
            match at {
                AtModifier::UnixTimestamp(ts) => write!(f, " @ {}", ts)?,
                AtModifier::Start => write!(f, " @ start()")?,
                AtModifier::End => write!(f, " @ end()")?,
            }
        }
        Ok(())
    }
}

impl fmt::Display for RangeVector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}[{}]", self.vector, self.range)
    }
}

impl fmt::Display for LabelMatcher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}\"{}\"", self.label, self.op, self.value)
    }
}

impl fmt::Display for FunctionCall {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}(", self.name)?;
        for (i, arg) in self.args.iter().enumerate() {
            if i > 0 { write!(f, ", ")?; }
            write!(f, "{}", arg)?;
        }
        write!(f, ")")
    }
}

impl fmt::Display for Aggregation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.op)?;
        if let Some(ref param) = self.param {
            write!(f, "({}, ", param)?;
        } else {
            write!(f, "(")?;
        }
        // grouping
        if self.grouping.by {
            write!(f, "by (")?;
        } else {
            write!(f, "without (")?;
        }
        for (i, label) in self.grouping.labels.iter().enumerate() {
            if i > 0 { write!(f, ", ")?; }
            write!(f, "{}", label)?;
        }
        write!(f, ") ")?;
        write!(f, "({}))", self.expr)
    }
}

impl fmt::Display for Subquery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}[{}", self.expr, self.range)?;
        if let Some(ref step) = self.step {
            write!(f, ":{}", step)?;
        }
        write!(f, "]")?;
        if let Some(ref offset) = self.offset {
            write!(f, " offset {}", offset)?;
        }
        Ok(())
    }
}

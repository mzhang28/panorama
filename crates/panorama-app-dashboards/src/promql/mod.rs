//! PromQL (Prometheus Query Language) implementation for the Dashboards plugin.
//!
//! This module provides a PromQL parser and translator that converts PromQL
//! expressions into Panorama Query Language (PQL) calls plus DataFrame
//! post-processing steps.
//!
//! ## Architecture
//!
//! ```text
//! PromQL string
//!   → parser::parse() → ast::Expr
//!     → translator::translate() → TranslatedQuery { pql, post_steps }
//!       → ctx.query(pql_string) → Vec<Value> rows
//!         → apply post_steps → DataFrame
//! ```
//!
//! ## Supported PromQL features
//!
//! - Instant and range vector selectors with label matchers (=, !=, =~, !~)
//! - All arithmetic, comparison, and logical/set binary operators
//! - Aggregation operators: sum, avg, min, max, count, stddev, stdvar, topk, bottomk, quantile
//! - Functions: rate, irate, increase, delta, deriv, predict_linear, changes, resets,
//!   histogram_quantile, absent, sort, sort_desc, label_replace, label_join
//! - Subqueries, offset modifier, @ timestamp modifier
//! - Vector matching: on(), ignoring(), group_left(), group_right()

pub mod ast;
pub mod parser;
pub mod registry;
pub mod translator;

pub use ast::Expr;
pub use parser::parse;
pub use registry::MetricRegistry;
pub use translator::{translate, PostStep, TranslatedQuery};

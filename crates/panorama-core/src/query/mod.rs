//! Panorama Query Language v0 — surface syntax, AST, IR, and parser.
//!
//! See `QUERY_DESIGN.md` for the full language specification.

pub mod ast;
pub mod ir;
pub mod parser;

pub use ast::Query;
pub use parser::{parse_query, ParseError};

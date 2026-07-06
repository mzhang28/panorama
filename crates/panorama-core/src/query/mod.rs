//! Panorama Query Language v0 — surface syntax, AST, IR, and parser.
//!
//! See `QUERY_DESIGN.md` for the full language specification.

use std::collections::HashMap;

pub mod ast;
pub mod eval;
pub mod ir;
pub mod parser;

pub use ast::Query;
pub use eval::eval_query;
pub use parser::{parse_query, ParseError};

/// Parse a single JSON row from `RETURN n` back into a `Node`.
/// Columns expected: `id`, `fields_json`; others defaulted.
pub fn row_to_node(row: &serde_json::Value) -> Option<crate::types::Node> {
  let obj = row.as_object()?;
  let id = uuid::Uuid::parse_str(obj.get("id")?.as_str()?).ok()?;
  let fields_val = obj.get("fields_json").or_else(|| obj.get("fields"))?;
  let fields: HashMap<String, crate::types::FieldValue> = match fields_val {
    serde_json::Value::String(s) => serde_json::from_str(s).unwrap_or_default(),
    serde_json::Value::Object(_) => serde_json::from_value(fields_val.clone()).unwrap_or_default(),
    _ => HashMap::new(),
  };
  Some(crate::types::Node {
    id,
    fields,
    space_id: uuid::Uuid::nil(),
    preferred_schemas: vec![],
    app_managed: None,
    created_at: chrono::Utc::now(),
    updated_at: chrono::Utc::now(),
  })
}

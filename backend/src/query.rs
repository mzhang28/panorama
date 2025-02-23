lalrpop_util::lalrpop_mod!(query_grammar);

use std::ops::Range;

use anyhow::{Result, anyhow, bail};

pub struct Query(Vec<QueryExpr>);

pub struct QueryExpr {
  kind: QueryExprKind,
  span: Range<usize>,
}

pub enum QueryExprKind {
  Atom(String),
  Exact(String),
  FieldConstraint(String, Op),
  And(Query, Query),
  Or(Query, Query),
  Not(Query),
}

pub enum Op {
  Eq,
  Neq,
  Like,
  Gt,
  Lt,
  Gte,
  Lte,
}

pub fn parse_query(input: impl AsRef<str>) -> Result<Query> {
  let parser = query_grammar::QueryParser::new();
  let input = input.as_ref();
  let result = match parser.parse(input) {
    Ok(v) => v,
    Err(e) => bail!("failed to parse"),
  };

  todo!()
}

#[cfg(test)]
mod tests {
  use super::parse_query;
}

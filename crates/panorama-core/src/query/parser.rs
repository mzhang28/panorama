//! Nom-based parser for the Panorama Query Language v0 surface syntax.
//!
//! Uses `nom` parser combinators for all parsing. The grammar mirrors
//! `QUERY_DESIGN.md` §3.
//!
//! ## Grammar
//!
//! ```text
//! query         = match_clause+ return_clause (order_by)? (limit)? (skip)?
//! match_clause  = "MATCH" "(" var ")" (ref_traverse | "IN" space "(" string ")") where_clause?
//! ref_traverse  = "-[" ":REF" "(" string ")" ("*" int (".." int)?)? "]->"
//!                  "(" var ")" "IN" space "(" string ")"
//! where_clause  = "WHERE" predicate
//! predicate     = or_expr
//! or_expr       = and_expr ("OR" and_expr)*
//! and_expr      = atomic ("AND" atomic)*
//! atomic        = "NOT" atomic
//!               | "HAS_FIELD" "(" var "," string "," string ")"
//!               | "SCAN" "(" field_expr ")"
//!               | conforms_to
//!               | "(" predicate ")"
//!               | field_expr
//! conforms_to   = var "CONFORMS" "TO" "schema" "(" string ("," range)? ")"
//! range         = int (".." int)?
//! field_expr    = field_path ("IS" ("NOT")? "NULL"
//!                  | "IN" "[" value ("," value)* "]"
//!                  | "LIKE" string
//!                  | cmp_op value)
//! field_path    = var "." (ident ".")? ident crdt_view?
//! crdt_view     = "@" ("merged" | "ops" | "at" "(" string ")")
//! cmp_op        = "=" | "!=" | "<>" | "<" | "<=" | ">" | ">="
//! value         = string | int | float | "true" | "false" | "null"
//! return_clause = "RETURN" return_col ("," return_col)*
//! return_col    = field_path ("AS" ident)? | var ("AS" ident)?
//! order_by      = "ORDER" "BY" field_path ("ASC" | "DESC")?
//! limit         = "LIMIT" int
//! skip          = "SKIP" int
//! ```

use nom::{
  branch::alt,
  bytes::complete::{tag, tag_no_case, take_while1},
  character::complete::multispace0,
  combinator::{cut, map, opt, peek, recognize},
  multi::{many0, separated_list0},
  sequence::{delimited, preceded},
  IResult,
};

use super::ast::*;
use std::str::FromStr;

// ── Parse error ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ParseError {
  pub message: String,
  pub pos: usize,
}

impl std::fmt::Display for ParseError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{} at position {}", self.message, self.pos)
  }
}

impl std::error::Error for ParseError {}

// ── Public entry point ──────────────────────────────────────────────────────────

pub fn parse_query(src: &str) -> Result<Query, ParseError> {
  match query(src) {
    Ok((remaining, q)) => {
      let rem = remaining.trim();
      if rem.is_empty() {
        Ok(q)
      } else {
        Err(ParseError {
          message: format!("unexpected trailing input: '{}'", &rem[..rem.len().min(40)]),
          pos: offset(src, rem),
        })
      }
    }
    Err(nom::Err::Error(e)) | Err(nom::Err::Failure(e)) => Err(ParseError {
      message: format!("parse error: {:?}", e.code),
      pos: offset(src, e.input),
    }),
    Err(_) => Err(ParseError {
      message: "incomplete input".into(),
      pos: src.len(),
    }),
  }
}

fn offset(src: &str, remaining: &str) -> usize {
  src.len().saturating_sub(remaining.len())
}

// ── Whitespace & keyword helpers ───────────────────────────────────────────────

/// Skip optional whitespace (including newlines).
fn ws(s: &str) -> IResult<&str, ()> {
  map(multispace0, drop)(s)
}

/// A case-insensitive keyword, consuming trailing whitespace and checking
/// that it's not part of a longer identifier.
fn keyword<'a>(kw: &'static str) -> impl FnMut(&'a str) -> IResult<&'a str, ()> {
  move |s: &str| {
    let (s, _) = multispace0(s)?;
    let (s, _) = tag_no_case(kw)(s)?;
    // Word boundary: next char must not be alphanumeric or underscore
    if let Some(c) = s.chars().next() {
      if c.is_alphanumeric() || c == '_' {
        return Err(nom::Err::Error(nom::error::Error::new(
          s,
          nom::error::ErrorKind::Tag,
        )));
      }
    }
    let (s, _) = multispace0(s)?;
    Ok((s, ()))
  }
}

/// Comma with optional surrounding whitespace.
fn comma(s: &str) -> IResult<&str, ()> {
  let (s, _) = ws(s)?;
  let (s, _) = tag(",")(s)?;
  let (s, _) = ws(s)?;
  Ok((s, ()))
}

// ── Identifiers and literals ────────────────────────────────────────────────────

/// A bare identifier: starts with alpha or `_`, continues with alphanumeric or `_`.
fn ident(s: &str) -> IResult<&str, String> {
  let (s, _) = ws(s)?;
  let (s, r) = recognize(take_while1(|c: char| c.is_alphanumeric() || c == '_'))(s)?;
  // Must start with alpha or underscore
  let first = r.chars().next().unwrap();
  if !first.is_alphabetic() && first != '_' {
    return Err(nom::Err::Error(nom::error::Error::new(
      s,
      nom::error::ErrorKind::Alpha,
    )));
  }
  let (s, _) = ws(s)?;
  Ok((s, r.to_string()))
}

/// A variable name (same as ident for v0).
fn var_name(s: &str) -> IResult<&str, String> {
  ident(s)
}

/// A double-quoted string literal. Supports `\"` and `\\` escapes.
fn string_lit(s: &str) -> IResult<&str, String> {
  let (s, _) = ws(s)?;
  let (s, _) = tag("\"")(s)?;
  let mut result = String::new();
  let mut chars = s.char_indices();
  let mut end = None;
  while let Some((i, c)) = chars.next() {
    if c == '\\' {
      if let Some((_, next)) = chars.next() {
        match next {
          '"' => result.push('"'),
          '\\' => result.push('\\'),
          'n' => result.push('\n'),
          't' => result.push('\t'),
          'r' => result.push('\r'),
          other => {
            result.push('\\');
            result.push(other);
          }
        }
      } else {
        result.push('\\');
        break;
      }
    } else if c == '"' {
      end = Some(i + 1);
      break;
    } else {
      result.push(c);
    }
  }
  match end {
    Some(pos) => Ok((&s[pos..], result)),
    None => Err(nom::Err::Error(nom::error::Error::new(
      s,
      nom::error::ErrorKind::TakeWhile1,
    ))),
  }
}

/// An integer literal, optionally negative.
fn integer_lit(s: &str) -> IResult<&str, i64> {
  let (s, _) = ws(s)?;
  let (s, neg) = opt(tag("-"))(s)?;
  let (s, digits) = take_while1(|c: char| c.is_ascii_digit())(s)?;
  let num_str = if neg.is_some() {
    format!("-{}", digits)
  } else {
    digits.to_string()
  };
  match i64::from_str(&num_str) {
    Ok(n) => Ok((s, n)),
    Err(_) => Err(nom::Err::Error(nom::error::Error::new(
      s,
      nom::error::ErrorKind::Digit,
    ))),
  }
}

/// A boolean literal.
fn boolean_lit(s: &str) -> IResult<&str, bool> {
  let (s, _) = ws(s)?;
  alt((
    map(keyword("true"), |_| true),
    map(keyword("false"), |_| false),
  ))(s)
}

/// A null literal.
fn null_lit(s: &str) -> IResult<&str, Value> {
  let (s, _) = keyword("null")(s)?;
  Ok((s, Value::Null))
}

// ── Values ──────────────────────────────────────────────────────────────────────

fn value(s: &str) -> IResult<&str, Value> {
  alt((
    map(string_lit, Value::String),
    map(double_lit, Value::Float),
    map(integer_lit, Value::Integer),
    map(boolean_lit, Value::Boolean),
    null_lit,
  ))(s)
}

/// Parse a float literal: digits "." digits
fn double_lit(s: &str) -> IResult<&str, f64> {
  let (s, _) = ws(s)?;
  let (s, neg) = opt(tag("-"))(s)?;
  let (s, int_part) = take_while1(|c: char| c.is_ascii_digit())(s)?;
  let (s, _) = tag(".")(s)?;
  let (s, frac_part) = take_while1(|c: char| c.is_ascii_digit())(s)?;
  let num_str = if neg.is_some() {
    format!("-{}.{}", int_part, frac_part)
  } else {
    format!("{}.{}", int_part, frac_part)
  };
  match f64::from_str(&num_str) {
    Ok(n) => Ok((s, n)),
    Err(_) => Err(nom::Err::Error(nom::error::Error::new(
      s,
      nom::error::ErrorKind::Float,
    ))),
  }
}

// ── Field paths ─────────────────────────────────────────────────────────────────

/// `var "." (ns_part ".")? ident ("@" crdt_view)?`
/// `ns_part` = `ident` | `string_lit` (for quoted namespaces like `"com.example.app"`)
fn field_path(s: &str) -> IResult<&str, FieldPath> {
  let (s, variable) = var_name(s)?;
  let (s, _) = tag(".")(s)?;
  let (s, first) = ns_part(s)?;
  let (s, (namespace, field)) = if let Ok((s2, _)) = peek::<_, _, (), _>(tag("."))(s) {
    let (s, _) = tag(".")(s2)?;
    let (s, f) = ident(s)?;
    (s, (Some(first), f))
  } else {
    (s, (None, first))
  };
  let (s, view) = opt(crdt_view)(s)?;
  Ok((
    s,
    FieldPath {
      variable,
      namespace,
      field,
      view,
    },
  ))
}

/// A namespace or field-name part: either a bare ident or a quoted string.
fn ns_part(s: &str) -> IResult<&str, String> {
  alt((string_lit, ident))(s)
}

/// `@merged` | `@ops` | `@at("cursor")`
fn crdt_view(s: &str) -> IResult<&str, CrdtView> {
  let (s, _) = tag("@")(s)?;
  alt((
    map(keyword("merged"), |_| CrdtView::Merged),
    map(keyword("ops"), |_| CrdtView::Ops),
    map(
      preceded(keyword("at"), delimited(tag("("), string_lit, tag(")"))),
      CrdtView::At,
    ),
  ))(s)
}

// ── Comparison operators ────────────────────────────────────────────────────────

fn cmp_op(s: &str) -> IResult<&str, CmpOp> {
  let (s, _) = ws(s)?;
  alt((
    nom::combinator::value(CmpOp::Neq, alt((tag("!="), tag("<>")))),
    nom::combinator::value(CmpOp::Lte, tag("<=")),
    nom::combinator::value(CmpOp::Gte, tag(">=")),
    nom::combinator::value(CmpOp::Eq, tag("=")),
    nom::combinator::value(CmpOp::Lt, tag("<")),
    nom::combinator::value(CmpOp::Gt, tag(">")),
  ))(s)
}

// ── Predicates ──────────────────────────────────────────────────────────────────

/// Top-level predicate: OR binds loosest.
fn predicate(s: &str) -> IResult<&str, Predicate> {
  or_expr(s)
}

/// `and_expr ("OR" and_expr)*`
fn or_expr(s: &str) -> IResult<&str, Predicate> {
  let (s, first) = and_expr(s)?;
  let (s, rest) = many0(preceded(keyword("OR"), and_expr))(s)?;
  Ok((
    s,
    rest
      .into_iter()
      .fold(first, |acc, p| Predicate::Or(Box::new(acc), Box::new(p))),
  ))
}

/// `atomic ("AND" atomic)*`
fn and_expr(s: &str) -> IResult<&str, Predicate> {
  let (s, first) = atomic(s)?;
  let (s, rest) = many0(preceded(keyword("AND"), atomic))(s)?;
  Ok((
    s,
    rest
      .into_iter()
      .fold(first, |acc, p| Predicate::And(Box::new(acc), Box::new(p))),
  ))
}

/// `"NOT" atomic | "(" predicate ")" | conforms_to | has_field | scan | field_expr`
fn atomic(s: &str) -> IResult<&str, Predicate> {
  let (s, _) = ws(s)?;
  alt((
    map(preceded(keyword("NOT"), cut(atomic)), |p| {
      Predicate::Not(Box::new(p))
    }),
    delimited(tag("("), predicate, cut(tag(")"))),
    conforms_to,
    has_field,
    scan,
    field_expr,
  ))(s)
}

/// `var "CONFORMS" "TO" "schema" "(" string ("," range)? ")"`
fn conforms_to(s: &str) -> IResult<&str, Predicate> {
  let (s, field) = var_name(s)?;
  let (s, _) = keyword("CONFORMS")(s)?;
  let (s, _) = keyword("TO")(s)?;
  let (s, _) = keyword("schema")(s)?;
  let (s, _) = tag("(")(s)?;
  let (s, schema_id) = string_lit(s)?;
  let (s, ver) = opt(preceded(comma, version_range))(s)?;
  let (version_min, version_max) = ver.unwrap_or((None, None));
  let (s, _) = cut(tag(")"))(s)?;
  Ok((
    s,
    Predicate::ConformsTo {
      field,
      schema_id,
      version_min,
      version_max,
    },
  ))
}

/// `int (".." int)?`  (after the comma in CONFORMS TO)
fn version_range(s: &str) -> IResult<&str, (Option<u32>, Option<u32>)> {
  let (s, min) = integer_lit(s)?;
  let (s, max) = opt(preceded(tag(".."), integer_lit))(s)?;
  Ok((s, (Some(min as u32), max.map(|m| m as u32))))
}

/// `"HAS_FIELD" "(" var "," string "," string ")"`
fn has_field(s: &str) -> IResult<&str, Predicate> {
  let (s, _) = keyword("HAS_FIELD")(s)?;
  let (s, _) = tag("(")(s)?;
  let (s, variable) = var_name(s)?;
  let (s, _) = comma(s)?;
  let (s, namespace) = string_lit(s)?;
  let (s, _) = comma(s)?;
  let (s, field_name) = string_lit(s)?;
  let (s, _) = cut(tag(")"))(s)?;
  Ok((
    s,
    Predicate::HasField {
      variable,
      namespace,
      field_name,
    },
  ))
}

/// `"SCAN" "(" field_expr ")"` — marks predicate as requiring full scan.
fn scan(s: &str) -> IResult<&str, Predicate> {
  let (s, _) = keyword("SCAN")(s)?;
  let (s, _) = tag("(")(s)?;
  let (s, pred) = field_expr(s)?;
  let (s, _) = cut(tag(")"))(s)?;
  Ok((s, Predicate::Scan(Box::new(pred))))
}

/// `field_path ("IS" ("NOT")? "NULL" | "IN" "[" ... "]" | "LIKE" string | cmp_op value)`
fn field_expr(s: &str) -> IResult<&str, Predicate> {
  let (s, fp) = field_path(s)?;
  let (s, _) = ws(s)?;

  alt((
    is_null_expr(fp.clone()),
    in_set_expr(fp.clone()),
    like_expr(fp.clone()),
    comparison_expr(fp),
  ))(s)
}

fn is_null_expr(fp: FieldPath) -> impl FnMut(&str) -> IResult<&str, Predicate> {
  move |s: &str| {
    let (s, _) = keyword("IS")(s)?;
    let (s, not) = opt(keyword("NOT"))(s)?;
    let (s, _) = keyword("NULL")(s)?;
    Ok((
      s,
      Predicate::IsNull {
        field_path: fp.clone(),
        not: not.is_some(),
      },
    ))
  }
}

fn in_set_expr(fp: FieldPath) -> impl FnMut(&str) -> IResult<&str, Predicate> {
  move |s: &str| {
    let (s, _) = keyword("IN")(s)?;
    let (s, _) = tag("[")(s)?;
    let (s, values) = separated_list0(comma, value)(s)?;
    let (s, _) = cut(tag("]"))(s)?;
    Ok((
      s,
      Predicate::In {
        field_path: fp.clone(),
        values,
        not: false,
      },
    ))
  }
}

fn like_expr(fp: FieldPath) -> impl FnMut(&str) -> IResult<&str, Predicate> {
  move |s: &str| {
    let (s, _) = keyword("LIKE")(s)?;
    let (s, pattern) = string_lit(s)?;
    Ok((
      s,
      Predicate::Like {
        field_path: fp.clone(),
        pattern,
        not: false,
      },
    ))
  }
}

fn comparison_expr(fp: FieldPath) -> impl FnMut(&str) -> IResult<&str, Predicate> {
  move |s: &str| {
    let (s, op) = cmp_op(s)?;
    let (s, val) = value(s)?;
    Ok((
      s,
      Predicate::FieldCompare {
        field_path: fp.clone(),
        op,
        value: val,
      },
    ))
  }
}

// ── RETURN ──────────────────────────────────────────────────────────────────────

/// `"RETURN" return_col ("," return_col)*`
fn return_clause(s: &str) -> IResult<&str, ReturnClause> {
  let (s, _) = keyword("RETURN")(s)?;
  let (s, cols) = separated_list0(comma, return_col)(s)?;
  if cols.is_empty() {
    return Err(nom::Err::Error(nom::error::Error::new(
      s,
      nom::error::ErrorKind::Many0,
    )));
  }
  Ok((s, ReturnClause { columns: cols }))
}

/// `field_path ("AS" ident)?`  |  `var ("AS" ident)?` (whole node)
///
/// Try field_path first (which always has a "."), then fall back to bare var.
fn return_col(s: &str) -> IResult<&str, ReturnColumn> {
  let (s, _) = ws(s)?;

  // Try field_path — it always contains at least one "."
  // We can detect: if we see `var "." ...` it's a field path.
  // Let's parse var, then check for dot.
  let (s, var) = var_name(s)?;

  let (s, expression) = if let Ok((s2, _)) = peek::<_, _, (), _>(tag("."))(s) {
    // It's a field path — we've consumed `var`, now parse from the dot onward
    let (s, _) = tag(".")(s2)?;
    let (s, first) = ns_part(s)?;
    let (s, (namespace, field)) = if let Ok((s3, _)) = peek::<_, _, (), _>(tag("."))(s) {
      let (s, _) = tag(".")(s3)?;
      let (s, f) = ident(s)?;
      (s, (Some(first), f))
    } else {
      (s, (None, first))
    };
    let (s, view) = opt(crdt_view)(s)?;
    (
      s,
      ReturnExpr::Field(FieldPath {
        variable: var,
        namespace,
        field,
        view,
      }),
    )
  } else {
    // Whole node: bare variable
    (s, ReturnExpr::Node(var))
  };

  let (s, alias) = opt(preceded(keyword("AS"), ident))(s)?;
  Ok((s, ReturnColumn { expression, alias }))
}

// ── MATCH ───────────────────────────────────────────────────────────────────────

/// `"MATCH" "(" var ")" (ref_traverse | "IN" space "(" string ")") where_clause?`
fn match_clause(s: &str) -> IResult<&str, MatchClause> {
  let (s, _) = keyword("MATCH")(s)?;
  let (s, _) = tag("(")(s)?;
  let (s, variable) = var_name(s)?;
  let (s, _) = tag(")")(s)?;

  // Check for ref traverse: starts with `-[`
  let (s, source) = if let Ok((_, _)) = peek::<_, _, (), _>(tag("-["))(s) {
    ref_traverse(s)?
  } else {
    // Simple: IN space("name")
    let (s, _) = keyword("IN")(s)?;
    let (s, _) = keyword("space")(s)?;
    let (s, _) = tag("(")(s)?;
    let (s, name) = string_lit(s)?;
    let (s, _) = cut(tag(")"))(s)?;
    (s, MatchSource::Space(name))
  };

  let (s, where_clause) = opt(where_clause)(s)?;

  Ok((
    s,
    MatchClause {
      variable,
      source,
      where_clause,
    },
  ))
}

/// `"-[" ":REF" "(" string ")" ("*" int (".." int)?)? "]->" "(" var ")" "IN" space "(" string ")"`
fn ref_traverse(s: &str) -> IResult<&str, MatchSource> {
  let (s, _) = tag("-[")(s)?;
  let (s, _) = tag(":")(s)?;
  let (s, _) = keyword("REF")(s)?;
  let (s, _) = tag("(")(s)?;
  let (s, edge_type) = string_lit(s)?;
  let (s, _) = tag(")")(s)?;

  // Optional depth: `*1` or `*1..3`
  let (s, (min_depth, max_depth)) = if let Ok((_, _)) = peek::<_, _, (), _>(tag("*"))(s) {
    let (s, _) = tag("*")(s)?;
    let (s, min) = integer_lit(s)?;
    let (s, max) = opt(preceded(tag(".."), integer_lit))(s)?;
    (s, (min as u32, max.map(|m| m as u32).unwrap_or(min as u32)))
  } else {
    (s, (1, 1))
  };

  let (s, _) = tag("]")(s)?;
  let (s, _) = tag("->")(s)?;
  let (s, _) = tag("(")(s)?;
  let (s, target_var) = var_name(s)?;
  let (s, _) = tag(")")(s)?;
  let (s, _) = keyword("IN")(s)?;
  let (s, _) = keyword("space")(s)?;
  let (s, _) = tag("(")(s)?;
  let (s, space_name) = string_lit(s)?;
  let (s, _) = tag(")")(s)?;

  Ok((
    s,
    MatchSource::RefTraverse {
      edge_type,
      min_depth,
      max_depth,
      target_var,
      target_source: Box::new(MatchSource::Space(space_name)),
    },
  ))
}

/// `"WHERE" predicate`
fn where_clause(s: &str) -> IResult<&str, WhereClause> {
  let (s, _) = keyword("WHERE")(s)?;
  let (s, p) = predicate(s)?;
  Ok((s, WhereClause { predicate: p }))
}

// ── ORDER BY, LIMIT, SKIP ───────────────────────────────────────────────────────

/// `"ORDER" "BY" field_path ("ASC" | "DESC")?`
fn order_by(s: &str) -> IResult<&str, OrderBy> {
  let (s, _) = keyword("ORDER")(s)?;
  let (s, _) = keyword("BY")(s)?;
  let (s, field) = field_path(s)?;
  // Optional direction: ASC | DESC | (nothing, default ASC)
  let (s, direction) = alt((
    map(keyword("ASC"), |_| OrderDir::Asc),
    map(keyword("DESC"), |_| OrderDir::Desc),
    nom::combinator::value(OrderDir::Asc, nom::combinator::success(())),
  ))(s)?;
  Ok((s, OrderBy { field, direction }))
}

/// `"LIMIT" int`
fn limit_clause(s: &str) -> IResult<&str, u64> {
  let (s, _) = keyword("LIMIT")(s)?;
  let (s, n) = integer_lit(s)?;
  Ok((s, n as u64))
}

/// `"SKIP" int`
fn skip_clause(s: &str) -> IResult<&str, u64> {
  let (s, _) = keyword("SKIP")(s)?;
  let (s, n) = integer_lit(s)?;
  Ok((s, n as u64))
}

// ── Top-level query ─────────────────────────────────────────────────────────────

/// `match_clause+ return_clause order_by? limit? skip?`
fn query(s: &str) -> IResult<&str, Query> {
  let (s, _) = ws(s)?;
  let (s, matches) = many0(match_clause)(s)?;
  if matches.is_empty() {
    return Err(nom::Err::Error(nom::error::Error::new(
      s,
      nom::error::ErrorKind::Tag,
    )));
  }
  let (s, return_clause) = return_clause(s)?;
  let (s, order_by) = opt(order_by)(s)?;
  let (s, limit) = opt(limit_clause)(s)?;
  let (s, skip) = opt(skip_clause)(s)?;
  let (s, _) = ws(s)?;
  Ok((
    s,
    Query {
      matches,
      return_clause,
      order_by,
      limit,
      skip,
    },
  ))
}

// ── Tests ───────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
  use super::*;

  // ── Basic parsing ──────────────────────────────────────────────────────────

  #[test]
  fn test_simple_match() {
    let q = parse_query(r#"MATCH (n) IN space("personal") RETURN n"#).unwrap();
    assert_eq!(q.matches.len(), 1);
    assert_eq!(q.matches[0].variable, "n");
    if let MatchSource::Space(ref s) = q.matches[0].source {
      assert_eq!(s, "personal");
    } else {
      panic!("expected Space source");
    }
    assert_eq!(q.return_clause.columns.len(), 1);
  }

  #[test]
  fn test_conforms_to() {
    let q = parse_query(
      r#"MATCH (n) IN space("personal") WHERE n CONFORMS TO schema("com.example.event") RETURN n.title AS title"#,
    )
    .unwrap();
    assert!(q.matches[0].where_clause.is_some());
    let pred = &q.matches[0].where_clause.as_ref().unwrap().predicate;
    match pred {
      Predicate::ConformsTo { schema_id, .. } => assert_eq!(schema_id, "com.example.event"),
      _ => panic!("expected ConformsTo, got {:?}", pred),
    }
  }

  #[test]
  fn test_conforms_to_with_version() {
    let q = parse_query(
      r#"MATCH (n) IN space("personal") WHERE n CONFORMS TO schema("com.example.event", 2..4) RETURN n"#,
    )
    .unwrap();
    let pred = &q.matches[0].where_clause.as_ref().unwrap().predicate;
    match pred {
      Predicate::ConformsTo {
        version_min,
        version_max,
        ..
      } => {
        assert_eq!(*version_min, Some(2));
        assert_eq!(*version_max, Some(4));
      }
      _ => panic!("expected ConformsTo"),
    }
  }

  #[test]
  fn test_conforms_to_single_version() {
    let q = parse_query(
      r#"MATCH (n) IN space("personal") WHERE n CONFORMS TO schema("com.example.event", 2) RETURN n"#,
    )
    .unwrap();
    let pred = &q.matches[0].where_clause.as_ref().unwrap().predicate;
    match pred {
      Predicate::ConformsTo { version_min, .. } => {
        assert_eq!(*version_min, Some(2));
      }
      _ => panic!("expected ConformsTo"),
    }
  }

  #[test]
  fn test_field_compare() {
    let q =
      parse_query(r#"MATCH (n) IN space("personal") WHERE n.start_time > "2026-01-01" RETURN n"#)
        .unwrap();
    let pred = &q.matches[0].where_clause.as_ref().unwrap().predicate;
    match pred {
      Predicate::FieldCompare { field_path, op, .. } => {
        assert_eq!(field_path.field, "start_time");
        assert_eq!(*op, CmpOp::Gt);
      }
      _ => panic!("expected FieldCompare, got {:?}", pred),
    }
  }

  #[test]
  fn test_field_compare_with_namespace() {
    let q = parse_query(
      r#"MATCH (n) IN space("personal") WHERE n."com.example.app".start_time > "2026-01-01" RETURN n"#,
    )
    .unwrap();
    let pred = &q.matches[0].where_clause.as_ref().unwrap().predicate;
    match pred {
      Predicate::FieldCompare { field_path, .. } => {
        assert_eq!(field_path.field, "start_time");
        assert_eq!(field_path.namespace.as_deref(), Some("com.example.app"));
      }
      _ => panic!("expected FieldCompare, got {:?}", pred),
    }
  }

  #[test]
  fn test_limit_skip() {
    let q = parse_query(r#"MATCH (n) IN space("personal") RETURN n LIMIT 50 SKIP 100"#).unwrap();
    assert_eq!(q.limit, Some(50));
    assert_eq!(q.skip, Some(100));
  }

  #[test]
  fn test_order_by() {
    let q =
      parse_query(r#"MATCH (n) IN space("personal") RETURN n ORDER BY n.start_time DESC"#).unwrap();
    assert!(q.order_by.is_some());
    let ob = q.order_by.as_ref().unwrap();
    assert_eq!(ob.field.field, "start_time");
    assert!(matches!(ob.direction, OrderDir::Desc));
  }

  #[test]
  fn test_order_by_default_asc() {
    let q =
      parse_query(r#"MATCH (n) IN space("personal") RETURN n ORDER BY n.start_time"#).unwrap();
    assert_eq!(q.order_by.as_ref().unwrap().direction, OrderDir::Asc);
  }

  #[test]
  fn test_has_field() {
    let q = parse_query(
      r#"MATCH (n) IN space("personal") WHERE HAS_FIELD(n, "app", "attendees") RETURN n"#,
    )
    .unwrap();
    let pred = &q.matches[0].where_clause.as_ref().unwrap().predicate;
    match pred {
      Predicate::HasField {
        namespace,
        field_name,
        ..
      } => {
        assert_eq!(namespace, "app");
        assert_eq!(field_name, "attendees");
      }
      _ => panic!("expected HasField, got {:?}", pred),
    }
  }

  #[test]
  fn test_has_field_wildcard() {
    let q =
      parse_query(r#"MATCH (n) IN space("personal") WHERE HAS_FIELD(n, "*", "title") RETURN n"#)
        .unwrap();
    let pred = &q.matches[0].where_clause.as_ref().unwrap().predicate;
    match pred {
      Predicate::HasField { namespace, .. } => {
        assert_eq!(namespace, "*");
      }
      _ => panic!("expected HasField"),
    }
  }

  #[test]
  fn test_is_null() {
    let q = parse_query(r#"MATCH (n) IN space("personal") WHERE n.foo IS NULL RETURN n"#).unwrap();
    let pred = &q.matches[0].where_clause.as_ref().unwrap().predicate;
    assert!(matches!(pred, Predicate::IsNull { not: false, .. }));
  }

  #[test]
  fn test_is_not_null() {
    let q =
      parse_query(r#"MATCH (n) IN space("personal") WHERE n.foo IS NOT NULL RETURN n"#).unwrap();
    let pred = &q.matches[0].where_clause.as_ref().unwrap().predicate;
    assert!(matches!(pred, Predicate::IsNull { not: true, .. }));
  }

  #[test]
  fn test_in_list() {
    let q =
      parse_query(r#"MATCH (n) IN space("personal") WHERE n.color IN ["red", "blue"] RETURN n"#)
        .unwrap();
    let pred = &q.matches[0].where_clause.as_ref().unwrap().predicate;
    match pred {
      Predicate::In { values, .. } => assert_eq!(values.len(), 2),
      _ => panic!("expected In, got {:?}", pred),
    }
  }

  #[test]
  fn test_like() {
    let q = parse_query(r#"MATCH (n) IN space("personal") WHERE n.title LIKE "%world%" RETURN n"#)
      .unwrap();
    let pred = &q.matches[0].where_clause.as_ref().unwrap().predicate;
    assert!(matches!(pred, Predicate::Like { .. }));
  }

  #[test]
  fn test_scan() {
    let q = parse_query(r#"MATCH (n) IN space("personal") WHERE SCAN(n.notes = "hello") RETURN n"#)
      .unwrap();
    let pred = &q.matches[0].where_clause.as_ref().unwrap().predicate;
    // SCAN wraps the inner predicate
    match pred {
      Predicate::Scan(inner) => {
        assert!(matches!(inner.as_ref(), Predicate::FieldCompare { .. }));
      }
      _ => panic!("expected Scan wrapper, got {:?}", pred),
    }
  }

  #[test]
  fn test_and_or() {
    let q = parse_query(
      r#"MATCH (n) IN space("personal") WHERE n.a = 1 AND n.b = 2 OR n.c = 3 RETURN n"#,
    )
    .unwrap();
    let pred = &q.matches[0].where_clause.as_ref().unwrap().predicate;
    // OR binds looser: (a=1 AND b=2) OR c=3
    assert!(matches!(pred, Predicate::Or(..)));
  }

  #[test]
  fn test_not() {
    let q = parse_query(r#"MATCH (n) IN space("personal") WHERE NOT n.foo = 5 RETURN n"#).unwrap();
    let pred = &q.matches[0].where_clause.as_ref().unwrap().predicate;
    assert!(matches!(pred, Predicate::Not(..)));
  }

  #[test]
  fn test_return_multiple_columns() {
    let q = parse_query(
      r#"MATCH (n) IN space("personal") RETURN n.title AS title, n.start_time AS start"#,
    )
    .unwrap();
    assert_eq!(q.return_clause.columns.len(), 2);
    assert_eq!(q.return_clause.columns[0].alias.as_deref(), Some("title"));
    assert_eq!(q.return_clause.columns[1].alias.as_deref(), Some("start"));
  }

  #[test]
  fn test_return_whole_node() {
    let q = parse_query(r#"MATCH (n) IN space("personal") RETURN n"#).unwrap();
    match &q.return_clause.columns[0].expression {
      ReturnExpr::Node(v) => assert_eq!(v, "n"),
      _ => panic!("expected whole-node return"),
    }
  }

  // ── Ref traverse ───────────────────────────────────────────────────────────

  #[test]
  fn test_ref_traverse() {
    let q = parse_query(
      r#"MATCH (a)-[:REF("attendee")]->(b) IN space("personal") RETURN a.title, b.name LIMIT 100"#,
    )
    .unwrap();
    match &q.matches[0].source {
      MatchSource::RefTraverse {
        edge_type,
        target_var,
        min_depth,
        max_depth,
        ..
      } => {
        assert_eq!(edge_type, "attendee");
        assert_eq!(target_var, "b");
        assert_eq!(*min_depth, 1);
        assert_eq!(*max_depth, 1);
      }
      _ => panic!("expected RefTraverse, got {:?}", q.matches[0].source),
    }
  }

  #[test]
  fn test_ref_traverse_multi_hop() {
    let q =
      parse_query(r#"MATCH (a)-[:REF("parent")*1..3]->(b) IN space("work") RETURN a, b LIMIT 100"#)
        .unwrap();
    match &q.matches[0].source {
      MatchSource::RefTraverse {
        min_depth,
        max_depth,
        ..
      } => {
        assert_eq!(*min_depth, 1);
        assert_eq!(*max_depth, 3);
      }
      _ => panic!("expected RefTraverse"),
    }
  }

  // ── CRDT view selectors ────────────────────────────────────────────────────

  #[test]
  fn test_crdt_view_merged() {
    let q = parse_query(r#"MATCH (n) IN space("personal") RETURN n.event.title@merged AS title"#)
      .unwrap();
    let col = &q.return_clause.columns[0];
    match &col.expression {
      ReturnExpr::Field(fp) => {
        assert_eq!(fp.field, "title");
        assert_eq!(fp.namespace.as_deref(), Some("event"));
        assert_eq!(fp.view, Some(CrdtView::Merged));
      }
      _ => panic!("expected Field"),
    }
  }

  #[test]
  fn test_crdt_view_ops() {
    let q = parse_query(r#"MATCH (n) IN space("personal") RETURN n.event.attendees@ops"#).unwrap();
    let col = &q.return_clause.columns[0];
    match &col.expression {
      ReturnExpr::Field(fp) => assert_eq!(fp.view, Some(CrdtView::Ops)),
      _ => panic!("expected Field"),
    }
  }

  #[test]
  fn test_crdt_view_at() {
    let q = parse_query(
      r#"MATCH (n) IN space("personal") RETURN n.event.count@at("cursor_abc") AS count"#,
    )
    .unwrap();
    let col = &q.return_clause.columns[0];
    match &col.expression {
      ReturnExpr::Field(fp) => assert_eq!(fp.view, Some(CrdtView::At("cursor_abc".into()))),
      _ => panic!("expected Field"),
    }
  }

  #[test]
  fn test_crdt_view_in_where() {
    let q =
      parse_query(r#"MATCH (n) IN space("personal") WHERE n.event.count@merged > 5 RETURN n"#)
        .unwrap();
    let pred = &q.matches[0].where_clause.as_ref().unwrap().predicate;
    match pred {
      Predicate::FieldCompare { field_path, .. } => {
        assert_eq!(field_path.view, Some(CrdtView::Merged));
      }
      _ => panic!("expected FieldCompare"),
    }
  }

  // ── Error cases ────────────────────────────────────────────────────────────

  #[test]
  fn test_trailing_input_is_error() {
    let r = parse_query(r#"MATCH (n) IN space("personal") RETURN n garbage"#);
    assert!(r.is_err());
  }

  #[test]
  fn test_missing_return_is_error() {
    let r = parse_query(r#"MATCH (n) IN space("personal")"#);
    assert!(r.is_err());
  }
}

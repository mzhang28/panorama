//! PromQL Parser using `nom` parser combinators.

use nom::{
  branch::alt,
  bytes::complete::{tag, tag_no_case, take_while1},
  character::complete::{char as char_c, multispace0},
  combinator::{all_consuming, map, opt, value},
  multi::{many0, separated_list0},
  number::complete::double,
  sequence::{delimited, pair, preceded, tuple},
  IResult,
};

use crate::promql::ast::*;

// ── Error type ────────────────────────────────────────────────────────────────

/// Error type returned when parsing fails.
#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
  pub message: String,
  pub pos: usize,
}

impl std::fmt::Display for ParseError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "at position {}: {}", self.pos, self.message)
  }
}

impl std::error::Error for ParseError {}

impl From<ParseError> for panorama_core::PluginError {
  fn from(e: ParseError) -> Self {
    panorama_core::PluginError::bad_request(&format!("PromQL parse error: {}", e))
  }
}

// ── Public entry point ────────────────────────────────────────────────────────

/// Parse a PromQL expression string into an AST.
pub fn parse(src: &str) -> Result<Expr, ParseError> {
  let trimmed = src.trim();
  if trimmed.is_empty() {
    return Err(ParseError {
      message: "empty query".into(),
      pos: 0,
    });
  }

  match all_consuming(|i| parse_expr_depth(i, 0))(trimmed) {
    Ok((_, expr)) => Ok(expr),
    Err(nom::Err::Error(e)) | Err(nom::Err::Failure(e)) => {
      let offset = trimmed.len() - e.input.len();
      Err(ParseError {
        message: format!("parse error near '{}'", e.input),
        pos: offset,
      })
    }
    Err(nom::Err::Incomplete(_)) => Err(ParseError {
      message: "unexpected end of input".into(),
      pos: trimmed.len(),
    }),
  }
}

// ── Basic combinator helpers ──────────────────────────────────────────────────

fn ws<'a, F, O>(inner: F) -> impl FnMut(&'a str) -> IResult<&'a str, O>
where
  F: FnMut(&'a str) -> IResult<&'a str, O>,
{
  delimited(multispace0, inner, multispace0)
}

fn keyword<'a>(kw: &'static str) -> impl FnMut(&'a str) -> IResult<&'a str, &'a str> {
  move |input: &'a str| {
    let (input_after_ws, _) = multispace0(input)?;
    let (next_input, res) = tag_no_case(kw)(input_after_ws)?;
    if let Some(first_char) = next_input.chars().next() {
      if first_char.is_alphanumeric() || first_char == '_' {
        return Err(nom::Err::Error(nom::error::Error::new(
          input,
          nom::error::ErrorKind::Tag,
        )));
      }
    }
    let (final_input, _) = multispace0(next_input)?;
    Ok((final_input, res))
  }
}

fn identifier(input: &str) -> IResult<&str, String> {
  ws(map(
    take_while1(|c: char| c.is_alphanumeric() || c == '_' || c == ':'),
    |s: &str| s.to_string(),
  ))(input)
}

fn parse_string<'a>(input: &'a str) -> IResult<&'a str, String> {
  let parse_quoted = |quote: char| {
    move |input: &'a str| -> IResult<&'a str, String> {
      let (input, _) = char_c(quote)(input)?;
      let mut result = String::new();
      let mut chars = input.char_indices().peekable();

      while let Some((idx, c)) = chars.next() {
        if c == quote {
          let end_bytes = idx + c.len_utf8();
          let remaining = &input[end_bytes..];
          return Ok((remaining, result));
        }
        if c == '\\' {
          if let Some((_, escaped)) = chars.next() {
            match escaped {
              'n' => result.push('\n'),
              't' => result.push('\t'),
              'r' => result.push('\r'),
              '\\' => result.push('\\'),
              '"' => result.push('"'),
              '\'' => result.push('\''),
              other => {
                result.push('\\');
                result.push(other);
              }
            }
          } else {
            return Err(nom::Err::Error(nom::error::Error::new(
              input,
              nom::error::ErrorKind::Escaped,
            )));
          }
        } else {
          result.push(c);
        }
      }
      Err(nom::Err::Error(nom::error::Error::new(
        input,
        nom::error::ErrorKind::Tag,
      )))
    }
  };

  ws(alt((parse_quoted('"'), parse_quoted('\''))))(input)
}

fn parse_number(input: &str) -> IResult<&str, f64> {
  ws(double)(input)
}

// ── Duration parsing ──────────────────────────────────────────────────────────

fn parse_single_duration(input: &str) -> IResult<&str, i64> {
  let (input, val_str) = take_while1(|c: char| c.is_ascii_digit() || c == '.')(input)?;
  let val: f64 = val_str
    .parse()
    .map_err(|_| nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Float)))?;

  if val.is_nan() || val.is_infinite() || val < 0.0 {
    return Err(nom::Err::Error(nom::error::Error::new(
      input,
      nom::error::ErrorKind::Float,
    )));
  }

  let (input, unit) = alt((tag("ms"), tag("s"), tag("m"), tag("h"), tag("d"), tag("w")))(input)?;

  let unit_ms: i64 = match unit {
    "ms" => 1,
    "s" => 1000,
    "m" => 60 * 1000,
    "h" => 3600 * 1000,
    "d" => 24 * 3600 * 1000,
    "w" => 7 * 24 * 3600 * 1000,
    _ => unreachable!(),
  };

  let ms = val * unit_ms as f64;
  if ms > i64::MAX as f64 {
    return Err(nom::Err::Error(nom::error::Error::new(
      input,
      nom::error::ErrorKind::TooLarge,
    )));
  }

  Ok((input, ms as i64))
}

fn parse_duration(input: &str) -> IResult<&str, Duration> {
  let (input, dur_list) = ws(many0(parse_single_duration))(input)?;
  if dur_list.is_empty() {
    return Err(nom::Err::Error(nom::error::Error::new(
      input,
      nom::error::ErrorKind::Many0,
    )));
  }

  let mut total: i64 = 0;
  for ms in dur_list {
    total = total.checked_add(ms).ok_or_else(|| {
      nom::Err::Error(nom::error::Error::new(
        input,
        nom::error::ErrorKind::TooLarge,
      ))
    })?;
  }

  Ok((input, Duration::new(total)))
}

fn parse_at_modifier(input: &str) -> IResult<&str, AtModifier> {
  preceded(
    ws(char_c('@')),
    alt((
      value(AtModifier::Start, keyword("start()")),
      value(AtModifier::End, keyword("end()")),
      map(parse_number, AtModifier::UnixTimestamp),
    )),
  )(input)
}

// ── Label matchers ────────────────────────────────────────────────────────────

fn parse_match_op(input: &str) -> IResult<&str, MatchOp> {
  ws(alt((
    value(MatchOp::NotRegex, tag("!~")),
    value(MatchOp::Regex, tag("=~")),
    value(MatchOp::NotEq, tag("!=")),
    value(MatchOp::Eq, tag("=")),
  )))(input)
}

fn parse_label_matcher(input: &str) -> IResult<&str, LabelMatcher> {
  let (input, (label, op, value)) = tuple((identifier, parse_match_op, parse_string))(input)?;
  Ok((input, LabelMatcher { label, op, value }))
}

fn parse_label_matchers(input: &str) -> IResult<&str, Vec<LabelMatcher>> {
  delimited(
    ws(char_c('{')),
    separated_list0(ws(char_c(',')), parse_label_matcher),
    ws(char_c('}')),
  )(input)
}

// ── Vector Selectors ──────────────────────────────────────────────────────────

fn parse_instant_vector(input: &str) -> IResult<&str, InstantVector> {
  let (input, (metric_name, matchers)) = alt((
    pair(identifier, opt(parse_label_matchers)),
    map(parse_label_matchers, |m| (String::new(), Some(m))),
  ))(input)?;

  let matchers = matchers.unwrap_or_default();
  if metric_name.is_empty() && matchers.is_empty() {
    return Err(nom::Err::Error(nom::error::Error::new(
      input,
      nom::error::ErrorKind::Tag,
    )));
  }

  let (input, offset) = opt(preceded(keyword("offset"), parse_duration))(input)?;
  let (input, at) = opt(parse_at_modifier)(input)?;

  Ok((
    input,
    InstantVector {
      metric_name,
      matchers,
      offset,
      at,
    },
  ))
}

// ── Vector Matching & Grouping ────────────────────────────────────────────────

fn parse_label_list(input: &str) -> IResult<&str, Vec<String>> {
  delimited(
    ws(char_c('(')),
    separated_list0(ws(char_c(',')), identifier),
    ws(char_c(')')),
  )(input)
}

fn parse_vector_matching(input: &str) -> IResult<&str, VectorMatching> {
  let (input, card) = alt((
    value(VectorMatchCard::On, keyword("on")),
    value(VectorMatchCard::Ignoring, keyword("ignoring")),
  ))(input)?;

  let (input, labels) = parse_label_list(input)?;

  let (input, group_side) = opt(alt((
    map(
      preceded(keyword("group_left"), opt(parse_label_list)),
      |l| (Some(l.unwrap_or_default()), None),
    ),
    map(
      preceded(keyword("group_right"), opt(parse_label_list)),
      |r| (None, Some(r.unwrap_or_default())),
    ),
  )))(input)?;

  let (group_left, group_right) = group_side.unwrap_or((None, None));

  Ok((
    input,
    VectorMatching {
      card,
      labels,
      group_left,
      group_right,
    },
  ))
}

fn parse_grouping(input: &str) -> IResult<&str, Grouping> {
  alt((
    map(preceded(keyword("by"), parse_label_list), |labels| {
      Grouping { by: true, labels }
    }),
    map(preceded(keyword("without"), parse_label_list), |labels| {
      Grouping { by: false, labels }
    }),
  ))(input)
}

// ── Aggregations & Functions ──────────────────────────────────────────────────

fn parse_aggregation_op(input: &str) -> IResult<&str, AggregationOp> {
  ws(alt((
    value(AggregationOp::Sum, keyword("sum")),
    value(AggregationOp::Avg, keyword("avg")),
    value(AggregationOp::Min, keyword("min")),
    value(AggregationOp::Max, keyword("max")),
    value(AggregationOp::Count, keyword("count")),
    value(AggregationOp::Stddev, keyword("stddev")),
    value(AggregationOp::Stdvar, keyword("stdvar")),
    value(AggregationOp::TopK, keyword("topk")),
    value(AggregationOp::BottomK, keyword("bottomk")),
    value(AggregationOp::CountValues, keyword("count_values")),
    value(AggregationOp::Quantile, keyword("quantile")),
    value(AggregationOp::Group, keyword("group")),
  )))(input)
}

fn parse_aggregation_expr(input: &str, depth: usize) -> IResult<&str, Expr> {
  let (input, op) = parse_aggregation_op(input)?;
  let (input, grouping_first) = opt(parse_grouping)(input)?;

  // Check if parens have param + expr or just expr
  let (input, (param, expr)) = delimited(
    ws(char_c('(')),
    alt((
      // Param + Expr, e.g. topk(5, rate(...))
      map(
        tuple((
          |i| parse_expr_depth(i, depth + 1),
          ws(char_c(',')),
          |i| parse_expr_depth(i, depth + 1),
        )),
        |(p, _, e)| (Some(Box::new(p)), e),
      ),
      // Single Expr, e.g. sum(rate(...))
      map(|i| parse_expr_depth(i, depth + 1), |e| (None, e)),
    )),
    ws(char_c(')')),
  )(input)?;

  let (input, grouping_after) = opt(parse_grouping)(input)?;
  let grouping = grouping_first.or(grouping_after).unwrap_or(Grouping {
    by: true,
    labels: vec![],
  });

  Ok((
    input,
    Expr::Aggregation(Aggregation {
      op,
      expr: Box::new(expr),
      grouping,
      param,
    }),
  ))
}

fn parse_function_call(input: &str, depth: usize) -> IResult<&str, FunctionCall> {
  let (input, name) = identifier(input)?;
  let (input, args) = delimited(
    ws(char_c('(')),
    separated_list0(ws(char_c(',')), |i| parse_expr_depth(i, depth + 1)),
    ws(char_c(')')),
  )(input)?;

  Ok((
    input,
    FunctionCall {
      name: name.to_lowercase(),
      args,
    },
  ))
}

// ── Primary Expression Parsing ────────────────────────────────────────────────

fn parse_primary(input: &str, depth: usize) -> IResult<&str, Expr> {
  if depth > 100 {
    return Err(nom::Err::Error(nom::error::Error::new(
      input,
      nom::error::ErrorKind::TooLarge,
    )));
  }

  // 1. Number literal
  if let Ok((rem, n)) = parse_number(input) {
    if nom::character::complete::alpha1::<&str, nom::error::Error<&str>>(rem).is_err()
      || rem.starts_with('e')
      || rem.starts_with('E')
    {
      return Ok((rem, Expr::NumberLiteral(n)));
    }
  }

  // 2. String literal
  if let Ok((rem, s)) = parse_string(input) {
    return Ok((rem, Expr::StringLiteral(s)));
  }

  // 3. Parenthesized expression
  if let Ok((rem, e)) = delimited(
    ws(char_c('(')),
    |i| parse_expr_depth(i, depth + 1),
    ws(char_c(')')),
  )(input)
  {
    return Ok((rem, Expr::Paren(Box::new(e))));
  }

  // 4. Aggregation expression
  if let Ok((rem, agg)) = parse_aggregation_expr(input, depth) {
    return Ok((rem, agg));
  }

  // 5. Function call
  if let Ok((rem, fc)) = parse_function_call(input, depth) {
    return Ok((rem, Expr::FunctionCall(fc)));
  }

  // 6. Instant Vector Selector
  let (input, iv) = parse_instant_vector(input)?;
  let expr = Expr::InstantVector(iv);

  Ok((input, expr))
}

// ── Postfix (RangeVector & Subquery) Parsing ──────────────────────────────────

fn parse_postfix(input: &str, depth: usize) -> IResult<&str, Expr> {
  let (input, mut expr) = parse_primary(input, depth)?;

  // RangeVector or Subquery bracket: [5m] or [5m:1m]
  if let Ok((input, (range, step))) = delimited(
    ws(char_c('[')),
    tuple((
      parse_duration,
      opt(preceded(ws(char_c(':')), opt(parse_duration))),
    )),
    ws(char_c(']')),
  )(input)
  {
    let (input, offset) = opt(preceded(keyword("offset"), parse_duration))(input)?;

    match expr {
      Expr::InstantVector(iv) if step.is_none() && offset.is_none() => {
        expr = Expr::RangeVector(RangeVector {
          vector: Box::new(iv),
          range,
        });
      }
      _ => {
        expr = Expr::Subquery(Subquery {
          expr: Box::new(expr),
          range,
          step: step.flatten(),
          offset,
        });
      }
    }
    return Ok((input, expr));
  }

  Ok((input, expr))
}

// ── Unary Operations ──────────────────────────────────────────────────────────

fn parse_unary(input: &str, depth: usize) -> IResult<&str, Expr> {
  if depth > 100 {
    return Err(nom::Err::Error(nom::error::Error::new(
      input,
      nom::error::ErrorKind::TooLarge,
    )));
  }

  alt((
    map(
      preceded(ws(char_c('-')), |i| parse_unary(i, depth + 1)),
      |e| {
        Expr::UnaryOp(UnaryOp {
          op: UnaryOpKind::Minus,
          expr: Box::new(e),
        })
      },
    ),
    map(
      preceded(ws(char_c('+')), |i| parse_unary(i, depth + 1)),
      |e| {
        Expr::UnaryOp(UnaryOp {
          op: UnaryOpKind::Plus,
          expr: Box::new(e),
        })
      },
    ),
    |i| parse_postfix(i, depth),
  ))(input)
}

// ── Binary Operations & Precedence ────────────────────────────────────────────

fn parse_bin_op(input: &str) -> IResult<&str, BinOpKind> {
  ws(alt((
    value(BinOpKind::Add, tag("+")),
    value(BinOpKind::Sub, tag("-")),
    value(BinOpKind::Mul, tag("*")),
    value(BinOpKind::Div, tag("/")),
    value(BinOpKind::Mod, tag("%")),
    value(BinOpKind::Pow, tag("^")),
    value(BinOpKind::Eq, tag("==")),
    value(BinOpKind::NotEq, tag("!=")),
    value(BinOpKind::Gte, tag(">=")),
    value(BinOpKind::Lte, tag("<=")),
    value(BinOpKind::Gt, tag(">")),
    value(BinOpKind::Lt, tag("<")),
    value(BinOpKind::And, keyword("and")),
    value(BinOpKind::Or, keyword("or")),
    value(BinOpKind::Unless, keyword("unless")),
  )))(input)
}

fn parse_binary_expr(input: &str, min_prec: Precedence, depth: usize) -> IResult<&str, Expr> {
  if depth > 100 {
    return Err(nom::Err::Error(nom::error::Error::new(
      input,
      nom::error::ErrorKind::TooLarge,
    )));
  }

  let (mut current_input, mut lhs) = parse_unary(input, depth)?;

  loop {
    let (next_input, op) = match parse_bin_op(current_input) {
      Ok(res) => res,
      Err(_) => break,
    };

    let prec = op.precedence();
    if prec < min_prec {
      break;
    }

    let (next_input, matching) = opt(parse_vector_matching)(next_input)?;

    let next_prec = match prec {
      Precedence::Power => Precedence::Power,
      _ => match prec {
        Precedence::LogicalOr => Precedence::LogicalAnd,
        Precedence::LogicalAnd => Precedence::Comparison,
        Precedence::Comparison => Precedence::Additive,
        Precedence::Additive => Precedence::Multiplicative,
        Precedence::Multiplicative => Precedence::Power,
        Precedence::Power => Precedence::Unary,
        _ => Precedence::Unary,
      },
    };

    let (after_rhs, rhs) = parse_binary_expr(next_input, next_prec, depth + 1)?;

    lhs = Expr::BinaryOp(BinaryOp {
      lhs: Box::new(lhs),
      rhs: Box::new(rhs),
      op,
      matching,
    });

    current_input = after_rhs;
  }

  Ok((current_input, lhs))
}

fn parse_expr_depth(input: &str, depth: usize) -> IResult<&str, Expr> {
  parse_binary_expr(input, Precedence::Lowest, depth)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
  use super::*;

  fn p(src: &str) -> Expr {
    parse(src).expect(&format!("failed to parse: {}", src))
  }

  #[test]
  fn test_instant_vector_selectors() {
    assert_eq!(
      p("http_requests_total"),
      Expr::InstantVector(InstantVector {
        metric_name: "http_requests_total".into(),
        matchers: vec![],
        offset: None,
        at: None,
      })
    );

    assert_eq!(
      p(r#"http_requests_total{job="prometheus", group="canary"}"#),
      Expr::InstantVector(InstantVector {
        metric_name: "http_requests_total".into(),
        matchers: vec![
          LabelMatcher {
            label: "job".into(),
            op: MatchOp::Eq,
            value: "prometheus".into(),
          },
          LabelMatcher {
            label: "group".into(),
            op: MatchOp::Eq,
            value: "canary".into(),
          },
        ],
        offset: None,
        at: None,
      })
    );
  }

  #[test]
  fn test_range_vector() {
    assert_eq!(
      p("http_requests_total[5m]"),
      Expr::RangeVector(RangeVector {
        vector: Box::new(InstantVector {
          metric_name: "http_requests_total".into(),
          matchers: vec![],
          offset: None,
          at: None,
        }),
        range: Duration::new(5 * 60 * 1000),
      })
    );
  }

  #[test]
  fn test_function_call() {
    assert_eq!(
      p("rate(http_requests_total[5m])"),
      Expr::FunctionCall(FunctionCall {
        name: "rate".into(),
        args: vec![Expr::RangeVector(RangeVector {
          vector: Box::new(InstantVector {
            metric_name: "http_requests_total".into(),
            matchers: vec![],
            offset: None,
            at: None,
          }),
          range: Duration::new(5 * 60 * 1000),
        })],
      })
    );
  }

  #[test]
  fn test_aggregation() {
    assert_eq!(
      p("sum by (job) (rate(http_requests_total[5m]))"),
      Expr::Aggregation(Aggregation {
        op: AggregationOp::Sum,
        expr: Box::new(Expr::FunctionCall(FunctionCall {
          name: "rate".into(),
          args: vec![Expr::RangeVector(RangeVector {
            vector: Box::new(InstantVector {
              metric_name: "http_requests_total".into(),
              matchers: vec![],
              offset: None,
              at: None,
            }),
            range: Duration::new(5 * 60 * 1000),
          })],
        })),
        grouping: Grouping {
          by: true,
          labels: vec!["job".into()],
        },
        param: None,
      })
    );
  }

  #[test]
  fn test_binary_op() {
    let expr = p("http_requests_total + 10");
    match expr {
      Expr::BinaryOp(bin) => {
        assert_eq!(bin.op, BinOpKind::Add);
        assert_eq!(*bin.rhs, Expr::NumberLiteral(10.0));
      }
      _ => panic!("expected BinaryOp"),
    }
  }

  #[test]
  fn test_duration_parsing() {
    assert_eq!(
      parse_duration("5m").unwrap().1,
      Duration::new(5 * 60 * 1000)
    );
    assert_eq!(
      parse_duration("1h30m").unwrap().1,
      Duration::new(3600 * 1000 + 30 * 60 * 1000)
    );
  }

  #[test]
  fn test_fuzz_crash_regressions() {
    assert!(parse("a and").is_err());
    assert!(parse("a or").is_err());
    assert!(parse("a unless").is_err());
    assert!(parse("a on").is_err());
    assert!(parse("a ignoring").is_err());
    assert!(parse("a group_left").is_err());
    assert!(parse("a group_right").is_err());

    assert!(parse("and🦀").is_err());
    assert!(parse("ignoring🔥").is_err());

    if let Ok(Expr::StringLiteral(s)) = parse(r#""hello\nworld""#) {
      assert_eq!(s, "hello\nworld");
    } else {
      panic!("failed string escape test");
    }

    assert!(parse("metric[1e300d]").is_err());
    assert!(parse("metric[-5m]").is_err());
    assert!(parse("metric[nand]").is_err());

    let deep = "(".repeat(150) + "a" + &")".repeat(150);
    assert!(parse(&deep).is_err());
  }
}

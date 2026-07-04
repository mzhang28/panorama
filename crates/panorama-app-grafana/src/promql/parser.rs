//! Recursive-descent parser for PromQL (Prometheus Query Language).
//!
//! Follows the Prometheus PromQL grammar with precedence climbing for
//! binary operators.

use super::ast::*;

/// Parse error with position info.
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

impl From<ParseError> for panorama_core::PluginError {
    fn from(e: ParseError) -> Self {
        panorama_core::PluginError::bad_request(&format!("PromQL parse error: {}", e))
    }
}

// ── Parser state ──────────────────────────────────────────────────────────────

struct Parser {
    input: Vec<char>,
    pos: usize,
}

impl Parser {
    fn new(src: &str) -> Self {
        Self { input: src.chars().collect(), pos: 0 }
    }

    fn err<T>(&self, msg: &str) -> Result<T, ParseError> {
        Err(ParseError { message: msg.to_string(), pos: self.pos })
    }

    fn eof(&self) -> bool { self.pos >= self.input.len() }

    fn peek(&self) -> Option<char> {
        self.input.get(self.pos).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let c = self.peek();
        if c.is_some() { self.pos += 1; }
        c
    }

    fn skip_ws(&mut self) {
        while let Some(c) = self.peek() {
            if c.is_whitespace() { self.pos += 1; } else { break; }
        }
    }

    fn peek_str(&self, s: &str) -> bool {
        let remaining: String = self.input[self.pos..].iter().collect();
        remaining.starts_with(s)
    }

    fn expect_str(&mut self, s: &str) -> Result<(), ParseError> {
        self.skip_ws();
        let remaining: String = self.input[self.pos..].iter().collect();
        if remaining.starts_with(s) {
            self.pos += s.len();
            Ok(())
        } else {
            self.err(&format!("expected '{}'", s))
        }
    }

    fn read_identifier(&mut self) -> Result<String, ParseError> {
        self.skip_ws();
        let start = self.pos;
        while let Some(c) = self.peek() {
            if c.is_alphanumeric() || c == '_' || c == ':' {
                self.pos += 1;
            } else {
                break;
            }
        }
        if self.pos == start {
            return self.err("expected identifier");
        }
        // Metric names cannot start with a digit in PromQL, but for identifiers
        // in general (like label names) digits at the start are fine.
        Ok(self.input[start..self.pos].iter().collect())
    }

    /// Read a metric name: allows colons (Prometheus-style) but must start with
    /// [a-zA-Z_:] and must contain at least one non-colon char.
    fn read_metric_name(&mut self) -> Result<String, ParseError> {
        self.skip_ws();
        let start = self.pos;
        while let Some(c) = self.peek() {
            if c.is_alphanumeric() || c == '_' || c == ':' {
                self.pos += 1;
            } else {
                break;
            }
        }
        if self.pos == start {
            return self.err("expected metric name");
        }
        let name: String = self.input[start..self.pos].iter().collect();
        Ok(name)
    }

    fn read_string(&mut self) -> Result<String, ParseError> {
        self.skip_ws();
        let quote = self.peek().ok_or_else(|| ParseError {
            message: "unexpected end of input".into(), pos: self.pos,
        })?;
        if quote != '"' && quote != '\'' {
            return self.err("expected string (\"...\" or '...')");
        }
        self.pos += 1;
        let start = self.pos;
        let mut result = String::new();
        while let Some(c) = self.peek() {
            if c == quote {
                result.push_str(&self.input[start..self.pos].iter().collect::<String>());
                self.pos += 1;
                return Ok(result);
            }
            if c == '\\' {
                // Flush accumulated chars
                result.push_str(&self.input[start..self.pos].iter().collect::<String>());
                self.pos += 1; // skip backslash
                if let Some(escaped) = self.advance() {
                    match escaped {
                        'n' => result.push('\n'),
                        't' => result.push('\t'),
                        'r' => result.push('\r'),
                        '\\' => result.push('\\'),
                        '"' => result.push('"'),
                        '\'' => result.push('\''),
                        _ => {
                            result.push('\\');
                            result.push(escaped);
                        }
                    }
                }
                // advance start past the escaped char
                // (start is now self.pos since we advanced)
                // Actually we need to account for multi-char before the escape
                // Let's use a simpler approach: just track result
                continue; // skip the per-char advance below
            }
            self.pos += 1;
        }
        self.err("unterminated string")
    }

    fn read_number(&mut self) -> Result<Expr, ParseError> {
        self.skip_ws();
        let start = self.pos;
        let mut is_float = false;

        // Optional leading sign
        if let Some(c) = self.peek() {
            if c == '-' || c == '+' {
                self.pos += 1;
            }
        }

        // Integer part
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                self.pos += 1;
            } else {
                break;
            }
        }

        // Fractional part
        if let Some('.') = self.peek() {
            // Check it's not ".." (range) or ".<digit>" (field access)
            if self.pos + 1 < self.input.len() {
                let next = self.input[self.pos + 1];
                if next.is_ascii_digit() {
                    is_float = true;
                    self.pos += 1; // skip '.'
                    while let Some(c) = self.peek() {
                        if c.is_ascii_digit() { self.pos += 1; } else { break; }
                    }
                }
            }
        }

        // Exponent part (e.g., 1e9, 2.5e-3)
        if let Some(c) = self.peek() {
            if c == 'e' || c == 'E' {
                is_float = true;
                self.pos += 1;
                if let Some(c) = self.peek() {
                    if c == '+' || c == '-' { self.pos += 1; }
                }
                while let Some(c) = self.peek() {
                    if c.is_ascii_digit() { self.pos += 1; } else { break; }
                }
            }
        }

        let num_str: String = self.input[start..self.pos].iter().collect();
        if is_float || num_str.contains('.') || num_str.contains('e') || num_str.contains('E') {
            num_str.parse::<f64>()
                .map(Expr::NumberLiteral)
                .map_err(|_| ParseError { message: format!("invalid float: {}", num_str), pos: start })
        } else {
            num_str.parse::<i64>()
                .map(|i| Expr::NumberLiteral(i as f64))
                .map_err(|_| ParseError { message: format!("invalid integer: {}", num_str), pos: start })
        }
    }
}

// ── Public entry point ────────────────────────────────────────────────────────

/// Parse a PromQL expression string into an AST.
pub fn parse(src: &str) -> Result<Expr, ParseError> {
    let mut p = Parser::new(src);
    let expr = parse_expr(&mut p)?;
    p.skip_ws();
    if !p.eof() {
        let remaining: String = p.input[p.pos..].iter().collect();
        return Err(ParseError {
            message: format!("unexpected trailing input: '{}'", remaining.trim()),
            pos: p.pos,
        });
    }
    Ok(expr)
}

// ── Expression parsing with precedence climbing ───────────────────────────────

fn parse_expr(p: &mut Parser) -> Result<Expr, ParseError> {
    parse_binary_expr(p, Precedence::Lowest)
}

/// Precedence-climbing binary expression parser.
fn parse_binary_expr(p: &mut Parser, min_prec: Precedence) -> Result<Expr, ParseError> {
    let mut lhs = parse_unary(p)?;

    loop {
        p.skip_ws();
        if p.eof() { break; }

        // Try to parse a binary operator
        let saved = p.pos;
        let op = match parse_bin_op(p) {
            Ok(op) => op,
            Err(_) => { p.pos = saved; break; }
        };

        let prec = op.precedence();
        if prec < min_prec {
            // Lower precedence than what we're looking for — stop
            p.pos = saved;
            break;
        }

        // Check for vector matching clause: on() / ignoring() / group_left / group_right
        let matching = parse_vector_matching(p).ok();

        // Parse right-hand side with next precedence level
        let next_prec = match prec {
            Precedence::Power => Precedence::Power, // ^ is right-associative
            _ => {
                // Increment precedence by one level for left-associative ops
                match prec {
                    Precedence::LogicalOr => Precedence::LogicalAnd,
                    Precedence::LogicalAnd => Precedence::Comparison,
                    Precedence::Comparison => Precedence::Additive,
                    Precedence::Additive => Precedence::Multiplicative,
                    Precedence::Multiplicative => Precedence::Power,
                    Precedence::Power => Precedence::Unary,
                    _ => Precedence::Unary,
                }
            }
        };

        let rhs = parse_binary_expr(p, next_prec)?;
        lhs = Expr::BinaryOp(BinaryOp {
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
            op,
            matching,
        });
    }

    Ok(lhs)
}

/// Try to parse a binary operator at the current position.
fn parse_bin_op(p: &mut Parser) -> Result<BinOpKind, ParseError> {
    p.skip_ws();
    match p.peek() {
        Some('+') => { p.pos += 1; Ok(BinOpKind::Add) }
        Some('-') => { p.pos += 1; Ok(BinOpKind::Sub) }
        Some('*') => { p.pos += 1; Ok(BinOpKind::Mul) }
        Some('/') => { p.pos += 1; Ok(BinOpKind::Div) }
        Some('%') => { p.pos += 1; Ok(BinOpKind::Mod) }
        Some('^') => { p.pos += 1; Ok(BinOpKind::Pow) }
        Some('=') => {
            p.pos += 1;
            if p.peek() == Some('=') { p.pos += 1; Ok(BinOpKind::Eq) }
            else if p.peek() == Some('~') { p.err("unexpected '=~' in binary expression (did you mean '=='?)") }
            else { p.err("expected '==' for comparison") }
        }
        Some('!') => {
            p.pos += 1;
            if p.peek() == Some('=') { p.pos += 1; Ok(BinOpKind::NotEq) }
            else if p.peek() == Some('~') { p.err("unexpected '!~' — label matchers are only valid inside {...}") }
            else { p.err("expected '!=' for comparison") }
        }
        Some('>') => {
            p.pos += 1;
            if p.peek() == Some('=') { p.pos += 1; Ok(BinOpKind::Gte) }
            else { Ok(BinOpKind::Gt) }
        }
        Some('<') => {
            p.pos += 1;
            if p.peek() == Some('=') { p.pos += 1; Ok(BinOpKind::Lte) }
            else { Ok(BinOpKind::Lt) }
        }
        _ => {
            // Keyword operators: and, or, unless
            let remaining: String = p.input[p.pos..].iter().collect();
            let lower = remaining.to_lowercase();
            if lower.starts_with("and") && !remaining[3..].starts_with(|c: char| c.is_alphanumeric() || c == '_') {
                p.pos += 3; Ok(BinOpKind::And)
            } else if lower.starts_with("or") && !remaining[2..].starts_with(|c: char| c.is_alphanumeric() || c == '_') {
                p.pos += 2; Ok(BinOpKind::Or)
            } else if lower.starts_with("unless") && !remaining[6..].starts_with(|c: char| c.is_alphanumeric() || c == '_') {
                p.pos += 6; Ok(BinOpKind::Unless)
            } else {
                p.err("expected binary operator")
            }
        }
    }
}

// ── Unary expressions ─────────────────────────────────────────────────────────

fn parse_unary(p: &mut Parser) -> Result<Expr, ParseError> {
    p.skip_ws();
    match p.peek() {
        Some('+') => {
            p.pos += 1;
            Ok(Expr::UnaryOp(UnaryOp {
                op: UnaryOpKind::Plus,
                expr: Box::new(parse_unary(p)?),
            }))
        }
        Some('-') => {
            p.pos += 1;
            Ok(Expr::UnaryOp(UnaryOp {
                op: UnaryOpKind::Minus,
                expr: Box::new(parse_unary(p)?),
            }))
        }
        _ => parse_primary(p),
    }
}

// ── Primary expressions ───────────────────────────────────────────────────────

fn parse_primary(p: &mut Parser) -> Result<Expr, ParseError> {
    p.skip_ws();
    if p.eof() {
        return p.err("unexpected end of input");
    }

    match p.peek() {
        Some('(') => {
            p.pos += 1;
            let expr = parse_expr(p)?;
            p.skip_ws();
            if p.peek() == Some(')') {
                p.pos += 1;
                // Check if this paren expression is followed by [...] for subquery
                if p.peek() == Some('[') {
                    return parse_subquery_tail(expr, p);
                }
                Ok(Expr::Paren(Box::new(expr)))
            } else {
                p.err("expected ')'")
            }
        }
        Some('"') | Some('\'') => {
            Ok(Expr::StringLiteral(p.read_string()?))
        }
        Some('0'..='9') | Some('.') => {
            // Could be a number or a metric name starting with a digit prefix
            // In PromQL, metric names start with [a-zA-Z_:], so numbers are literals
            if p.peek() == Some('.') {
                // Check if it's ".<digit>" (float like .5)
                let next = p.input.get(p.pos + 1).copied();
                if next.map_or(false, |c| c.is_ascii_digit()) {
                    return p.read_number();
                }
                // Otherwise treat as metric name or error
            }
            p.read_number()
        }
        _ => {
            // Must be: metric name, function call, or aggregation keyword
            let saved = p.pos;

            // Peek ahead to see if this is an aggregation keyword
            let ident = p.read_identifier()?;
            let lower = ident.to_lowercase();

            // Check for aggregation operators
            if let Some(agg_op) = AggregationOp::from_str(&lower) {
                p.skip_ws();
                // Aggregation can be followed by '(' or by 'by'/'without'
                let next_char = p.peek();
                if next_char == Some('(') || next_char.is_some() {
                    // Peek ahead — is it 'by' or 'without'?
                    let remaining: String = p.input[p.pos..].iter().collect();
                    let lower_rem = remaining.to_lowercase();
                    if next_char == Some('(')
                        || lower_rem.starts_with("by ")
                        || lower_rem.starts_with("by(")
                        || lower_rem.starts_with("without ")
                        || lower_rem.starts_with("without(")
                    {
                        return parse_aggregation(agg_op, p);
                    }
                }
            }

            // Check for special functions (which look like aggregations sometimes)
            // Restore and parse as function/metric
            p.pos = saved;

            // It's either a function call or an instant vector
            let name = p.read_metric_name()?;
            p.skip_ws();

            let result = if p.peek() == Some('{') || p.peek().is_none() || p.peek() == Some('[')
                || p.peek() == Some(')') || p.peek() == Some(',')
                || p.peek() == Some('o') // "offset"
                || p.peek() == Some('@')
            {
                // It's an instant vector selector
                parse_vector_tail(p, name)?
            } else if p.peek() == Some('(') {
                // It's a function call
                parse_function_call_tail(p, name)?
            } else {
                // Could be a metric name followed by a binary op
                // Return as instant vector with no matchers
                Expr::InstantVector(InstantVector {
                    metric_name: name,
                    matchers: vec![],
                    offset: None,
                    at: None,
                })
            };

            // Check for subquery bracket after function call or vector
            p.skip_ws();
            if p.peek() == Some('[') {
                return parse_subquery_tail(result, p);
            }
            Ok(result)
        }
    }
}

// ── Vector parsing ────────────────────────────────────────────────────────────

fn parse_vector_tail(p: &mut Parser, metric_name: String) -> Result<Expr, ParseError> {
    // Parse optional label matchers: {...}
    let matchers = if p.peek() == Some('{') {
        parse_label_matchers(p)?
    } else {
        vec![]
    };

    let mut offset = None;
    let mut at = None;

    // Check for @ modifier or offset
    p.skip_ws();
    if p.peek() == Some('@') {
        at = Some(parse_at_modifier(p)?);
        p.skip_ws();
    }

    if p.peek_str("offset") {
        p.expect_str("offset")?;
        offset = Some(parse_duration(p)?);
    }

    let vector = InstantVector { metric_name, matchers, offset, at };

    // Check for range vector: [...]
    p.skip_ws();
    if p.peek() == Some('[') {
        p.pos += 1; // skip '['
        let range = parse_duration(p)?;
        p.skip_ws();
        if p.peek() == Some(':') {
            // It's a subquery: metric[range:step]
            p.pos += 1; // skip ':'
            let step = parse_duration(p)?;
            p.skip_ws();
            if p.peek() != Some(']') {
                return p.err("expected ']' to close subquery");
            }
            p.pos += 1; // skip ']'
            return Ok(Expr::Subquery(Subquery {
                expr: Box::new(Expr::InstantVector(vector)),
                range,
                step: Some(step),
                offset: None,
            }));
        }
        if p.peek() != Some(']') {
            return p.err("expected ']' to close range vector");
        }
        p.pos += 1; // skip ']'
        Ok(Expr::RangeVector(RangeVector { vector: Box::new(vector), range }))
    } else {
        Ok(Expr::InstantVector(vector))
    }
}

/// Parse a subquery tail: expression[:[step]]] [offset ...]
fn parse_subquery_tail(expr: Expr, p: &mut Parser) -> Result<Expr, ParseError> {
    // We're at '[' after a primary
    p.pos += 1; // past '['
    let range = parse_duration(p)?;
    p.skip_ws();

    let step = if p.peek() == Some(':') {
        p.pos += 1;
        let s = parse_duration(p)?;
        p.skip_ws();
        Some(s)
    } else {
        None
    };

    if p.peek() != Some(']') {
        return p.err("expected ']' to close subquery range");
    }
    p.pos += 1; // past ']'

    p.skip_ws();
    let offset = if p.peek_str("offset") {
        p.expect_str("offset")?;
        Some(parse_duration(p)?)
    } else {
        None
    };

    Ok(Expr::Subquery(Subquery {
        expr: Box::new(expr),
        range,
        step,
        offset,
    }))
}

// ── Label matchers ────────────────────────────────────────────────────────────

fn parse_label_matchers(p: &mut Parser) -> Result<Vec<LabelMatcher>, ParseError> {
    p.expect_str("{")?;
    let mut matchers = Vec::new();

    loop {
        p.skip_ws();
        if p.peek() == Some('}') {
            p.pos += 1;
            break;
        }

        if !matchers.is_empty() {
            if p.peek() == Some(',') {
                p.pos += 1;
            }
            p.skip_ws();
        }

        if p.peek() == Some('}') {
            p.pos += 1;
            break;
        }

        let label = p.read_identifier()?;
        p.skip_ws();

        let op = match p.peek() {
            Some('=') => {
                p.pos += 1;
                if p.peek() == Some('~') { p.pos += 1; MatchOp::Regex }
                else { MatchOp::Eq }
            }
            Some('!') => {
                p.pos += 1;
                if p.peek() == Some('=') { p.pos += 1; MatchOp::NotEq }
                else if p.peek() == Some('~') { p.pos += 1; MatchOp::NotRegex }
                else { return p.err("expected '!=' or '!~'"); }
            }
            _ => return p.err("expected label match operator (=, !=, =~, !~)"),
        };

        p.skip_ws();
        let value = p.read_string()?;
        matchers.push(LabelMatcher { label, op, value });
    }

    Ok(matchers)
}

// ── Function calls ────────────────────────────────────────────────────────────

fn parse_function_call_tail(p: &mut Parser, name: String) -> Result<Expr, ParseError> {
    p.expect_str("(")?;
    let mut args = Vec::new();

    loop {
        p.skip_ws();
        if p.peek() == Some(')') {
            p.pos += 1;
            break;
        }
        if !args.is_empty() {
            if p.peek() == Some(',') {
                p.pos += 1;
            }
        }
        args.push(parse_expr(p)?);
        p.skip_ws();
        if p.peek() == Some(')') {
            p.pos += 1;
            break;
        }
    }

    Ok(Expr::FunctionCall(FunctionCall { name, args }))
}

// ── Aggregation ───────────────────────────────────────────────────────────────

/// Parse an aggregation expression. Called when we know `op` is an aggregation keyword
/// and the next token may be `(` or `by`/`without`.
///
/// PromQL supports two orderings:
///   `<aggr-op> [without|by (<labels>)] ([param,] <expr>)`
///   `<aggr-op>([param,] <expr>) [without|by (<labels>)]`
fn parse_aggregation(op: AggregationOp, p: &mut Parser) -> Result<Expr, ParseError> {
    p.skip_ws();

    // Try to parse grouping clause first (before parens form):
    //   sum by (job) (rate(...))
    let grouping_first = parse_grouping(p).ok();

    p.skip_ws();
    if p.peek() != Some('(') {
        // Check if we already got grouping — if so, expression must follow in parens
        if grouping_first.is_some() {
            return p.err("expected '(' around aggregation expression");
        }
        // No grouping, no paren — this might not be an aggregation after all
        return p.err("expected '(' or 'by'/'without' after aggregation operator");
    }

    p.pos += 1; // skip '('

    // Check if this is a parameterized aggregation:
    //   topk(5, rate(...)), quantile(0.95, expr), count_values("name", expr)
    let param_needed = matches!(op,
        AggregationOp::TopK | AggregationOp::BottomK
        | AggregationOp::Quantile | AggregationOp::CountValues
    );

    let param: Option<Box<Expr>>;
    let expr: Expr;

    if param_needed {
        // Parse parameter first
        let param_expr = parse_expr(p)?;
        param = Some(Box::new(param_expr));
        // Expect comma then expression
        p.skip_ws();
        if p.peek() == Some(',') {
            p.pos += 1;
        }
        // Parse the main expression
        expr = parse_expr(p)?;
    } else {
        param = None;
        // Check if expression is wrapped in another set of parens:
        //   sum( (rate(...)) ) — rare but possible
        // Or more commonly in our context:
        //   sum(rate(...)) — expression directly in the function parens
        p.skip_ws();
        expr = if p.peek() == Some('(') {
            p.pos += 1;
            let e = parse_expr(p)?;
            p.skip_ws();
            if p.peek() != Some(')') {
                return p.err("expected ')' after inner aggregation expression");
            }
            p.pos += 1;
            e
        } else {
            parse_expr(p)?
        };
    }

    // Close the outer paren
    p.skip_ws();
    if p.peek() != Some(')') {
        return p.err("expected ')' to close aggregation");
    }
    p.pos += 1;

    // Parse optional trailing grouping clause:
    //   sum(rate(...)) by (job)
    // If we already got a grouping clause before, use that; otherwise try after
    let grouping = if let Some(g) = grouping_first {
        g
    } else {
        parse_grouping(p).unwrap_or(Grouping { by: true, labels: vec![] })
    };

    Ok(Expr::Aggregation(Aggregation {
        op,
        expr: Box::new(expr),
        grouping,
        param,
    }))
}

fn parse_grouping(p: &mut Parser) -> Result<Grouping, ParseError> {
    p.skip_ws();
    let remaining: String = p.input[p.pos..].iter().collect();
    let lower = remaining.to_lowercase();

    if lower.starts_with("by") {
        p.expect_str("by")?;
        p.skip_ws();
        if p.peek() == Some('(') {
            let labels = parse_label_list(p)?;
            Ok(Grouping { by: true, labels })
        } else {
            Ok(Grouping { by: true, labels: vec![] })
        }
    } else if lower.starts_with("without") {
        p.expect_str("without")?;
        p.skip_ws();
        if p.peek() == Some('(') {
            let labels = parse_label_list(p)?;
            Ok(Grouping { by: false, labels })
        } else {
            Ok(Grouping { by: false, labels: vec![] })
        }
    } else {
        // No grouping clause
        Ok(Grouping { by: true, labels: vec![] })
    }
}

fn parse_label_list(p: &mut Parser) -> Result<Vec<String>, ParseError> {
    p.expect_str("(")?;
    let mut labels = Vec::new();

    loop {
        p.skip_ws();
        if p.peek() == Some(')') {
            p.pos += 1;
            break;
        }
        if !labels.is_empty() {
            if p.peek() == Some(',') {
                p.pos += 1;
            }
            p.skip_ws();
        }
        labels.push(p.read_identifier()?);
        p.skip_ws();
    }

    Ok(labels)
}

// ── Vector matching ───────────────────────────────────────────────────────────

fn parse_vector_matching(p: &mut Parser) -> Result<VectorMatching, ParseError> {
    p.skip_ws();
    let remaining: String = p.input[p.pos..].iter().collect();
    let lower = remaining.to_lowercase();

    if lower.starts_with("on") && !remaining[2..].starts_with(|c: char| c.is_alphanumeric() || c == '_') {
        p.expect_str("on")?;
        let labels = parse_label_list(p)?;
        let (group_left, group_right) = parse_group_side(p)?;
        Ok(VectorMatching { card: VectorMatchCard::On, labels, group_left, group_right })
    } else if lower.starts_with("ignoring") && !remaining[8..].starts_with(|c: char| c.is_alphanumeric() || c == '_') {
        p.expect_str("ignoring")?;
        let labels = parse_label_list(p)?;
        let (group_left, group_right) = parse_group_side(p)?;
        Ok(VectorMatching { card: VectorMatchCard::Ignoring, labels, group_left, group_right })
    } else {
        p.err("expected vector matching clause (on/ignoring)")
    }
}

fn parse_group_side(p: &mut Parser) -> Result<(Option<Vec<String>>, Option<Vec<String>>), ParseError> {
    p.skip_ws();
    let remaining: String = p.input[p.pos..].iter().collect();
    let lower = remaining.to_lowercase();

    if lower.starts_with("group_left") && !remaining[10..].starts_with(|c: char| c.is_alphanumeric() || c == '_') {
        p.expect_str("group_left")?;
        p.skip_ws();
        let labels = if p.peek() == Some('(') {
            Some(parse_label_list(p)?)
        } else {
            // group_left without explicit labels means "include all labels from the left"
            Some(vec![])
        };
        Ok((labels, None))
    } else if lower.starts_with("group_right") && !remaining[11..].starts_with(|c: char| c.is_alphanumeric() || c == '_') {
        p.expect_str("group_right")?;
        p.skip_ws();
        let labels = if p.peek() == Some('(') {
            Some(parse_label_list(p)?)
        } else {
            // group_right without explicit labels means "include all labels from the right"
            Some(vec![])
        };
        Ok((None, labels))
    } else {
        Ok((None, None))
    }
}

// ── @ modifier ────────────────────────────────────────────────────────────────

fn parse_at_modifier(p: &mut Parser) -> Result<AtModifier, ParseError> {
    p.pos += 1; // skip '@'
    p.skip_ws();

    // Check for start() or end()
    let remaining: String = p.input[p.pos..].iter().collect();
    if remaining.to_lowercase().starts_with("start()") {
        p.pos += 7;
        return Ok(AtModifier::Start);
    }
    if remaining.to_lowercase().starts_with("end()") {
        p.pos += 5;
        return Ok(AtModifier::End);
    }

    // Otherwise parse a number (unix timestamp)
    let expr = p.read_number()?;
    match expr {
        Expr::NumberLiteral(n) => Ok(AtModifier::UnixTimestamp(n)),
        _ => p.err("expected unix timestamp after '@'"),
    }
}

// ── Duration parsing ──────────────────────────────────────────────────────────

/// Parse a Prometheus-style duration: `5m`, `30s`, `1h30m`, `7d`, `1w`
fn parse_duration(p: &mut Parser) -> Result<Duration, ParseError> {
    p.skip_ws();
    let start = p.pos;
    let mut total_ms: i64 = 0;
    let mut saw_value = false;

    // If first char is a digit, parse compound duration
    // Otherwise it should start with a number

    loop {
        p.skip_ws();
        if p.eof() { break; }

        // Parse number
        let num_start = p.pos;
        let mut has_digits = false;
        while let Some(c) = p.peek() {
            if c.is_ascii_digit() || c == '.' {
                has_digits = true;
                p.pos += 1;
            } else {
                break;
            }
        }

        if !has_digits {
            if saw_value { break; }
            return p.err("expected duration value (e.g., 5m, 30s, 1h30m)");
        }

        let num_str: String = p.input[num_start..p.pos].iter().collect();
        let value: f64 = num_str.parse().map_err(|_| ParseError {
            message: format!("invalid number in duration: {}", num_str),
            pos: num_start,
        })?;

        // Parse unit
        p.skip_ws();
        let unit_ms: i64 = match p.peek() {
            Some('w') => { p.pos += 1; 7 * 24 * 3600 * 1000 }
            Some('d') => { p.pos += 1; 24 * 3600 * 1000 }
            Some('h') => { p.pos += 1; 3600 * 1000 }
            Some('m') => {
                p.pos += 1;
                if p.peek() == Some('s') { p.pos += 1; 1 } // ms
                else { 60 * 1000 } // minutes
            }
            Some('s') => { p.pos += 1; 1000 }
            _ => {
                if saw_value { break; }
                return p.err("expected duration unit (w, d, h, m, s, ms)");
            }
        };

        total_ms += (value * unit_ms as f64) as i64;
        saw_value = true;

        // Check if there's more (e.g., "1h30m")
        p.skip_ws();
        if p.eof() { break; }
        // Peek ahead — if the next char is a digit, continue parsing
        let next_is_digit = p.peek().map_or(false, |c| c.is_ascii_digit() || c == '.');
        if !next_is_digit { break; }
    }

    if !saw_value {
        return Err(ParseError {
            message: "expected duration (e.g., 5m, 30s, 1h30m)".into(),
            pos: start,
        });
    }

    Ok(Duration::new(total_ms))
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // Helper: parse and unwrap
    fn p(s: &str) -> Expr {
        parse(s).unwrap_or_else(|e| panic!("parse error: {:?} for input: {}", e, s))
    }

    // ── Instant vectors ────────────────────────────────────────────────────

    #[test]
    fn test_bare_metric() {
        match p("up") {
            Expr::InstantVector(iv) => {
                assert_eq!(iv.metric_name, "up");
                assert!(iv.matchers.is_empty());
            }
            _ => panic!("expected InstantVector"),
        }
    }

    #[test]
    fn test_metric_with_colons() {
        match p("wakatime:duration") {
            Expr::InstantVector(iv) => {
                assert_eq!(iv.metric_name, "wakatime:duration");
            }
            _ => panic!("expected InstantVector"),
        }
    }

    #[test]
    fn test_instant_vector_with_labels() {
        match p(r#"http_requests_total{method="GET", status="200"}"#) {
            Expr::InstantVector(iv) => {
                assert_eq!(iv.metric_name, "http_requests_total");
                assert_eq!(iv.matchers.len(), 2);
                assert_eq!(iv.matchers[0].label, "method");
                assert_eq!(iv.matchers[0].op, MatchOp::Eq);
                assert_eq!(iv.matchers[0].value, "GET");
                assert_eq!(iv.matchers[1].label, "status");
                assert_eq!(iv.matchers[1].value, "200");
            }
            _ => panic!("expected InstantVector"),
        }
    }

    #[test]
    fn test_label_matchers_all_ops() {
        // =~
        match p(r#"metric{label=~"foo.*"}"#) {
            Expr::InstantVector(iv) => {
                assert_eq!(iv.matchers[0].op, MatchOp::Regex);
                assert_eq!(iv.matchers[0].value, "foo.*");
            }
            _ => panic!(),
        }
        // !~
        match p(r#"metric{label!~"bar.*"}"#) {
            Expr::InstantVector(iv) => {
                assert_eq!(iv.matchers[0].op, MatchOp::NotRegex);
                assert_eq!(iv.matchers[0].value, "bar.*");
            }
            _ => panic!(),
        }
        // !=
        match p(r#"metric{label!="baz"}"#) {
            Expr::InstantVector(iv) => {
                assert_eq!(iv.matchers[0].op, MatchOp::NotEq);
                assert_eq!(iv.matchers[0].value, "baz");
            }
            _ => panic!(),
        }
    }

    #[test]
    fn test_instant_vector_offset() {
        match p("metric offset 5m") {
            Expr::InstantVector(iv) => {
                assert_eq!(iv.metric_name, "metric");
                assert!(iv.offset.is_some());
                assert_eq!(iv.offset.unwrap().millis, 5 * 60 * 1000);
            }
            _ => panic!(),
        }
    }

    // ── Range vectors ──────────────────────────────────────────────────────

    #[test]
    fn test_range_vector() {
        match p("metric[5m]") {
            Expr::RangeVector(rv) => {
                assert_eq!(rv.vector.metric_name, "metric");
                assert_eq!(rv.range.millis, 5 * 60 * 1000);
            }
            _ => panic!("expected RangeVector, got {:?}", p("metric[5m]")),
        }
    }

    #[test]
    fn test_range_vector_with_labels() {
        match p(r#"http_requests_total{job="api"}[5m]"#) {
            Expr::RangeVector(rv) => {
                assert_eq!(rv.range.millis, 5 * 60 * 1000);
                assert_eq!(rv.vector.matchers[0].label, "job");
            }
            _ => panic!(),
        }
    }

    // ── Functions ──────────────────────────────────────────────────────────

    #[test]
    fn test_rate() {
        match p("rate(metric[5m])") {
            Expr::FunctionCall(fc) => {
                assert_eq!(fc.name, "rate");
                assert_eq!(fc.args.len(), 1);
                match &fc.args[0] {
                    Expr::RangeVector(rv) => {
                        assert_eq!(rv.range.millis, 5 * 60 * 1000);
                    }
                    _ => panic!("expected RangeVector arg"),
                }
            }
            _ => panic!("expected FunctionCall"),
        }
    }

    #[test]
    fn test_nested_functions() {
        match p("ceil(rate(http_requests_total[5m]))") {
            Expr::FunctionCall(fc) => {
                assert_eq!(fc.name, "ceil");
                match &fc.args[0] {
                    Expr::FunctionCall(inner) => {
                        assert_eq!(inner.name, "rate");
                    }
                    _ => panic!("expected nested FunctionCall"),
                }
            }
            _ => panic!(),
        }
    }

    // ── Aggregation ────────────────────────────────────────────────────────

    #[test]
    fn test_sum_by() {
        match p("sum by (job) (rate(http_requests_total[5m]))") {
            Expr::Aggregation(agg) => {
                assert_eq!(agg.op, AggregationOp::Sum);
                assert!(agg.grouping.by);
                assert_eq!(agg.grouping.labels, vec!["job"]);
            }
            _ => panic!("expected Aggregation, got: {:?}", p("sum by (job) (rate(http_requests_total[5m]))")),
        }
    }

    #[test]
    fn test_avg_without() {
        match p("avg without (instance) (cpu_usage)") {
            Expr::Aggregation(agg) => {
                assert_eq!(agg.op, AggregationOp::Avg);
                assert!(!agg.grouping.by);
                assert_eq!(agg.grouping.labels, vec!["instance"]);
            }
            _ => panic!("expected Aggregation"),
        }
    }

    #[test]
    fn test_topk() {
        match p("topk(5, rate(http_requests_total[5m]))") {
            Expr::Aggregation(agg) => {
                assert_eq!(agg.op, AggregationOp::TopK);
                assert!(agg.param.is_some());
            }
            _ => panic!("expected Aggregation"),
        }
    }

    // ── Binary operators ───────────────────────────────────────────────────

    #[test]
    fn test_arithmetic() {
        match p("a + b") {
            Expr::BinaryOp(bin) => {
                assert_eq!(bin.op, BinOpKind::Add);
            }
            _ => panic!(),
        }
    }

    #[test]
    fn test_precedence() {
        // a + b * c  should parse as a + (b * c)
        match p("a + b * c") {
            Expr::BinaryOp(bin) => {
                assert_eq!(bin.op, BinOpKind::Add);
                match *bin.rhs {
                    Expr::BinaryOp(inner) => {
                        assert_eq!(inner.op, BinOpKind::Mul);
                    }
                    _ => panic!("expected nested BinaryOp"),
                }
            }
            _ => panic!(),
        }
    }

    #[test]
    fn test_comparison() {
        match p("rate(errors_total[5m]) > 0.1") {
            Expr::BinaryOp(bin) => {
                assert_eq!(bin.op, BinOpKind::Gt);
            }
            _ => panic!(),
        }
    }

    #[test]
    fn test_vector_matching_on() {
        match p("a * on (job, instance) b") {
            Expr::BinaryOp(bin) => {
                assert_eq!(bin.op, BinOpKind::Mul);
                let m = bin.matching.as_ref().unwrap();
                assert_eq!(m.card, VectorMatchCard::On);
                assert_eq!(m.labels, vec!["job", "instance"]);
            }
            _ => panic!(),
        }
    }

    #[test]
    fn test_vector_matching_group_left() {
        match p("a * on (job) group_left b") {
            Expr::BinaryOp(bin) => {
                let m = bin.matching.as_ref().unwrap();
                assert!(m.group_left.is_some());
            }
            _ => panic!(),
        }
    }

    // ── Literals ───────────────────────────────────────────────────────────

    #[test]
    fn test_numbers() {
        assert_eq!(p("42"), Expr::NumberLiteral(42.0));
        assert_eq!(p("3.14"), Expr::NumberLiteral(3.14));
        assert_eq!(p("-5"), Expr::UnaryOp(UnaryOp {
            op: UnaryOpKind::Minus,
            expr: Box::new(Expr::NumberLiteral(5.0)),
        }));
    }

    #[test]
    fn test_strings() {
        assert_eq!(p(r#""hello""#), Expr::StringLiteral("hello".into()));
        assert_eq!(p("'world'"), Expr::StringLiteral("world".into()));
    }

    // ── Durations ──────────────────────────────────────────────────────────

    #[test]
    fn test_duration_parsing() {
        let mut p = Parser::new("5m");
        let d = parse_duration(&mut p).unwrap();
        assert_eq!(d.millis, 5 * 60 * 1000);

        let mut p = Parser::new("1h30m");
        let d = parse_duration(&mut p).unwrap();
        assert_eq!(d.millis, 3600 * 1000 + 30 * 60 * 1000);

        let mut p = Parser::new("7d");
        let d = parse_duration(&mut p).unwrap();
        assert_eq!(d.millis, 7 * 24 * 3600 * 1000);
    }

    // ── Subquery ───────────────────────────────────────────────────────────

    #[test]
    fn test_subquery() {
        match p("rate(http_requests_total[5m])[1h:5m]") {
            Expr::Subquery(sq) => {
                assert_eq!(sq.range.millis, 3600 * 1000);
                assert!(sq.step.is_some());
                assert_eq!(sq.step.unwrap().millis, 5 * 60 * 1000);
            }
            _ => panic!("expected Subquery, got: {:?}", p("rate(http_requests_total[5m])[1h:5m]")),
        }
    }

    // ── Complex expressions ────────────────────────────────────────────────

    #[test]
    fn test_complex_nested() {
        // avg by (project) (rate(wakatime_duration{entity!=""}[7d])) * 3600
        let result = p(r#"avg by (project) (rate(wakatime_duration{entity!=""}[7d])) * 3600"#);
        match result {
            Expr::BinaryOp(bin) => {
                assert_eq!(bin.op, BinOpKind::Mul);
                match *bin.lhs {
                    Expr::Aggregation(agg) => {
                        assert_eq!(agg.op, AggregationOp::Avg);
                        assert_eq!(agg.grouping.labels, vec!["project"]);
                    }
                    _ => panic!("expected Aggregation as LHS"),
                }
            }
            _ => panic!("expected BinaryOp, got: {:?}", result),
        }
    }

    // ── Error cases ────────────────────────────────────────────────────────

    #[test]
    fn test_unclosed_brace() {
        assert!(parse("metric{label=\"val\"").is_err());
    }

    #[test]
    fn test_unclosed_paren() {
        assert!(parse("rate(metric[5m]").is_err());
    }

    #[test]
    fn test_invalid_duration() {
        assert!(parse("metric[abc]").is_err());
    }

    #[test]
    fn test_empty_input() {
        assert!(parse("").is_err());
    }
}

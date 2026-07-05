//! Recursive-descent parser for the Panorama Query Language v0 surface syntax.

use super::ast::*;

/// Parse error with 1-based line/column info.
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

// ── Parser state ────────────────────────────────────────────────────────────

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

    fn expect_char(&mut self, expected: char) -> Result<(), ParseError> {
        self.skip_ws();
        match self.peek() {
            Some(c) if c == expected => { self.pos += 1; Ok(()) }
            _ => self.err(&format!("expected '{}'", expected)),
        }
    }

    fn expect_keyword(&mut self, kw: &str) -> Result<(), ParseError> {
        self.skip_ws();
        let remaining: String = self.input[self.pos..].iter().collect();
        if remaining.to_lowercase().starts_with(&kw.to_lowercase()) {
            self.pos += kw.len();
            Ok(())
        } else {
            self.err(&format!("expected keyword '{}'", kw))
        }
    }

    fn read_identifier(&mut self) -> Result<String, ParseError> {
        self.skip_ws();
        let start = self.pos;
        while let Some(c) = self.peek() {
            if c.is_alphanumeric() || c == '_' {
                self.pos += 1;
            } else {
                break;
            }
        }
        if self.pos == start {
            return self.err("expected identifier");
        }
        Ok(self.input[start..self.pos].iter().collect())
    }

    fn read_string(&mut self) -> Result<String, ParseError> {
        self.skip_ws();
        let quote = self.peek().ok_or_else(|| ParseError {
            message: "unexpected end of input".into(), pos: self.pos,
        })?;
        if quote != '"' && quote != '\'' {
            return self.err("expected string (\"...\" or '...')");
        }
        self.pos += 1; // opening quote
        let start = self.pos;
        while let Some(c) = self.peek() {
            if c == quote {
                let val: String = self.input[start..self.pos].iter().collect();
                self.pos += 1; // closing quote
                return Ok(val);
            }
            if c == '\\' { self.pos += 1; } // skip escaped char
            self.pos += 1;
        }
        self.err("unterminated string")
    }

    fn read_integer(&mut self) -> Result<i64, ParseError> {
        self.skip_ws();
        let start = self.pos;
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() || (self.pos == start && c == '-') {
                self.pos += 1;
            } else {
                break;
            }
        }
        let s: String = self.input[start..self.pos].iter().collect();
        s.parse::<i64>().map_err(|_| ParseError {
            message: format!("invalid integer: {}", s), pos: start,
        })
    }
}

// ── Public entry point ──────────────────────────────────────────────────────

pub fn parse_query(src: &str) -> Result<Query, ParseError> {
    let mut p = Parser::new(src);
    parse_full_query(&mut p)
}

// ── Top level ───────────────────────────────────────────────────────────────

fn parse_full_query(p: &mut Parser) -> Result<Query, ParseError> {
    let mut matches = Vec::new();

    // Parse one or more MATCH clauses
    loop {
        p.skip_ws();
        if p.eof() { break; }
        if !matches.is_empty() {
            // We've already seen at least one MATCH; if the next keyword isn't
            // MATCH, we're done with the MATCH block.
            let remaining: String = p.input[p.pos..].iter().collect();
            if !remaining.to_lowercase().starts_with("match") {
                break;
            }
        }
        matches.push(parse_match(p)?);
    }

    let return_clause = parse_return(p)?;

    let mut order_by = None;
    let mut limit = None;
    let mut skip = None;

    while !p.eof() {
        p.skip_ws();
        let remaining: String = p.input[p.pos..].iter().collect();
        let lower = remaining.to_lowercase();
        if lower.starts_with("order by") {
            order_by = Some(parse_order_by(p)?);
        } else if lower.starts_with("limit") {
            p.expect_keyword("LIMIT")?;
            limit = Some(p.read_integer()? as u64);
        } else if lower.starts_with("skip") {
            p.expect_keyword("SKIP")?;
            skip = Some(p.read_integer()? as u64);
        } else {
            break;
        }
    }

    Ok(Query { matches, return_clause, order_by, limit, skip })
}

// ── MATCH ───────────────────────────────────────────────────────────────────

fn parse_match(p: &mut Parser) -> Result<MatchClause, ParseError> {
    p.expect_keyword("MATCH")?;
    p.expect_char('(')?;
    let variable = p.read_identifier()?;
    p.expect_char(')')?;

    // Check for reference traversal: (a)-[:REF("...")]->(b)
    let source = if let Some(c) = p.peek() {
        if c == '-' {
            return parse_ref_traverse(p, variable);
        }
        MatchSource::Space(parse_in_space(p)?)
    } else {
        return p.err("expected IN space(...) after MATCH");
    };

    let where_clause = if peek_keyword(p, "WHERE") {
        p.expect_keyword("WHERE")?;
        Some(WhereClause { predicate: parse_predicates(p)? })
    } else {
        None
    };

    Ok(MatchClause { variable, source, where_clause })
}

fn parse_ref_traverse(p: &mut Parser, from_var: String) -> Result<MatchClause, ParseError> {
    // -[:REF("edge")]->
    p.expect_char('-')?;
    p.expect_char('[')?;
    p.expect_char(':')?;
    p.expect_keyword("REF")?;
    p.expect_char('(')?;
    let edge_type = p.read_string()?;
    p.expect_char(')')?;

    // Optional depth: *1 or *1..3
    let (min_depth, max_depth) = if p.peek() == Some('*') {
        p.pos += 1;
        let min = p.read_integer()? as u32;
        let max = if p.peek() == Some('.') {
            p.pos += 1;
            p.expect_char('.')?;
            p.read_integer()? as u32
        } else {
            min
        };
        (min, max)
    } else {
        (1, 1)
    };

    p.expect_char(']')?;
    // Direction arrow
    p.expect_char('-')?;
    p.expect_char('>')?;

    p.expect_char('(')?;
    let target_var = p.read_identifier()?;
    p.expect_char(')')?;

    let source = MatchSource::RefTraverse {
        edge_type,
        min_depth,
        max_depth,
        target_var: target_var.clone(),
        target_source: Box::new(MatchSource::Space(parse_in_space(p)?)),
    };

    let where_clause = if peek_keyword(p, "WHERE") {
        p.expect_keyword("WHERE")?;
        Some(WhereClause { predicate: parse_predicates(p)? })
    } else {
        None
    };

    Ok(MatchClause { variable: from_var, source, where_clause })
}

fn parse_in_space(p: &mut Parser) -> Result<String, ParseError> {
    p.expect_keyword("IN")?;
    p.expect_keyword("space")?;
    p.expect_char('(')?;
    let name = p.read_string()?;
    p.expect_char(')')?;
    Ok(name)
}

// ── WHERE predicates ────────────────────────────────────────────────────────

fn peek_keyword(p: &Parser, kw: &str) -> bool {
    let remaining: String = p.input[p.pos..].iter().collect();
    remaining.trim_start().to_lowercase().starts_with(&kw.to_lowercase())
}

fn parse_predicates(p: &mut Parser) -> Result<Predicate, ParseError> {
    // OR has lower precedence: a AND b OR c AND d  →  Or(And(a,b), And(c,d))
    let mut left = parse_and_expr(p)?;
    while peek_keyword(p, "OR") {
        p.expect_keyword("OR")?;
        let right = parse_and_expr(p)?;
        left = Predicate::Or(Box::new(left), Box::new(right));
    }
    Ok(left)
}

fn parse_and_expr(p: &mut Parser) -> Result<Predicate, ParseError> {
    let mut left = parse_atomic_predicate(p)?;
    while peek_keyword(p, "AND") {
        p.expect_keyword("AND")?;
        let right = parse_atomic_predicate(p)?;
        left = Predicate::And(Box::new(left), Box::new(right));
    }
    Ok(left)
}

fn parse_atomic_predicate(p: &mut Parser) -> Result<Predicate, ParseError> {
    parse_predicate(p)
}

fn parse_predicate(p: &mut Parser) -> Result<Predicate, ParseError> {
    p.skip_ws();

    // HAS_FIELD(...) ?
    if peek_keyword(p, "HAS_FIELD") {
        return parse_has_field(p);
    }

    // SCAN(...) ?
    if peek_keyword(p, "SCAN") {
        return parse_scan(p);
    }

    // Read the first identifier and decide what follows.
    let start = p.pos;
    let first = p.read_identifier()?;

    // `n CONFORMS TO schema(...)` — variable followed by CONFORMS
    if peek_keyword(p, "CONFORMS") {
        p.pos = start;
        return parse_conforms(p);
    }

    // `n.ns.field ...` — variable followed by a dot = field path expression
    if p.peek() == Some('.') {
        p.pos = start;
        return parse_field_expr(p);
    }

    // `n IS NULL` ?
    if peek_keyword(p, "IS") {
        p.pos = start;
        return parse_field_expr(p);
    }

    // `n IN [...]` ?
    if peek_keyword(p, "IN") {
        p.pos = start;
        return parse_field_expr(p);
    }

    // `n LIKE ...` ?
    if peek_keyword(p, "LIKE") {
        p.pos = start;
        return parse_field_expr(p);
    }

    p.err(&format!(
        "unexpected token after '{}' in WHERE clause — expected CONFORMS TO, '.' (field path), IS, IN, or LIKE",
        first
    ))
}

fn parse_conforms(p: &mut Parser) -> Result<Predicate, ParseError> {
    let var = p.read_identifier()?;
    p.expect_keyword("CONFORMS")?;
    p.expect_keyword("TO")?;
    p.expect_keyword("schema")?;
    p.expect_char('(')?;
    let schema_id = p.read_string()?;

    let (version_min, version_max) = if p.peek() == Some(',') {
        p.pos += 1;
        let v = p.read_integer()? as u32;
        if p.peek() == Some('.') {
            p.pos += 1;
            p.expect_char('.')?;
            let max = p.read_integer()? as u32;
            (Some(v), Some(max))
        } else {
            (Some(v), None)
        }
    } else {
        (None, None)
    };
    p.expect_char(')')?;

    Ok(Predicate::ConformsTo { field: var, schema_id, version_min, version_max })
}

fn parse_has_field(p: &mut Parser) -> Result<Predicate, ParseError> {
    p.expect_keyword("HAS_FIELD")?;
    p.expect_char('(')?;
    let variable = p.read_identifier()?;
    p.expect_char(',')?;
    let namespace = p.read_string()?;
    p.expect_char(',')?;
    let field_name = p.read_string()?;
    p.expect_char(')')?;
    Ok(Predicate::HasField { variable, namespace, field_name })
}

fn parse_scan(p: &mut Parser) -> Result<Predicate, ParseError> {
    p.expect_keyword("SCAN")?;
    p.expect_char('(')?;
    let mut pred = parse_field_expr(p)?;
    // Mark the predicate as a scan
    match &mut pred {
        Predicate::FieldCompare { scan, .. } => *scan = true,
        Predicate::Like { .. } => {} // LIKE is always a scan for now
        _ => {}
    }
    p.expect_char(')')?;
    Ok(pred)
}

fn parse_field_expr(p: &mut Parser) -> Result<Predicate, ParseError> {
    let field_path = parse_field_path(p)?;

    p.skip_ws();

    // IS NULL / IS NOT NULL
    if peek_keyword(p, "IS") {
        p.expect_keyword("IS")?;
        let not = if peek_keyword(p, "NOT") {
            p.expect_keyword("NOT")?;
            true
        } else {
            false
        };
        p.expect_keyword("NULL")?;
        return Ok(Predicate::IsNull { field_path, not });
    }

    // IN [...]
    if peek_keyword(p, "IN") {
        p.expect_keyword("IN")?;
        p.expect_char('[')?;
        let mut values = Vec::new();
        loop {
            p.skip_ws();
            if p.peek() == Some(']') { p.pos += 1; break; }
            values.push(parse_value(p)?);
            p.skip_ws();
            if p.peek() == Some(',') { p.pos += 1; }
        }
        return Ok(Predicate::In { field_path, values, not: false });
    }

    // LIKE
    if peek_keyword(p, "LIKE") {
        p.expect_keyword("LIKE")?;
        let pattern = p.read_string()?;
        return Ok(Predicate::Like { field_path, pattern, not: false });
    }

    // Comparison operator
    let op = parse_cmp_op(p)?;
    let value = parse_value(p)?;
    Ok(Predicate::FieldCompare { field_path, op, value, scan: false })
}

fn parse_field_path(p: &mut Parser) -> Result<FieldPath, ParseError> {
    let variable = p.read_identifier()?;
    p.expect_char('.')?;

    // Check if next is a quoted namespace or bare field
    let first = p.read_identifier()?;
    if p.peek() == Some('.') {
        // Form: var.ns.field
        p.pos += 1; // skip '.'
        let field = p.read_identifier()?;
        Ok(FieldPath { variable, namespace: Some(first), field })
    } else {
        // Form: var.field
        Ok(FieldPath { variable, namespace: None, field: first })
    }
}

fn parse_cmp_op(p: &mut Parser) -> Result<CmpOp, ParseError> {
    p.skip_ws();
    match p.peek() {
        Some('=') => { p.pos += 1; Ok(CmpOp::Eq) }
        Some('!') => {
            p.pos += 1;
            if p.peek() == Some('=') { p.pos += 1; Ok(CmpOp::Neq) }
            else { p.err("expected '!='") }
        }
        Some('<') => {
            p.pos += 1;
            if p.peek() == Some('=') { p.pos += 1; Ok(CmpOp::Lte) }
            else if p.peek() == Some('>') { p.pos += 1; Ok(CmpOp::Neq) }
            else { Ok(CmpOp::Lt) }
        }
        Some('>') => {
            p.pos += 1;
            if p.peek() == Some('=') { p.pos += 1; Ok(CmpOp::Gte) }
            else { Ok(CmpOp::Gt) }
        }
        _ => p.err("expected comparison operator (= != < <= > >=)"),
    }
}

fn parse_value(p: &mut Parser) -> Result<Value, ParseError> {
    p.skip_ws();
    match p.peek() {
        Some('"') | Some('\'') => Ok(Value::String(p.read_string()?)),
        Some('-') | Some('0'..='9') => {
            // Try integer first, then float
            let start = p.pos;
            if let Ok(i) = p.read_integer() {
                if p.peek() == Some('.') {
                    p.pos += 1;
                    let frac_start = p.pos;
                    while let Some(c) = p.peek() {
                        if c.is_ascii_digit() { p.pos += 1; } else { break; }
                    }
                    let frac: String = p.input[frac_start..p.pos].iter().collect();
                    let s = format!("{}.{}", i, frac);
                    return Ok(Value::Float(s.parse::<f64>().map_err(|_| ParseError {
                        message: "invalid float".into(), pos: start,
                    })?));
                }
                Ok(Value::Integer(i))
            } else {
                p.err("expected number")
            }
        }
        Some('t') | Some('T') => {
            if peek_keyword(p, "true") { p.expect_keyword("true")?; Ok(Value::Boolean(true)) }
            else { p.err("expected 'true'") }
        }
        Some('f') | Some('F') => {
            if peek_keyword(p, "false") { p.expect_keyword("false")?; Ok(Value::Boolean(false)) }
            else { p.err("expected 'false'") }
        }
        Some('n') | Some('N') => {
            if peek_keyword(p, "null") { p.expect_keyword("null")?; Ok(Value::Null) }
            else { p.err("expected 'null'") }
        }
        _ => p.err("expected value (string, number, boolean, null)"),
    }
}

// ── RETURN ──────────────────────────────────────────────────────────────────

fn parse_return(p: &mut Parser) -> Result<ReturnClause, ParseError> {
    p.expect_keyword("RETURN")?;
    let mut columns = Vec::new();

    loop {
        p.skip_ws();
        if p.eof() { break; }

        let remaining: String = p.input[p.pos..].iter().collect();
        let lower = remaining.to_lowercase();
        if lower.starts_with("order") || lower.starts_with("limit") || lower.starts_with("skip") {
            break;
        }

        let col = parse_return_column(p)?;
        columns.push(col);

        p.skip_ws();
        if p.peek() == Some(',') { p.pos += 1; } else { break; }
    }

    if columns.is_empty() {
        return p.err("expected at least one column in RETURN");
    }
    Ok(ReturnClause { columns })
}

fn parse_return_column(p: &mut Parser) -> Result<ReturnColumn, ParseError> {
    p.skip_ws();

    let start = p.pos;
    let variable = p.read_identifier()?;

    let expression = if p.peek() == Some('.') {
        // n.field or n.ns.field — backtrack and parse as field path
        p.pos = start;
        ReturnExpr::Field(parse_field_path(p)?)
    } else {
        // Bare variable: whole node
        ReturnExpr::Node(variable)
    };

    let alias = if peek_keyword(p, "AS") {
        p.expect_keyword("AS")?;
        Some(p.read_identifier()?)
    } else {
        None
    };

    Ok(ReturnColumn { expression, alias })
}

// ── ORDER BY ────────────────────────────────────────────────────────────────

fn parse_order_by(p: &mut Parser) -> Result<OrderBy, ParseError> {
    p.expect_keyword("ORDER")?;
    p.expect_keyword("BY")?;
    let field = parse_field_path(p)?;
    let direction = if peek_keyword(p, "ASC") {
        p.expect_keyword("ASC")?;
        OrderDir::Asc
    } else if peek_keyword(p, "DESC") {
        p.expect_keyword("DESC")?;
        OrderDir::Desc
    } else {
        OrderDir::Asc // default
    };
    Ok(OrderBy { field, direction })
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_match() {
        let q = parse_query(r#"MATCH (n) IN space("personal") RETURN n"#).unwrap();
        assert_eq!(q.matches.len(), 1);
        assert_eq!(q.matches[0].variable, "n");
        if let MatchSource::Space(ref s) = q.matches[0].source {
            assert_eq!(s, "personal");
        } else { panic!("expected Space source"); }
        assert_eq!(q.return_clause.columns.len(), 1);
    }

    #[test]
    fn test_conforms_to() {
        let q = parse_query(
            r#"MATCH (n) IN space("personal") WHERE n CONFORMS TO schema("com.example.event") RETURN n.title AS title"#,
        ).unwrap();
        assert!(q.matches[0].where_clause.is_some());
        let pred = &q.matches[0].where_clause.as_ref().unwrap().predicate;
        match pred {
            Predicate::ConformsTo { schema_id, .. } => assert_eq!(schema_id, "com.example.event"),
            _ => panic!("expected ConformsTo"),
        }
    }

    #[test]
    fn test_field_compare() {
        let q = parse_query(
            r#"MATCH (n) IN space("personal") WHERE n.start_time > "2026-01-01" RETURN n"#,
        ).unwrap();
        let pred = &q.matches[0].where_clause.as_ref().unwrap().predicate;
        match pred {
            Predicate::FieldCompare { field_path, op, value, .. } => {
                assert_eq!(field_path.field, "start_time");
                assert_eq!(*op, CmpOp::Gt);
            }
            _ => panic!("expected FieldCompare"),
        }
    }

    #[test]
    fn test_limit_skip() {
        let q = parse_query(
            r#"MATCH (n) IN space("personal") RETURN n LIMIT 50 SKIP 100"#,
        ).unwrap();
        assert_eq!(q.limit, Some(50));
        assert_eq!(q.skip, Some(100));
    }

    #[test]
    fn test_order_by() {
        let q = parse_query(
            r#"MATCH (n) IN space("personal") RETURN n ORDER BY n.start_time DESC"#,
        ).unwrap();
        assert!(q.order_by.is_some());
        let ob = q.order_by.as_ref().unwrap();
        assert_eq!(ob.field.field, "start_time");
        assert!(matches!(ob.direction, OrderDir::Desc));
    }

    #[test]
    fn test_has_field() {
        let q = parse_query(
            r#"MATCH (n) IN space("personal") WHERE HAS_FIELD(n, "app", "attendees") RETURN n"#,
        ).unwrap();
        let pred = &q.matches[0].where_clause.as_ref().unwrap().predicate;
        match pred {
            Predicate::HasField { namespace, field_name, .. } => {
                assert_eq!(namespace, "app");
                assert_eq!(field_name, "attendees");
            }
            _ => panic!("expected HasField"),
        }
    }

    #[test]
    fn test_ref_traverse() {
        let q = parse_query(
            r#"MATCH (a)-[:REF("attendee")]->(b) IN space("personal") RETURN a.title, b.name LIMIT 100"#,
        ).unwrap();
        match &q.matches[0].source {
            MatchSource::RefTraverse { edge_type, target_var, .. } => {
                assert_eq!(edge_type, "attendee");
                assert_eq!(target_var, "b");
            }
            _ => panic!("expected RefTraverse"),
        }
    }
}

//! A real parser and evaluator for `.only-pchi` rule files.
//!
//! This replaces the previous `RuleEngine`, which advertised "Only-Lang
//! rules" but actually held four rules hand-written as Rust structs
//! (`load_default_rules()`) — nothing was ever parsed from the `.only`
//! example file shipped alongside it. This module parses real text files.
//!
//! **Scope, stated plainly.** The original `pchi-schema/examples/kraken-rules.only`
//! uses `for` loops and `sum(... for i in 0..8)` comprehensions — a small
//! general-purpose language. Implementing that correctly is a larger
//! undertaking than this pass covers, so this parser supports a
//! documented, real subset instead of a half-correct version of the full
//! grammar:
//!
//! ```text
//! # comment
//! harmony(<float>)
//!
//! if <condition> then
//!     <action>
//!     <action>*
//! end
//! ```
//! `condition := term (("and" | "or") term)*`
//! `term := path op literal`, `path := ident ("." ident)*`,
//! `op := "==" | "!=" | ">=" | "<=" | ">" | "<"`,
//! `literal := float | "true" | "false" | "quoted string"`
//!
//! Actions: `escalate("msg")`, `deny("msg")`, `evolve(target, param)`,
//! `residual()` (parsed, intentionally a no-op — matches the source
//! file's use of it as an equilibrium-recheck marker, not a state
//! mutation).
//!
//! `pchi-schema/examples/kraken-rules.only` is kept as-is and documents
//! this at its head as the target syntax; `kraken-tentacle.only-pchi` in
//! the same directory is the fully-parseable rewrite this crate actually
//! loads by default.

use crate::state::{EquilibriumStatus, SceneState};
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    Number(f64),
    Bool(bool),
    Str(String),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Op {
    Eq,
    Ne,
    Gt,
    Lt,
    Ge,
    Le,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BoolOp {
    And,
    Or,
}

#[derive(Debug, Clone)]
pub struct Term {
    pub path: String,
    pub op: Op,
    pub literal: Literal,
}

#[derive(Debug, Clone)]
pub struct Condition {
    pub first: Term,
    /// (connector, term) pairs, evaluated strictly left-to-right (no
    /// precedence between `and`/`or` — parenthesized grouping is not
    /// supported in this subset).
    pub rest: Vec<(BoolOp, Term)>,
}

#[derive(Debug, Clone)]
pub enum Action {
    Escalate(String),
    Deny(String),
    Evolve(String, String),
    /// Parsed, intentionally not acted on — see module docs.
    Residual,
}

#[derive(Debug, Clone)]
pub struct Rule {
    pub name: String,
    pub condition: Condition,
    pub actions: Vec<Action>,
}

#[derive(Debug, Clone)]
pub struct RuleFile {
    pub harmony_threshold: f64,
    pub rules: Vec<Rule>,
}

#[derive(Debug)]
pub struct ParseError {
    pub line: usize,
    pub message: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}
impl std::error::Error for ParseError {}

/// Parse a `.only-pchi` source string into a `RuleFile`.
pub fn parse(source: &str) -> Result<RuleFile, ParseError> {
    let mut harmony_threshold = 1e-12;
    let mut rules = Vec::new();
    let mut rule_index = 0usize;

    let lines: Vec<&str> = source.lines().collect();
    let mut i = 0usize;
    while i < lines.len() {
        let lineno = i + 1;
        let raw = lines[i];
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            i += 1;
            continue;
        }

        if let Some(rest) = line.strip_prefix("harmony(") {
            let inner = rest
                .strip_suffix(')')
                .ok_or_else(|| perr(lineno, "harmony(...) missing closing paren"))?;
            harmony_threshold = inner
                .trim()
                .parse::<f64>()
                .map_err(|_| perr(lineno, format!("invalid harmony() threshold: {}", inner)))?;
            i += 1;
            continue;
        }

        if let Some(rest) = line.strip_prefix("if ") {
            // Condition may end with "then" on the same line, or "then" may
            // be the entire next non-blank line.
            let (cond_src, mut j) = if let Some(stripped) = rest.strip_suffix("then") {
                (stripped.trim().to_string(), i + 1)
            } else {
                let mut cond_src = rest.trim().to_string();
                let mut k = i + 1;
                loop {
                    if k >= lines.len() {
                        return Err(perr(lineno, "if without a matching then"));
                    }
                    let next = strip_comment(lines[k]).trim();
                    if next == "then" {
                        k += 1;
                        break;
                    }
                    if next.is_empty() {
                        k += 1;
                        continue;
                    }
                    // Condition continues onto this line.
                    cond_src.push(' ');
                    cond_src.push_str(next);
                    k += 1;
                }
                (cond_src, k)
            };
            let condition = parse_condition(&cond_src, lineno)?;

            let mut actions = Vec::new();
            loop {
                if j >= lines.len() {
                    return Err(perr(lineno, "if block missing end"));
                }
                let body_line = strip_comment(lines[j]).trim();
                j += 1;
                if body_line.is_empty() {
                    continue;
                }
                if body_line == "end" {
                    break;
                }
                actions.push(parse_action(body_line, j)?);
            }

            rule_index += 1;
            rules.push(Rule {
                name: format!("rule_{}", rule_index),
                condition,
                actions,
            });
            i = j;
            continue;
        }

        return Err(perr(lineno, format!("unsupported top-level statement: {}", line)));
    }

    Ok(RuleFile {
        harmony_threshold,
        rules,
    })
}

fn perr(line: usize, message: impl Into<String>) -> ParseError {
    ParseError {
        line,
        message: message.into(),
    }
}

fn strip_comment(line: &str) -> &str {
    match line.find('#') {
        Some(idx) => &line[..idx],
        None => line,
    }
}

fn parse_condition(src: &str, lineno: usize) -> Result<Condition, ParseError> {
    // Split on the lowest-precedence-free `and`/`or` tokens, word-boundary
    // aware (so e.g. a path containing "android" is not mis-split).
    let tokens = split_bool_tokens(src);
    if tokens.is_empty() {
        return Err(perr(lineno, "empty condition"));
    }
    let first = parse_term(&tokens[0], lineno)?;
    let mut rest = Vec::new();
    let mut k = 1;
    while k < tokens.len() {
        let op = match tokens[k].as_str() {
            "and" => BoolOp::And,
            "or" => BoolOp::Or,
            other => return Err(perr(lineno, format!("expected 'and'/'or', found '{}'", other))),
        };
        if k + 1 >= tokens.len() {
            return Err(perr(lineno, "dangling 'and'/'or' with no following term"));
        }
        let term = parse_term(&tokens[k + 1], lineno)?;
        rest.push((op, term));
        k += 2;
    }
    Ok(Condition { first, rest })
}

fn split_bool_tokens(src: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_str = false;
    let words: Vec<&str> = src.split_whitespace().collect();
    // Reconstruct while tracking quoted strings so "and"/"or" inside a
    // literal string doesn't get treated as a connector.
    let mut i = 0;
    while i < words.len() {
        let w = words[i];
        let quote_count = w.matches('"').count();
        if !in_str && (w == "and" || w == "or") {
            out.push(cur.trim().to_string());
            cur.clear();
            out.push(w.to_string());
            i += 1;
            continue;
        }
        if !cur.is_empty() {
            cur.push(' ');
        }
        cur.push_str(w);
        if quote_count % 2 == 1 {
            in_str = !in_str;
        }
        i += 1;
    }
    if !cur.trim().is_empty() {
        out.push(cur.trim().to_string());
    }
    out
}

fn parse_term(src: &str, lineno: usize) -> Result<Term, ParseError> {
    const OPS: &[(&str, Op)] = &[
        ("==", Op::Eq),
        ("!=", Op::Ne),
        (">=", Op::Ge),
        ("<=", Op::Le),
        (">", Op::Gt),
        ("<", Op::Lt),
    ];
    for (sym, op) in OPS {
        if let Some(idx) = src.find(sym) {
            let path = src[..idx].trim().to_string();
            let lit_src = src[idx + sym.len()..].trim();
            let literal = parse_literal(lit_src, lineno)?;
            if path.is_empty() {
                return Err(perr(lineno, format!("missing path before '{}'", sym)));
            }
            return Ok(Term { path, op: *op, literal });
        }
    }
    Err(perr(lineno, format!("no comparison operator found in condition term: {}", src)))
}

fn parse_literal(src: &str, lineno: usize) -> Result<Literal, ParseError> {
    if src == "true" {
        return Ok(Literal::Bool(true));
    }
    if src == "false" {
        return Ok(Literal::Bool(false));
    }
    if src.starts_with('"') && src.ends_with('"') && src.len() >= 2 {
        return Ok(Literal::Str(src[1..src.len() - 1].to_string()));
    }
    src.parse::<f64>()
        .map(Literal::Number)
        .map_err(|_| perr(lineno, format!("invalid literal: {}", src)))
}

fn parse_action(src: &str, lineno: usize) -> Result<Action, ParseError> {
    if let Some(rest) = src.strip_prefix("escalate(").and_then(|s| s.strip_suffix(')')) {
        return Ok(Action::Escalate(unquote(rest, lineno)?));
    }
    if let Some(rest) = src.strip_prefix("deny(").and_then(|s| s.strip_suffix(')')) {
        return Ok(Action::Deny(unquote(rest, lineno)?));
    }
    if let Some(rest) = src.strip_prefix("evolve(").and_then(|s| s.strip_suffix(')')) {
        let parts: Vec<&str> = rest.splitn(2, ',').collect();
        if parts.len() != 2 {
            return Err(perr(lineno, "evolve(target, param) requires two arguments"));
        }
        return Ok(Action::Evolve(parts[0].trim().to_string(), parts[1].trim().to_string()));
    }
    if src == "residual()" {
        return Ok(Action::Residual);
    }
    Err(perr(lineno, format!("unsupported action: {}", src)))
}

fn unquote(src: &str, lineno: usize) -> Result<String, ParseError> {
    let s = src.trim();
    if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
        Ok(s[1..s.len() - 1].to_string())
    } else {
        Err(perr(lineno, format!("expected a quoted string, found: {}", s)))
    }
}

/// Resolve a dotted path against the scene state. `equilibrium` is a
/// special case reading the current residual; `musical_context.*` and
/// `artist_tracking.position.*` are special-cased to their typed fields;
/// everything else — including multi-segment paths like
/// `tentacle_system.total_curl` — is looked up verbatim as a
/// `control_parameter` key, matching how the state is actually populated
/// by `MessageType::ControlParameter` (`{target_id}.{parameter}`).
pub fn resolve_path(path: &str, state: &SceneState) -> Option<Literal> {
    if path == "equilibrium" {
        return Some(Literal::Number(match &state.equilibrium_status {
            EquilibriumStatus::Disequilibrium { residual } => residual.abs(),
            EquilibriumStatus::Equilibrium => 0.0,
            EquilibriumStatus::Unknown => return None,
        }));
    }
    if let Some(field) = path.strip_prefix("musical_context.") {
        let ctx = state.musical_context.as_ref()?;
        return match field {
            "bpm" => Some(Literal::Number(ctx.bpm)),
            "beat" => Some(Literal::Number(ctx.beat as f64)),
            "kick" => Some(Literal::Bool(ctx.kick)),
            "snare" => Some(Literal::Bool(ctx.snare)),
            "section" => Some(Literal::Str(ctx.section.clone())),
            _ => None,
        };
    }
    if let Some(field) = path.strip_prefix("artist_tracking.position.") {
        let t = state.artist_tracking.as_ref()?;
        return match field {
            "x" => Some(Literal::Number(t.position[0])),
            "y" => Some(Literal::Number(t.position[1])),
            "z" => Some(Literal::Number(t.position[2])),
            _ => None,
        };
    }
    state.get_control_parameter(path).map(Literal::Number)
}

fn compare(actual: &Literal, op: Op, expected: &Literal) -> bool {
    match (actual, expected) {
        (Literal::Number(a), Literal::Number(b)) => match op {
            Op::Eq => (a - b).abs() < 1e-9,
            Op::Ne => (a - b).abs() >= 1e-9,
            Op::Gt => a > b,
            Op::Lt => a < b,
            Op::Ge => a >= b,
            Op::Le => a <= b,
        },
        (Literal::Bool(a), Literal::Bool(b)) => match op {
            Op::Eq => a == b,
            Op::Ne => a != b,
            _ => false,
        },
        (Literal::Str(a), Literal::Str(b)) => match op {
            Op::Eq => a == b,
            Op::Ne => a != b,
            _ => false,
        },
        _ => false,
    }
}

pub fn eval_condition(cond: &Condition, state: &SceneState) -> bool {
    let mut result = eval_term(&cond.first, state);
    for (op, term) in &cond.rest {
        let next = eval_term(term, state);
        result = match op {
            BoolOp::And => result && next,
            BoolOp::Or => result || next,
        };
    }
    result
}

fn eval_term(term: &Term, state: &SceneState) -> bool {
    match resolve_path(&term.path, state) {
        Some(actual) => compare(&actual, term.op, &term.literal),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::MusicalContext;

    const SAMPLE: &str = r#"
# comment line
harmony(1e-10)

if tentacle_system.total_curl > 4.0 then
    deny("Tentacle curl exceeds harmonic threshold")
    residual()
end

if musical_context.bpm > 120.0 and musical_context.kick == true
then
    evolve(kraken_main, peak_state)
end
"#;

    #[test]
    fn parses_sample_file() {
        let f = parse(SAMPLE).expect("should parse");
        assert_eq!(f.harmony_threshold, 1e-10);
        assert_eq!(f.rules.len(), 2);
        assert_eq!(f.rules[1].condition.rest.len(), 1);
        assert!(matches!(f.rules[0].actions[0], Action::Deny(_)));
        assert!(matches!(f.rules[0].actions[1], Action::Residual));
        assert!(matches!(f.rules[1].actions[0], Action::Evolve(_, _)));
    }

    #[test]
    fn evaluates_against_state() {
        let f = parse(SAMPLE).unwrap();
        let mut state = SceneState::new();
        state.set_control_parameter("tentacle_system.total_curl".to_string(), 5.0);
        assert!(eval_condition(&f.rules[0].condition, &state));

        state.set_control_parameter("tentacle_system.total_curl".to_string(), 1.0);
        assert!(!eval_condition(&f.rules[0].condition, &state));
    }

    #[test]
    fn evaluates_and_condition_across_types() {
        let f = parse(SAMPLE).unwrap();
        let mut state = SceneState::new();
        state.update_musical_context(MusicalContext {
            bpm: 128.0,
            beat: 1,
            kick: true,
            snare: false,
            section: "drop".to_string(),
        });
        assert!(eval_condition(&f.rules[1].condition, &state));

        state.update_musical_context(MusicalContext {
            bpm: 128.0,
            beat: 1,
            kick: false,
            snare: false,
            section: "drop".to_string(),
        });
        assert!(!eval_condition(&f.rules[1].condition, &state));
    }

    #[test]
    fn rejects_unsupported_loop_syntax() {
        let err = parse("for tentacle in tentacle_system.tentacles\nend").unwrap_err();
        assert!(err.message.contains("unsupported"));
    }

    #[test]
    fn error_messages_carry_line_numbers() {
        let err = parse("if x > \nend").unwrap_err();
        assert_eq!(err.line, 1);
    }
}

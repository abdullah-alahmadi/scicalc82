//! The display tree, built from the input tree or from a formatted result.

use calc_core::calculator::ResultLine;
use calc_core::format::{Formatted, SurdTermText};
use calc_core::input::{Cursor, Func, Item, Row, SlotRef, Template, TemplateKind, Token};
use calc_core::stats::StatVar;

#[derive(Clone, Debug, PartialEq)]
pub enum Node {
    Text(String),
    /// Raised and smaller, attached to the preceding node (exponents).
    Sup(Vec<Node>),
    /// Lowered and smaller (the base of log_□).
    Sub(Vec<Node>),
    Frac(Vec<Node>, Vec<Node>),
    Root {
        index: Option<Vec<Node>>,
        radicand: Vec<Node>,
    },
    Abs(Vec<Node>),
    /// A letter with a bar (x̄) or hat (x̂) over it.
    Accent {
        text: String,
        hat: bool,
    },
    /// The LineIO fraction mark ⌟, drawn since no bundled font has it.
    FracMark,
    /// An empty slot, drawn as a small box.
    Empty,
    /// Where the input cursor is.
    Cursor,
}

fn text(s: impl Into<String>) -> Node {
    Node::Text(s.into())
}

type CursorRef<'a> = Option<(&'a [SlotRef], usize)>;

/// The input as a display tree, with a [`Node::Cursor`] marker if `cursor` is given.
pub fn input_nodes(row: &Row, cursor: Option<&Cursor>) -> Vec<Node> {
    row_nodes(row, cursor.map(|c| (c.path.as_slice(), c.pos)))
}

/// Whether a token continues the exponent typed after ×10ˣ: digits, a
/// point, and a sign as the first character.
fn continues_exponent(token: Token, run: &[Node]) -> bool {
    let started = run.iter().any(|n| matches!(n, Node::Text(_)));
    match token {
        Token::Digit(_) | Token::Point => true,
        Token::Neg | Token::Minus | Token::Plus => !started,
        _ => false,
    }
}

fn row_nodes(row: &Row, cursor: CursorRef<'_>) -> Vec<Node> {
    let cursor_here = |i: usize| matches!(cursor, Some(([], pos)) if pos == i);
    let mut out = Vec::new();
    // Digits typed after ×10ˣ are drawn raised, like the device.
    let mut exponent: Option<Vec<Node>> = None;
    let flush = |out: &mut Vec<Node>, run: Vec<Node>| {
        let empty = !run.iter().any(|n| matches!(n, Node::Text(_)));
        let mut run = run;
        if empty {
            run.push(Node::Empty);
        }
        out.push(Node::Sup(run));
    };

    for (i, item) in row.items.iter().enumerate() {
        if let Some(run) = &mut exponent {
            if let Item::Token(t) = item
                && continues_exponent(*t, run)
            {
                if cursor_here(i) {
                    run.push(Node::Cursor);
                }
                run.push(token_text(*t));
                continue;
            }
            // A cursor just after the exponent stays raised: typing there extends it.
            let mut run = exponent.take().expect("checked above");
            if cursor_here(i) {
                run.push(Node::Cursor);
            }
            flush(&mut out, run);
        } else if cursor_here(i) {
            out.push(Node::Cursor);
        }
        match item {
            Item::Token(Token::Exp10) => {
                out.push(text("×10"));
                exponent = Some(Vec::new());
            }
            Item::Token(t) => out.extend(token_nodes(*t)),
            Item::Template(t) => template_nodes(&mut out, t, |slot| match cursor {
                Some(([first, rest @ ..], pos)) if first.item == i && first.slot == slot => Some((rest, pos)),
                _ => None,
            }),
        }
    }
    match exponent {
        Some(mut run) => {
            if cursor_here(row.len()) {
                run.push(Node::Cursor);
            }
            flush(&mut out, run);
        }
        None if cursor_here(row.len()) => out.push(Node::Cursor),
        None => {}
    }
    out
}

fn token_text(t: Token) -> Node {
    match t {
        Token::Minus => text("−"),
        Token::Neg => text("-"),
        _ => text(t.to_string()),
    }
}

/// Plain text such as a menu label, with accented letters (x̄, ȳ, x̂, ŷ)
/// turned into drawn accents, since egui doesn't place combining marks.
pub fn label_nodes(label: &str) -> Vec<Node> {
    const MACRON: char = '\u{0304}';
    const CIRCUMFLEX: char = '\u{0302}';
    let mut out = Vec::new();
    let mut plain = String::new();
    let mut chars = label.chars().peekable();
    while let Some(c) = chars.next() {
        let accented = match c {
            'ȳ' => Some(('y', false)),
            'ŷ' => Some(('y', true)),
            _ => chars.next_if(|&n| n == MACRON || n == CIRCUMFLEX).map(|mark| (c, mark == CIRCUMFLEX)),
        };
        match accented {
            Some((base, hat)) => {
                if !plain.is_empty() {
                    out.push(Node::Text(std::mem::take(&mut plain)));
                }
                out.push(Node::Accent { text: base.to_string(), hat });
            }
            None => plain.push(c),
        }
    }
    if !plain.is_empty() {
        out.push(Node::Text(plain));
    }
    out
}

fn accent(s: &str, hat: bool) -> Node {
    Node::Accent { text: s.into(), hat }
}

fn sup(s: &str) -> Node {
    Node::Sup(vec![text(s)])
}

fn token_nodes(t: Token) -> Vec<Node> {
    match t {
        Token::Square => vec![sup("2")],
        Token::Cube => vec![sup("3")],
        Token::Recip => vec![sup("−1")],
        Token::AngleRad => vec![sup("r")],
        Token::AngleGrad => vec![sup("g")],
        Token::FracBar => vec![Node::FracMark],
        Token::RootOpen => vec![sup("x"), text("√(")],
        Token::Stat(StatVar::MeanX) => vec![accent("x", false)],
        Token::Stat(StatVar::MeanY) => vec![accent("y", false)],
        Token::EstX => vec![accent("x", true)],
        Token::EstY => vec![accent("y", true)],
        Token::Func(f @ (Func::Asin | Func::Acos | Func::Atan | Func::Asinh | Func::Acosh | Func::Atanh)) => {
            let base = f.name().trim_end_matches('(').trim_end_matches("⁻¹");
            vec![text(base), sup("−1"), text("(")]
        }
        _ => vec![token_text(t)],
    }
}

fn template_nodes<'a>(out: &mut Vec<Node>, t: &Template, cursor_in: impl Fn(usize) -> CursorRef<'a>) {
    let slot = |i: usize| {
        let row = &t.slots[i];
        let cursor = cursor_in(i);
        if row.is_empty() {
            let mut nodes = Vec::new();
            if cursor.is_some() {
                nodes.push(Node::Cursor);
            }
            nodes.push(Node::Empty);
            nodes
        } else {
            row_nodes(row, cursor)
        }
    };
    match t.kind {
        TemplateKind::Fraction => out.push(Node::Frac(slot(0), slot(1))),
        TemplateKind::MixedFraction => {
            out.extend(slot(0));
            out.push(Node::Frac(slot(1), slot(2)));
        }
        TemplateKind::Power => out.push(Node::Sup(slot(0))),
        TemplateKind::Pow10 => {
            out.push(text("10"));
            out.push(Node::Sup(slot(0)));
        }
        TemplateKind::ExpE => {
            out.push(text("e"));
            out.push(Node::Sup(slot(0)));
        }
        TemplateKind::Sqrt => out.push(Node::Root { index: None, radicand: slot(0) }),
        TemplateKind::CubeRoot => out.push(Node::Root { index: Some(vec![text("3")]), radicand: slot(0) }),
        TemplateKind::Root => out.push(Node::Root { index: Some(slot(0)), radicand: slot(1) }),
        TemplateKind::LogBase => {
            out.push(text("log"));
            out.push(Node::Sub(slot(0)));
            out.push(text("("));
            out.extend(slot(1));
            out.push(text(")"));
        }
        TemplateKind::Abs => out.push(Node::Abs(slot(0))),
    }
}

/// A result line as a display tree: stacked fractions, radicals and ×10ⁿ.
/// In LineIO (`natural` false) fractions are written `a⌟b`.
pub fn result_nodes(lines: &[ResultLine], natural: bool) -> Vec<Node> {
    let mut out = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if i > 0 {
            out.push(text(",  "));
        }
        if let Some(label) = line.label {
            out.push(text(format!("{label}=")));
        }
        out.extend(formatted_nodes(&line.value, natural));
    }
    out
}

fn over(num: Vec<Node>, den: u128) -> Vec<Node> {
    if den == 1 { num } else { vec![Node::Frac(num, vec![text(den.to_string())])] }
}

pub fn formatted_nodes(f: &Formatted, natural: bool) -> Vec<Node> {
    let mut out = Vec::new();
    match f {
        Formatted::Decimal(d) => {
            let sign = if d.negative { "-" } else { "" };
            out.push(text(format!("{sign}{}", d.mantissa)));
            if let Some(e) = d.exponent {
                out.push(text("×10"));
                out.push(sup(&e.to_string().replace('-', "−")));
            }
        }
        Formatted::Fraction(fr) => {
            if fr.negative {
                out.push(text("−"));
            }
            if !natural {
                if let Some(w) = fr.whole {
                    out.extend([text(w.to_string()), Node::FracMark]);
                }
                out.extend([text(fr.num.to_string()), Node::FracMark, text(fr.den.to_string())]);
                return out;
            }
            if let Some(w) = fr.whole {
                out.push(text(w.to_string()));
            }
            out.extend(over(vec![text(fr.num.to_string())], fr.den));
        }
        Formatted::Surd(s) => {
            if s.terms.is_empty() {
                out.push(text("0"));
            }
            for (i, term) in s.terms.iter().enumerate() {
                match (term.negative, i) {
                    (true, _) => out.push(text("−")),
                    (false, 0) => {}
                    (false, _) => out.push(text("+")),
                }
                out.extend(surd_term(term));
            }
        }
        Formatted::Pi(p) => {
            if p.negative {
                out.push(text("−"));
            }
            let mut num = Vec::new();
            if p.num != 1 {
                num.push(text(p.num.to_string()));
            }
            num.push(text("π"));
            out.extend(over(num, p.den));
        }
        Formatted::Dms(d) => out.push(text(d.to_string())),
    }
    out
}

fn surd_term(t: &SurdTermText) -> Vec<Node> {
    let mut num = Vec::new();
    if t.radicand == 1 || t.num != 1 {
        num.push(text(t.num.to_string()));
    }
    if t.radicand != 1 {
        num.push(Node::Root { index: None, radicand: vec![text(t.radicand.to_string())] });
    }
    over(num, t.den)
}

#[cfg(test)]
mod tests {
    use super::*;
    use calc_core::format::{FormatSettings, ResultForm, format_value};
    use calc_core::input::Editor;
    use calc_core::{Calculator, Key, Value};

    fn t(s: &str) -> Node {
        text(s)
    }

    #[test]
    fn fraction_with_cursor_in_denominator() {
        let mut ed = Editor::new();
        ed.insert_token(Token::Digit(1));
        ed.insert_template(TemplateKind::Fraction);
        let nodes = input_nodes(ed.root(), Some(ed.cursor()));
        assert_eq!(nodes, vec![Node::Frac(vec![t("1")], vec![Node::Cursor, Node::Empty])]);
    }

    #[test]
    fn exponent_after_times_ten_is_raised() {
        let mut ed = Editor::new();
        for tok in [Token::Digit(2), Token::Exp10, Token::Neg, Token::Digit(3)] {
            ed.insert_token(tok);
        }
        let nodes = input_nodes(ed.root(), Some(ed.cursor()));
        assert_eq!(nodes, vec![t("2"), t("×10"), Node::Sup(vec![t("-"), t("3"), Node::Cursor])]);
        ed.insert_token(Token::Plus);
        let nodes = input_nodes(ed.root(), Some(ed.cursor()));
        assert_eq!(nodes, vec![t("2"), t("×10"), Node::Sup(vec![t("-"), t("3")]), t("+"), Node::Cursor]);
        // Just after ×10 the exponent shows as an empty box with the cursor in it.
        let mut ed = Editor::new();
        ed.insert_token(Token::Exp10);
        let nodes = input_nodes(ed.root(), Some(ed.cursor()));
        assert_eq!(nodes, vec![t("×10"), Node::Sup(vec![Node::Cursor, Node::Empty])]);
    }

    #[test]
    fn postfix_tokens_become_superscripts() {
        let mut ed = Editor::new();
        for tok in [Token::Func(Func::Asin), Token::Digit(1), Token::RParen, Token::Square] {
            ed.insert_token(tok);
        }
        let nodes = input_nodes(ed.root(), None);
        assert_eq!(nodes, vec![t("sin"), sup("−1"), t("("), t("1"), t(")"), sup("2")]);
    }

    #[test]
    fn results_are_stacked() {
        let show =
            |v: &Value| formatted_nodes(&format_value(v, &FormatSettings::default(), ResultForm::Standard), true);
        let half_root2 = Value::integer(2).sqrt().unwrap().div(&Value::integer(2)).unwrap();
        let root2 = Node::Root { index: None, radicand: vec![t("2")] };
        assert_eq!(show(&half_root2), vec![Node::Frac(vec![root2], vec![t("2")])]);
        let three_quarters_pi = Value::pi().mul(&Value::integer(-3)).unwrap().div(&Value::integer(4)).unwrap();
        assert_eq!(show(&three_quarters_pi), vec![t("−"), Node::Frac(vec![t("3"), t("π")], vec![t("4")])]);
        assert_eq!(show(&Value::decimal(0.00125).unwrap()), vec![t("1.25"), t("×10"), sup("−3")]);
    }

    #[test]
    fn line_io_fractions_use_the_mark() {
        let f = format_value(
            &Value::rational(calc_core::value::Rational::new(-7, 3).unwrap()),
            &FormatSettings::default(),
            ResultForm::Standard,
        );
        assert_eq!(formatted_nodes(&f, false), vec![t("−"), t("7"), Node::FracMark, t("3")]);
    }

    #[test]
    fn labels_with_accents() {
        assert_eq!(label_nodes("2:x̄"), vec![t("2:"), accent("x", false)]);
        assert_eq!(label_nodes("5:ȳ"), vec![t("5:"), accent("y", false)]);
        assert_eq!(label_nodes("4:x̂"), vec![t("4:"), accent("x", true)]);
        assert_eq!(label_nodes("1:COMP"), vec![t("1:COMP")]);
    }

    #[test]
    fn stat_tokens() {
        let mut ed = Editor::new();
        for tok in [Token::Stat(StatVar::MeanX), Token::Digit(2), Token::EstY] {
            ed.insert_token(tok);
        }
        let nodes = input_nodes(ed.root(), None);
        assert_eq!(nodes, vec![accent("x", false), t("2"), accent("y", true)]);
    }

    #[test]
    fn pol_results_have_labels() {
        let mut calc = Calculator::new();
        for key in [Key::Shift, Key::Plus, Key::Digit(1), Key::Shift, Key::RParen, Key::Digit(1), Key::Equals] {
            calc.press(key);
        }
        let nodes = result_nodes(&calc.result_lines().unwrap(), true);
        assert_eq!(nodes[0], t("r="));
        assert!(nodes.contains(&t("θ=")));
    }
}

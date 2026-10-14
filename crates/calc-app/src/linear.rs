//! A one-line rendering of the input tree, used until the natural display
//! renderer arrives. Exponents are raised, empty slots show as `□`, and
//! multi-item fraction parts are parenthesized.

use calc_core::input::{Cursor, Item, Row, SlotRef, Template, TemplateKind, Token};

use crate::rich::{Script, Span};

/// The input split at the cursor: spans before it and spans after it.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Line {
    pub before: Vec<Span>,
    pub after: Vec<Span>,
}

#[derive(Default)]
struct Writer {
    spans: Vec<Span>,
    cursor_at: Option<usize>,
}

impl Writer {
    fn push(&mut self, text: impl Into<String>, script: Script) {
        self.spans.push(Span::new(text, script));
    }

    fn mark_cursor(&mut self) {
        self.cursor_at = Some(self.spans.len());
    }
}

type CursorRef<'a> = Option<(&'a [SlotRef], usize)>;

pub fn line(row: &Row, cursor: Option<&Cursor>) -> Line {
    let mut w = Writer::default();
    write_row(&mut w, row, Script::Normal, cursor.map(|c| (c.path.as_slice(), c.pos)));
    let split = w.cursor_at.unwrap_or(w.spans.len());
    let after = w.spans.split_off(split);
    Line { before: w.spans, after }
}

fn write_row(w: &mut Writer, row: &Row, script: Script, cursor: CursorRef<'_>) {
    let cursor_here = |i: usize| matches!(cursor, Some(([], pos)) if pos == i);
    for (i, item) in row.items.iter().enumerate() {
        if cursor_here(i) {
            w.mark_cursor();
        }
        match item {
            Item::Token(Token::AngleRad) => w.push("r", Script::Super),
            Item::Token(Token::AngleGrad) => w.push("g", Script::Super),
            Item::Token(t) => w.push(t.to_string(), script),
            Item::Template(t) => {
                // (a⁄b)² rather than a⁄b², which would read as a⁄(b²).
                let powered =
                    matches!(row.items.get(i + 1), Some(Item::Template(next)) if next.kind == TemplateKind::Power);
                let bracket = powered && matches!(t.kind, TemplateKind::Fraction | TemplateKind::MixedFraction);
                if bracket {
                    w.push("(", script);
                }
                write_template(w, t, script, |slot| match cursor {
                    Some(([first, rest @ ..], pos)) if first.item == i && first.slot == slot => Some((rest, pos)),
                    _ => None,
                });
                if bracket {
                    w.push(")", script);
                }
            }
        }
    }
    if cursor_here(row.len()) {
        w.mark_cursor();
    }
}

fn write_slot(w: &mut Writer, row: &Row, script: Script, cursor: CursorRef<'_>, bracket: bool) {
    if row.is_empty() {
        if cursor.is_some() {
            w.mark_cursor();
        }
        w.push("□", script);
        return;
    }
    let plain_number = row.items.iter().all(|item| matches!(item, Item::Token(t) if t.is_numeric()));
    let bracket = bracket && !plain_number && row.len() > 1;
    if bracket {
        w.push("(", script);
    }
    write_row(w, row, script, cursor);
    if bracket {
        w.push(")", script);
    }
}

fn write_template<'a>(w: &mut Writer, t: &Template, script: Script, cursor_in: impl Fn(usize) -> CursorRef<'a>) {
    // Nested scripts stay at one level; deeper nesting would be unreadable.
    let raised = if script == Script::Normal { Script::Super } else { script };
    let lowered = if script == Script::Normal { Script::Sub } else { script };
    let slot =
        |w: &mut Writer, i: usize, s: Script, bracket: bool| write_slot(w, &t.slots[i], s, cursor_in(i), bracket);
    match t.kind {
        TemplateKind::Fraction => {
            slot(w, 0, script, true);
            w.push("⁄", script);
            slot(w, 1, script, true);
        }
        TemplateKind::MixedFraction => {
            slot(w, 0, script, true);
            w.push(" ", script);
            slot(w, 1, script, true);
            w.push("⁄", script);
            slot(w, 2, script, true);
        }
        TemplateKind::Power => slot(w, 0, raised, false),
        TemplateKind::Pow10 => {
            w.push("10", script);
            slot(w, 0, raised, false);
        }
        TemplateKind::ExpE => {
            w.push("e", script);
            slot(w, 0, raised, false);
        }
        TemplateKind::Root => {
            slot(w, 0, raised, false);
            w.push("√(", script);
            slot(w, 1, script, false);
            w.push(")", script);
        }
        TemplateKind::LogBase => {
            w.push("log", script);
            slot(w, 0, lowered, false);
            w.push("(", script);
            slot(w, 1, script, false);
            w.push(")", script);
        }
        kind => {
            let open = match kind {
                TemplateKind::Sqrt => "√(",
                TemplateKind::CubeRoot => "∛(",
                TemplateKind::Abs => "Abs(",
                _ => unreachable!("handled above"),
            };
            w.push(open, script);
            slot(w, 0, script, false);
            w.push(")", script);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use calc_core::input::Editor;

    fn text(spans: &[Span]) -> String {
        spans
            .iter()
            .map(|s| match s.script {
                Script::Normal => s.text.clone(),
                Script::Super => format!("^[{}]", s.text),
                Script::Sub => format!("_[{}]", s.text),
            })
            .collect()
    }

    fn render(ed: &Editor) -> String {
        let l = line(ed.root(), Some(ed.cursor()));
        format!("{}|{}", text(&l.before), text(&l.after))
    }

    #[test]
    fn renders_templates_with_cursor() {
        let mut ed = Editor::new();
        ed.insert_token(Token::Digit(1));
        ed.insert_token(Token::Digit(2));
        ed.insert_template(TemplateKind::Fraction);
        assert_eq!(render(&ed), "12⁄|□");
        ed.insert_token(Token::Digit(5));
        ed.move_right();
        ed.insert_template(TemplateKind::Power);
        ed.insert_token(Token::Digit(2));
        assert_eq!(render(&ed), "(12⁄5)^[2]|");

        let mut ed = Editor::new();
        ed.insert_template(TemplateKind::Root);
        assert_eq!(render(&ed), "|^[□]√(□)");
        ed.move_right();
        assert_eq!(render(&ed), "^[□]√(|□)");
    }

    #[test]
    fn brackets_powered_fractions() {
        let mut ed = Editor::new();
        ed.insert_token(Token::Digit(2));
        ed.insert_template(TemplateKind::Fraction);
        ed.insert_token(Token::Digit(3));
        ed.move_right();
        ed.insert_template(TemplateKind::Power);
        ed.insert_token(Token::Digit(2));
        assert_eq!(render(&ed), "(2⁄3)^[2]|");
    }

    #[test]
    fn brackets_multi_item_fraction_parts() {
        let mut ed = Editor::new();
        ed.insert_template(TemplateKind::Fraction);
        for t in [Token::Digit(1), Token::Plus, Token::Digit(2)] {
            ed.insert_token(t);
        }
        let l = line(ed.root(), None);
        assert_eq!(text(&l.before), "(1+2)⁄□");
        assert!(l.after.is_empty());
    }
}

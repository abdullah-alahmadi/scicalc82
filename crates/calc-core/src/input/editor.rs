//! Cursor editing of the input tree, following the natural display's rules.

use std::fmt;

use super::node::{Item, Row, Template, TemplateKind, Token};

/// Most items an expression may hold (the device allows 99 bytes).
pub const MAX_ITEMS: usize = 99;
/// Deepest template nesting allowed.
pub const MAX_DEPTH: usize = 7;

/// One step into a template: which item of the row, and which of its slots.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SlotRef {
    pub item: usize,
    pub slot: usize,
}

/// A cursor position: the slots to descend through from the root row, then
/// a gap index within that row (0 is before the first item).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Cursor {
    pub path: Vec<SlotRef>,
    pub pos: usize,
}

#[derive(Clone, Debug, Default)]
pub struct Editor {
    root: Row,
    cursor: Cursor,
}

fn row_at<'a>(root: &'a Row, path: &[SlotRef]) -> Option<&'a Row> {
    path.iter().try_fold(root, |row, step| match row.items.get(step.item)? {
        Item::Template(t) => t.slots.get(step.slot),
        Item::Token(_) => None,
    })
}

fn row_at_mut<'a>(root: &'a mut Row, path: &[SlotRef]) -> &'a mut Row {
    path.iter().fold(root, |row, step| match &mut row.items[step.item] {
        Item::Template(t) => &mut t.slots[step.slot],
        Item::Token(_) => unreachable!("cursor paths only pass through templates"),
    })
}

impl Editor {
    pub fn new() -> Self {
        Self::default()
    }

    /// An editor holding `row`, with the cursor at the end (as when
    /// recalling an earlier calculation).
    pub fn from_row(row: Row) -> Self {
        let pos = row.len();
        Self { root: row, cursor: Cursor { path: Vec::new(), pos } }
    }

    pub fn root(&self) -> &Row {
        &self.root
    }

    pub fn cursor(&self) -> &Cursor {
        &self.cursor
    }

    pub fn is_empty(&self) -> bool {
        self.root.is_empty()
    }

    pub fn is_full(&self) -> bool {
        self.root.total_items() >= MAX_ITEMS
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// Moves the cursor, e.g. to the location of a Syntax ERROR.
    /// Returns false, leaving the cursor alone, if `cursor` is not a valid position.
    pub fn set_cursor(&mut self, cursor: Cursor) -> bool {
        let valid = row_at(&self.root, &cursor.path).is_some_and(|row| cursor.pos <= row.len());
        if valid {
            self.cursor = cursor;
        }
        valid
    }

    fn current_row(&self) -> &Row {
        row_at(&self.root, &self.cursor.path).expect("cursor is always valid")
    }

    fn template_containing_cursor(&self) -> Option<(&Template, usize)> {
        let (last, parent) = self.cursor.path.split_last()?;
        match &row_at(&self.root, parent)?.items[last.item] {
            Item::Template(t) => Some((t, last.slot)),
            Item::Token(_) => None,
        }
    }

    pub fn insert_token(&mut self, token: Token) -> bool {
        if self.is_full() {
            return false;
        }
        row_at_mut(&mut self.root, &self.cursor.path).items.insert(self.cursor.pos, Item::Token(token));
        self.cursor.pos += 1;
        true
    }

    /// Inserts an empty template and moves into its first slot. Fractions
    /// typed straight after a number take it as their numerator and move to
    /// the denominator instead.
    pub fn insert_template(&mut self, kind: TemplateKind) -> bool {
        if self.is_full() || self.cursor.path.len() >= MAX_DEPTH {
            return false;
        }
        let row = row_at_mut(&mut self.root, &self.cursor.path);
        let mut pos = self.cursor.pos;
        let mut template = Template::new(kind);
        let mut slot = 0;
        if kind.absorbs_number() {
            let start = row.items[..pos]
                .iter()
                .rposition(|item| !matches!(item, Item::Token(t) if t.is_numeric()))
                .map_or(0, |i| i + 1);
            if start < pos {
                template.slots[0].items = row.items.drain(start..pos).collect();
                pos = start;
                slot = 1;
            }
        }
        row.items.insert(pos, Item::Template(template));
        self.cursor.path.push(SlotRef { item: pos, slot });
        self.cursor.pos = 0;
        true
    }

    /// → : steps over a token, into a template, to the next slot, or out of
    /// the last slot. Wraps from the end of the line to the start.
    pub fn move_right(&mut self) {
        let row = self.current_row();
        if self.cursor.pos < row.len() {
            match &row.items[self.cursor.pos] {
                Item::Template(_) => {
                    self.cursor.path.push(SlotRef { item: self.cursor.pos, slot: 0 });
                    self.cursor.pos = 0;
                }
                Item::Token(_) => self.cursor.pos += 1,
            }
        } else if let Some((template, slot)) = self.template_containing_cursor() {
            if slot + 1 < template.slots.len() {
                self.cursor.path.last_mut().expect("inside a template").slot += 1;
                self.cursor.pos = 0;
            } else {
                let step = self.cursor.path.pop().expect("inside a template");
                self.cursor.pos = step.item + 1;
            }
        } else {
            self.cursor.pos = 0;
        }
    }

    /// ← : the mirror image of [`Self::move_right`]. Wraps from the start of
    /// the line to the end.
    pub fn move_left(&mut self) {
        let row = self.current_row();
        if self.cursor.pos > 0 {
            match &row.items[self.cursor.pos - 1] {
                Item::Template(t) => {
                    let slot = t.slots.len() - 1;
                    let pos = t.slots[slot].len();
                    self.cursor.path.push(SlotRef { item: self.cursor.pos - 1, slot });
                    self.cursor.pos = pos;
                }
                Item::Token(_) => self.cursor.pos -= 1,
            }
        } else if let Some((template, slot)) = self.template_containing_cursor() {
            if slot > 0 {
                let pos = template.slots[slot - 1].len();
                self.cursor.path.last_mut().expect("inside a template").slot -= 1;
                self.cursor.pos = pos;
            } else {
                let step = self.cursor.path.pop().expect("inside a template");
                self.cursor.pos = step.item;
            }
        } else {
            self.cursor.pos = self.root.len();
        }
    }

    /// ▲/▼ : moves between a fraction's numerator and denominator, using the
    /// innermost fraction that has a slot in that direction. Returns false
    /// if there is none, so the caller can use the key for something else.
    pub fn move_vertical(&mut self, down: bool) -> bool {
        for level in (0..self.cursor.path.len()).rev() {
            let step = self.cursor.path[level];
            let Some(Item::Template(t)) =
                row_at(&self.root, &self.cursor.path[..level]).and_then(|row| row.items.get(step.item))
            else {
                continue;
            };
            if let Some(target) = t.kind.vertical_neighbor(step.slot, down) {
                let pos = t.slots[target].len();
                self.cursor.path.truncate(level + 1);
                self.cursor.path[level].slot = target;
                self.cursor.pos = pos;
                return true;
            }
        }
        false
    }

    /// DEL: removes the token left of the cursor. Just after a template it
    /// moves into the template instead; at the start of a later slot it
    /// moves to the previous slot. At the start of a template's first slot,
    /// it removes the template and keeps that slot's contents, but only if
    /// every other slot is empty. Returns false if nothing happened.
    pub fn delete(&mut self) -> bool {
        let pos = self.cursor.pos;
        if pos > 0 {
            match self.current_row().items[pos - 1] {
                Item::Template(_) => self.move_left(),
                Item::Token(_) => {
                    row_at_mut(&mut self.root, &self.cursor.path).items.remove(pos - 1);
                    self.cursor.pos -= 1;
                }
            }
            return true;
        }
        let Some((template, slot)) = self.template_containing_cursor() else {
            return false;
        };
        if slot > 0 {
            self.move_left();
            return true;
        }
        if template.slots[1..].iter().any(|s| !s.is_empty()) {
            return false;
        }
        let (step, parent_path) = self.cursor.path.split_last().expect("inside a template");
        let step = *step;
        let parent = row_at_mut(&mut self.root, parent_path);
        let Item::Template(template) = &mut parent.items[step.item] else {
            unreachable!("cursor paths only pass through templates");
        };
        let contents = std::mem::take(&mut template.slots[0].items);
        parent.items.splice(step.item..=step.item, contents);
        self.cursor.path.pop();
        self.cursor.pos = step.item;
        true
    }
}

// A compact text notation for tests and debugging: each slot is wrapped in
// braces, e.g. `{1}/{2}` or `√{2}`, and `|` marks the cursor.

type CursorRef<'a> = Option<(&'a [SlotRef], usize)>;

fn write_row(f: &mut fmt::Formatter<'_>, row: &Row, cursor: CursorRef<'_>) -> fmt::Result {
    let cursor_here = |i: usize| matches!(cursor, Some(([], pos)) if pos == i);
    for (i, item) in row.items.iter().enumerate() {
        if cursor_here(i) {
            f.write_str("|")?;
        }
        match item {
            Item::Token(t) => write!(f, "{t}")?,
            Item::Template(t) => write_template(f, t, |slot| match cursor {
                Some(([first, rest @ ..], pos)) if first.item == i && first.slot == slot => Some((rest, pos)),
                _ => None,
            })?,
        }
    }
    if cursor_here(row.len()) {
        f.write_str("|")?;
    }
    Ok(())
}

fn write_template<'a>(
    f: &mut fmt::Formatter<'_>,
    t: &Template,
    cursor_in: impl Fn(usize) -> CursorRef<'a>,
) -> fmt::Result {
    let slot = |f: &mut fmt::Formatter<'_>, i: usize| {
        f.write_str("{")?;
        write_row(f, &t.slots[i], cursor_in(i))?;
        f.write_str("}")
    };
    match t.kind {
        TemplateKind::Fraction => {
            slot(f, 0)?;
            f.write_str("/")?;
            slot(f, 1)
        }
        TemplateKind::MixedFraction => {
            slot(f, 0)?;
            slot(f, 1)?;
            f.write_str("/")?;
            slot(f, 2)
        }
        TemplateKind::Root => {
            slot(f, 0)?;
            f.write_str("√")?;
            slot(f, 1)
        }
        TemplateKind::LogBase => {
            f.write_str("log_")?;
            slot(f, 0)?;
            slot(f, 1)
        }
        kind => {
            f.write_str(match kind {
                TemplateKind::Power => "^",
                TemplateKind::Pow10 => "10^",
                TemplateKind::ExpE => "e^",
                TemplateKind::Sqrt => "√",
                TemplateKind::CubeRoot => "∛",
                TemplateKind::Abs => "Abs",
                _ => unreachable!("multi-slot templates handled above"),
            })?;
            slot(f, 0)
        }
    }
}

impl fmt::Display for Row {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_row(f, self, None)
    }
}

/// The input with `|` at the cursor.
impl fmt::Display for Editor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_row(f, &self.root, Some((&self.cursor.path, self.cursor.pos)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::node::Func;
    use TemplateKind::*;

    fn token(c: char) -> Token {
        match c {
            '0'..='9' => Token::Digit(c as u8 - b'0'),
            '.' => Token::Point,
            '+' => Token::Plus,
            '-' => Token::Minus,
            '*' => Token::Times,
            '/' => Token::Divide,
            '(' => Token::LParen,
            ')' => Token::RParen,
            _ => panic!("no test token for {c:?}"),
        }
    }

    fn typed(ed: &mut Editor, s: &str) {
        for c in s.chars() {
            assert!(ed.insert_token(token(c)));
        }
    }

    fn editor(s: &str) -> Editor {
        let mut ed = Editor::new();
        typed(&mut ed, s);
        ed
    }

    #[test]
    fn typing_tokens() {
        let mut ed = editor("12+3");
        assert_eq!(ed.to_string(), "12+3|");
        ed.insert_token(Token::Func(Func::Sin));
        assert_eq!(ed.to_string(), "12+3sin(|");
        assert!(ed.delete());
        assert_eq!(ed.to_string(), "12+3|");
    }

    #[test]
    fn fraction_takes_preceding_number() {
        let mut ed = editor("2+1.5");
        ed.insert_template(Fraction);
        assert_eq!(ed.to_string(), "2+{1.5}/{|}");
        typed(&mut ed, "2");
        ed.move_right();
        typed(&mut ed, "+");
        assert_eq!(ed.to_string(), "2+{1.5}/{2}+|");
    }

    #[test]
    fn fraction_after_operator_starts_empty() {
        let mut ed = editor("2+");
        ed.insert_template(Fraction);
        assert_eq!(ed.to_string(), "2+{|}/{}");
        let mut ed = editor("");
        ed.insert_template(MixedFraction);
        assert_eq!(ed.to_string(), "{|}{}/{}");
    }

    #[test]
    fn power_attaches_to_previous_item() {
        let mut ed = editor("2");
        ed.insert_template(Power);
        typed(&mut ed, "10");
        assert_eq!(ed.to_string(), "2^{10|}");
        ed.move_right();
        assert_eq!(ed.to_string(), "2^{10}|");
    }

    #[test]
    fn left_and_right_walk_through_slots_and_wrap() {
        let mut ed = Editor::new();
        ed.insert_template(Root);
        typed(&mut ed, "3");
        ed.move_right();
        typed(&mut ed, "8");
        assert_eq!(ed.to_string(), "{3}√{8|}");
        let lefts = ["{3}√{|8}", "{3|}√{8}", "{|3}√{8}", "|{3}√{8}", "{3}√{8}|", "{3}√{8|}"];
        for want in lefts {
            ed.move_left();
            assert_eq!(ed.to_string(), want);
        }
        let rights = ["{3}√{8}|", "|{3}√{8}", "{|3}√{8}", "{3|}√{8}", "{3}√{|8}"];
        for want in rights {
            ed.move_right();
            assert_eq!(ed.to_string(), want);
        }
    }

    #[test]
    fn nested_templates() {
        let mut ed = Editor::new();
        ed.insert_template(Sqrt);
        typed(&mut ed, "1");
        ed.insert_template(Fraction);
        typed(&mut ed, "2");
        assert_eq!(ed.to_string(), "√{{1}/{2|}}");
        ed.move_right();
        assert_eq!(ed.to_string(), "√{{1}/{2}|}");
        ed.move_right();
        assert_eq!(ed.to_string(), "√{{1}/{2}}|");
    }

    #[test]
    fn up_and_down_switch_fraction_slots() {
        let mut ed = Editor::new();
        ed.insert_template(Fraction);
        typed(&mut ed, "12");
        assert!(ed.move_vertical(true));
        assert_eq!(ed.to_string(), "{12}/{|}");
        assert!(!ed.move_vertical(true));
        typed(&mut ed, "5");
        assert!(ed.move_vertical(false));
        assert_eq!(ed.to_string(), "{12|}/{5}");
        assert!(!ed.move_vertical(false));
        // From inside a √ nested in the denominator, ▲ goes to the numerator.
        ed.move_vertical(true);
        ed.insert_template(Sqrt);
        assert_eq!(ed.to_string(), "{12}/{5√{|}}");
        assert!(ed.move_vertical(false));
        assert_eq!(ed.to_string(), "{12|}/{5√{}}");
        assert!(!Editor::new().move_vertical(true));
    }

    #[test]
    fn delete_rules() {
        // DEL after a template moves into it, then deletes inside.
        let mut ed = Editor::new();
        ed.insert_template(Sqrt);
        typed(&mut ed, "23");
        ed.move_right();
        ed.delete();
        assert_eq!(ed.to_string(), "√{23|}");
        ed.delete();
        assert_eq!(ed.to_string(), "√{2|}");
        // At the start of the only slot, the template is unwrapped.
        ed.move_left();
        assert!(ed.delete());
        assert_eq!(ed.to_string(), "|2");
        assert!(!ed.delete());

        // At the start of a later slot, DEL moves back a slot.
        let mut ed = editor("1");
        ed.insert_template(Fraction);
        typed(&mut ed, "4");
        ed.move_left();
        ed.delete();
        assert_eq!(ed.to_string(), "{1|}/{4}");
        // A first slot can't be unwrapped while other slots have contents...
        ed.move_left();
        assert!(!ed.delete());
        assert_eq!(ed.to_string(), "{|1}/{4}");
        // ...but an emptied template can be removed.
        let mut ed = editor("7+");
        ed.insert_template(Fraction);
        assert!(ed.delete());
        assert_eq!(ed.to_string(), "7+|");
    }

    #[test]
    fn capacity_and_depth_limits() {
        let mut ed = editor(&"1".repeat(MAX_ITEMS - 1));
        ed.insert_template(Sqrt);
        assert!(ed.is_full());
        assert!(!ed.insert_token(Token::Digit(1)));
        assert!(!ed.insert_template(Sqrt));

        let mut ed = Editor::new();
        for _ in 0..MAX_DEPTH {
            assert!(ed.insert_template(Sqrt));
        }
        assert!(!ed.insert_template(Sqrt));
        assert!(ed.insert_token(Token::Digit(1)));
    }

    #[test]
    fn set_cursor_validates() {
        let mut ed = editor("1");
        ed.insert_template(Fraction);
        let inside = Cursor { path: vec![SlotRef { item: 0, slot: 0 }], pos: 1 };
        assert!(ed.set_cursor(inside.clone()));
        assert_eq!(ed.to_string(), "{1|}/{}");
        assert!(!ed.set_cursor(Cursor { path: vec![SlotRef { item: 0, slot: 2 }], pos: 0 }));
        assert!(!ed.set_cursor(Cursor { path: Vec::new(), pos: 5 }));
        assert_eq!(ed.cursor(), &inside);
    }

    #[derive(Clone, Debug)]
    enum Op {
        Token(char),
        Template(TemplateKind),
        Left,
        Right,
        Up,
        Down,
        Delete,
    }

    fn op() -> impl proptest::strategy::Strategy<Value = Op> {
        use proptest::prelude::*;
        let kinds = [Fraction, MixedFraction, Power, Pow10, ExpE, Sqrt, CubeRoot, Root, LogBase, Abs];
        prop_oneof![
            3 => proptest::char::ranges(vec!['0'..='9', '+'..='+'].into()).prop_map(Op::Token),
            2 => proptest::sample::select(kinds.to_vec()).prop_map(Op::Template),
            2 => Just(Op::Left),
            2 => Just(Op::Right),
            1 => Just(Op::Up),
            1 => Just(Op::Down),
            2 => Just(Op::Delete),
        ]
    }

    fn depth(row: &Row) -> usize {
        row.items
            .iter()
            .map(|item| match item {
                Item::Template(t) => 1 + t.slots.iter().map(depth).max().unwrap_or(0),
                Item::Token(_) => 0,
            })
            .max()
            .unwrap_or(0)
    }

    proptest::proptest! {
        #[test]
        fn random_editing_keeps_invariants(ops in proptest::collection::vec(op(), 0..200)) {
            let mut ed = Editor::new();
            for op in ops {
                match op {
                    Op::Token(c) => { ed.insert_token(token(c)); }
                    Op::Template(kind) => { ed.insert_template(kind); }
                    Op::Left => ed.move_left(),
                    Op::Right => ed.move_right(),
                    Op::Up => { ed.move_vertical(false); }
                    Op::Down => { ed.move_vertical(true); }
                    Op::Delete => { ed.delete(); }
                }
                let cursor = ed.cursor().clone();
                proptest::prop_assert!(ed.clone().set_cursor(cursor.clone()), "invalid cursor in {}", ed);
                proptest::prop_assert!(ed.root().total_items() <= MAX_ITEMS);
                proptest::prop_assert!(depth(ed.root()) <= MAX_DEPTH);
                // ← and → undo each other from every position.
                let mut probe = ed.clone();
                probe.move_left();
                probe.move_right();
                proptest::prop_assert_eq!(probe.cursor(), &cursor, "← then → in {}", ed);
                probe.move_right();
                probe.move_left();
                proptest::prop_assert_eq!(probe.cursor(), &cursor, "→ then ← in {}", ed);
            }
        }
    }

    #[test]
    fn recalled_rows_put_cursor_at_end() {
        let mut ed = editor("9");
        ed.insert_template(Fraction);
        typed(&mut ed, "4");
        let recalled = Editor::from_row(ed.root().clone());
        assert_eq!(recalled.to_string(), "{9}/{4}|");
        assert_eq!(ed.root().to_string(), "{9}/{4}");
    }
}

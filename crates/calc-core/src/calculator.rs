//! The top-level state machine: key presses in, display state out.

use crate::ast::LocatedError;
use crate::error::CalcError;
use crate::eval::{Env, Outcome, evaluate};
use crate::format::{Formatted, ResultForm, format_eng, format_value};
use crate::input::{Cursor, Editor, Item, Row, TemplateKind, Token, Var};
use crate::key::{Action, Key, Modifier, action, hyperbolic, var_for_key};
use crate::parse::parse;
use crate::state::{Memory, Rng, Settings};
use crate::value::Value;

/// Calculations kept for ▲/▼ replay.
pub const MAX_HISTORY: usize = 30;

/// A key that is waiting for a follow-up key.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Pending {
    #[default]
    None,
    /// hyp: waiting for sin, cos or tan.
    Hyp,
    /// STO: waiting for a variable key.
    Sto,
    /// RCL: waiting for a variable key.
    Rcl,
    /// DRG▸ menu: 1 °, 2 ʳ, 3 ᵍ
    DrgMenu,
    /// CLR menu: 1 Setup, 2 Memory, 3 All
    ClrMenu,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResultView {
    pub outcome: Outcome,
    pub form: ResultForm,
    /// Engineering-notation step, once ENG has been pressed.
    pub eng: Option<i32>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub enum Phase {
    #[default]
    Editing,
    Result(ResultView),
    Error(LocatedError),
}

/// One formatted line of a result, e.g. `r=` and its value for Pol(.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResultLine {
    pub label: Option<&'static str>,
    pub value: Formatted,
}

#[derive(Clone, Debug)]
struct HistoryEntry {
    row: Row,
    outcome: Outcome,
}

#[derive(Clone, Debug, Default)]
pub struct Calculator {
    editor: Editor,
    phase: Phase,
    modifier: Modifier,
    pending: Pending,
    settings: Settings,
    memory: Memory,
    history: Vec<HistoryEntry>,
    /// Which history entry ▲/▼ is showing.
    replay: Option<usize>,
    rng: Rng,
}

impl Calculator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reseed(&mut self, seed: u64) {
        self.rng = Rng::seeded(seed);
    }

    pub fn editor(&self) -> &Editor {
        &self.editor
    }

    pub fn phase(&self) -> &Phase {
        &self.phase
    }

    pub fn modifier(&self) -> Modifier {
        self.modifier
    }

    pub fn pending(&self) -> Pending {
        self.pending
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    pub fn settings_mut(&mut self) -> &mut Settings {
        &mut self.settings
    }

    pub fn memory(&self) -> &Memory {
        &self.memory
    }

    pub fn error(&self) -> Option<CalcError> {
        match &self.phase {
            Phase::Error(e) => Some(e.error),
            _ => None,
        }
    }

    pub fn result_lines(&self) -> Option<Vec<ResultLine>> {
        let Phase::Result(view) = &self.phase else { return None };
        let show = |v: &Value| match view.eng {
            Some(step) => Formatted::Decimal(format_eng(v.to_f64(), step)),
            None => format_value(v, &self.settings.format, view.form),
        };
        let line = |label, v| ResultLine { label, value: show(v) };
        Some(match &view.outcome {
            Outcome::Single(v) => vec![line(None, v)],
            Outcome::Polar { r, theta } => vec![line(Some("r"), r), line(Some("θ"), theta)],
            Outcome::Rect { x, y } => vec![line(Some("X"), x), line(Some("Y"), y)],
        })
    }

    /// The result as one line of text, e.g. `1/2` or `r=√2, θ=45`.
    pub fn result_text(&self) -> Option<String> {
        let lines = self.result_lines()?;
        let text = lines.iter().map(|l| match l.label {
            Some(label) => format!("{label}={}", l.value),
            None => l.value.to_string(),
        });
        Some(text.collect::<Vec<_>>().join(", "))
    }

    pub fn press_all(&mut self, keys: &[Key]) {
        for &key in keys {
            self.press(key);
        }
    }

    pub fn press(&mut self, key: Key) {
        match key {
            Key::Shift | Key::Alpha => {
                let pressed = if key == Key::Shift { Modifier::Shift } else { Modifier::Alpha };
                self.modifier = if self.modifier == pressed { Modifier::None } else { pressed };
                return;
            }
            _ => {}
        }
        let modifier = std::mem::take(&mut self.modifier);

        // On an error screen only AC and ←/→ (back to the error position) work.
        if let Phase::Error(err) = &self.phase {
            match key {
                Key::Ac | Key::On => self.clear_entry(),
                Key::Left | Key::Right => {
                    let at = err.at.clone();
                    self.editor.set_cursor(at);
                    self.phase = Phase::Editing;
                }
                _ => {}
            }
            return;
        }

        match std::mem::take(&mut self.pending) {
            Pending::Hyp => {
                if let Some(f) = hyperbolic(key, modifier) {
                    return self.insert_token(Token::Func(f));
                }
            }
            Pending::Sto => {
                if let Some(var) = var_for_key(key) {
                    return self.store(var);
                }
            }
            Pending::Rcl => {
                if let Some(var) = var_for_key(key) {
                    return self.recall(var);
                }
            }
            Pending::DrgMenu => {
                let unit = match key {
                    Key::Digit(1) => Some(Token::AngleDeg),
                    Key::Digit(2) => Some(Token::AngleRad),
                    Key::Digit(3) => Some(Token::AngleGrad),
                    _ => None,
                };
                if let Some(token) = unit {
                    return self.insert_token(token);
                }
            }
            Pending::ClrMenu => {
                if let Key::Digit(choice @ 1..=3) = key {
                    return self.clear_memory(choice);
                }
            }
            Pending::None => {}
        }
        self.perform(action(key, modifier));
    }

    fn perform(&mut self, action: Action) {
        match action {
            Action::Insert(token) => self.insert_token(token),
            Action::Template(kind) => self.insert_template(kind),
            Action::Left => {
                if !self.leave_result() {
                    self.editor.move_left();
                }
            }
            Action::Right => {
                if self.leave_result() {
                    self.editor.set_cursor(Cursor::default());
                } else {
                    self.editor.move_right();
                }
            }
            Action::Up => self.vertical(false),
            Action::Down => self.vertical(true),
            Action::Del => {
                if !self.leave_result() {
                    self.editor.delete();
                }
            }
            Action::Ac => self.clear_entry(),
            Action::Equals => {
                self.execute(ResultForm::Standard);
            }
            Action::ApproxEquals => {
                self.execute(ResultForm::Decimal);
            }
            Action::SToD => {
                if let Phase::Result(view) = &mut self.phase {
                    view.form = match view.form {
                        ResultForm::Standard => ResultForm::Decimal,
                        ResultForm::Decimal => ResultForm::Standard,
                    };
                    view.eng = None;
                }
            }
            Action::Eng(step) => {
                if let Phase::Result(view) = &mut self.phase {
                    // The first press shows plain engineering form (ENG) or one step down (SHIFT ENG).
                    view.eng = Some(match view.eng {
                        None => step.min(0),
                        Some(s) => s + step,
                    });
                }
            }
            Action::MemoryAdd { subtract } => self.memory_add(subtract),
            Action::Sto => self.pending = Pending::Sto,
            Action::Rcl => self.pending = Pending::Rcl,
            Action::Hyp => self.pending = Pending::Hyp,
            Action::DrgMenu => self.pending = Pending::DrgMenu,
            Action::ClrMenu => self.pending = Pending::ClrMenu,
            // MODE and SETUP menus arrive with the other modes.
            Action::Mode | Action::Setup | Action::Nothing => {}
        }
    }

    /// Switches from showing a result to editing its expression, cursor at
    /// the end. Returns false if a result wasn't showing.
    fn leave_result(&mut self) -> bool {
        if !matches!(self.phase, Phase::Result(_)) {
            return false;
        }
        self.editor = Editor::from_row(self.editor.root().clone());
        self.phase = Phase::Editing;
        self.replay = None;
        true
    }

    /// Typing after a result starts a new calculation; operators that need
    /// a left operand continue from Ans.
    fn start_new_if_result(&mut self, continues_ans: bool) {
        if matches!(self.phase, Phase::Result(_)) {
            self.editor.clear();
            self.phase = Phase::Editing;
            if continues_ans {
                self.editor.insert_token(Token::Ans);
            }
        }
        self.replay = None;
    }

    fn insert_token(&mut self, token: Token) {
        let continues_ans = matches!(
            token,
            Token::Plus
                | Token::Minus
                | Token::Times
                | Token::Divide
                | Token::Perm
                | Token::Comb
                | Token::Square
                | Token::Cube
                | Token::Recip
                | Token::Factorial
                | Token::Percent
                | Token::AngleDeg
                | Token::AngleRad
                | Token::AngleGrad
        );
        self.start_new_if_result(continues_ans);
        self.editor.insert_token(token);
    }

    fn insert_template(&mut self, kind: TemplateKind) {
        self.start_new_if_result(kind == TemplateKind::Power);
        self.editor.insert_template(kind);
    }

    fn clear_entry(&mut self) {
        self.editor.clear();
        self.phase = Phase::Editing;
        self.pending = Pending::None;
        self.replay = None;
    }

    fn clear_memory(&mut self, choice: u8) {
        if choice != 2 {
            self.settings = Settings::default();
        }
        if choice != 1 {
            self.memory = Memory::default();
        }
        if choice == 3 {
            self.history.clear();
        }
        self.clear_entry();
    }

    /// ▲/▼: switch fraction slots while editing, otherwise replay history.
    fn vertical(&mut self, down: bool) {
        let editing = matches!(self.phase, Phase::Editing);
        if editing && self.editor.move_vertical(down) {
            return;
        }
        if editing && !self.editor.is_empty() && self.replay.is_none() {
            return;
        }
        let len = self.history.len();
        let index = match (self.replay, down) {
            // A result on screen is the newest entry, so ▲ starts one before it.
            (None, false) if matches!(self.phase, Phase::Result(_)) => len.checked_sub(2),
            (None, false) => len.checked_sub(1),
            (Some(i), false) => i.checked_sub(1),
            (Some(i), true) => Some(i + 1).filter(|&i| i < len),
            (None, true) => None,
        };
        if let Some(i) = index {
            let entry = &self.history[i];
            self.editor = Editor::from_row(entry.row.clone());
            self.phase =
                Phase::Result(ResultView { outcome: entry.outcome.clone(), form: ResultForm::Standard, eng: None });
            self.replay = Some(i);
        }
    }

    /// Evaluates the input. Returns the outcome, or None after an error or
    /// on empty input.
    fn execute(&mut self, form: ResultForm) -> Option<Outcome> {
        if self.editor.is_empty() {
            return None;
        }
        let row = self.editor.root().clone();
        let mut env = Env { settings: &self.settings, memory: &self.memory, rng: &mut self.rng };
        match parse(&row).and_then(|expr| evaluate(&expr, &mut env)) {
            Ok(outcome) => {
                self.memory.ans = outcome.primary().clone();
                if self.history.len() == MAX_HISTORY {
                    self.history.remove(0);
                }
                self.history.push(HistoryEntry { row, outcome: outcome.clone() });
                self.phase = Phase::Result(ResultView { outcome: outcome.clone(), form, eng: None });
                self.replay = None;
                Some(outcome)
            }
            Err(e) => {
                self.phase = Phase::Error(e);
                None
            }
        }
    }

    /// The value STO and M+ act on: the result on screen, or the input
    /// evaluated now, or Ans for an empty input.
    fn current_value(&mut self) -> Option<Value> {
        match &self.phase {
            Phase::Result(view) => Some(view.outcome.primary().clone()),
            _ if self.editor.is_empty() => Some(self.memory.ans.clone()),
            _ => self.execute(ResultForm::Standard).map(|o| o.primary().clone()),
        }
    }

    fn store(&mut self, var: Var) {
        if let Some(value) = self.current_value() {
            self.memory.set(var, value);
        }
    }

    fn recall(&mut self, var: Var) {
        self.editor = Editor::from_row(Row::from(vec![Item::Token(Token::Var(var))]));
        let value = self.memory.get(var).clone();
        self.phase =
            Phase::Result(ResultView { outcome: Outcome::Single(value), form: ResultForm::Standard, eng: None });
        self.replay = None;
    }

    fn memory_add(&mut self, subtract: bool) {
        let Some(value) = self.current_value() else { return };
        let m = self.memory.get(Var::M);
        let updated = if subtract { m.sub(&value) } else { m.add(&value) };
        match updated {
            Ok(v) => self.memory.set(Var::M, v),
            Err(error) => self.phase = Phase::Error(LocatedError { error, at: self.editor.cursor().clone() }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::{FractionStyle, NumberFormat};
    use crate::state::AngleUnit;

    /// Presses keys described by space-separated words; runs of digits are
    /// typed one by one, and `S`/`A` are SHIFT/ALPHA.
    fn keys(calc: &mut Calculator, script: &str) {
        for word in script.split_whitespace() {
            if word.bytes().all(|b| b.is_ascii_digit()) {
                for b in word.bytes() {
                    calc.press(Key::Digit(b - b'0'));
                }
                continue;
            }
            let key = match word {
                "S" => Key::Shift,
                "A" => Key::Alpha,
                "+" => Key::Plus,
                "-" => Key::Minus,
                "*" => Key::Times,
                "/" => Key::Divide,
                "(" => Key::LParen,
                ")" => Key::RParen,
                "." => Key::Point,
                "=" => Key::Equals,
                "sin" => Key::Sin,
                "cos" => Key::Cos,
                "tan" => Key::Tan,
                "frac" => Key::Fraction,
                "sqrt" => Key::Sqrt,
                "sq" => Key::Square,
                "pow" => Key::Power,
                "log" => Key::Log,
                "logab" => Key::LogBase,
                "ln" => Key::Ln,
                "neg" => Key::Neg,
                "dms" => Key::Dms,
                "hyp" => Key::Hyp,
                "rcl" => Key::Rcl,
                "eng" => Key::Eng,
                "sd" => Key::SToD,
                "m+" => Key::MPlus,
                "del" => Key::Del,
                "ac" => Key::Ac,
                "ans" => Key::Ans,
                "exp" => Key::Exp10,
                "recip" => Key::Recip,
                "left" => Key::Left,
                "right" => Key::Right,
                "up" => Key::Up,
                "down" => Key::Down,
                _ => panic!("unknown key word {word:?}"),
            };
            calc.press(key);
        }
    }

    fn run(script: &str) -> Calculator {
        let mut calc = Calculator::new();
        keys(&mut calc, script);
        calc
    }

    fn result(script: &str) -> String {
        let calc = run(script);
        calc.result_text().unwrap_or_else(|| panic!("no result for {script:?}: {:?}", calc.phase()))
    }

    fn result_in(unit: AngleUnit, script: &str) -> String {
        let mut calc = Calculator::new();
        calc.settings_mut().angle = unit;
        keys(&mut calc, script);
        calc.result_text().unwrap_or_else(|| panic!("no result for {script:?}: {:?}", calc.phase()))
    }

    fn error(script: &str) -> CalcError {
        run(script).error().unwrap_or_else(|| panic!("no error for {script:?}"))
    }

    #[test]
    fn arithmetic_and_priority() {
        assert_eq!(result("1 + 2 * 3 ="), "7");
        assert_eq!(result("2 / 3 ="), "2/3");
        assert_eq!(result("1 frac 3 right + 1 frac 6 ="), "1/2");
        // Omitted multiplication binds tighter than ÷.
        assert_eq!(result("1 / 2 S exp ="), "0.1591549431");
        assert_eq!(result("2 S exp ="), "2π");
        assert_eq!(result("2 ( 3 ) ="), "6");
        assert_eq!(result("2 sin 30 ) ="), "1");
        // (−) binds looser than x².
        assert_eq!(result("neg 2 sq ="), "-4");
        assert_eq!(result("( neg 2 ) sq ="), "4");
        assert_eq!(result("2 * - 3 ="), "-6");
    }

    #[test]
    fn powers_roots_and_logs() {
        assert_eq!(result("2 pow 10 ="), "1024");
        assert_eq!(result("2 pow neg 1 ="), "1/2");
        assert_eq!(result("2 pow 3 right sq ="), "64");
        assert_eq!(result("sqrt 8 ="), "2√2");
        assert_eq!(result("sqrt 2 right * sqrt 3 ="), "√6");
        assert_eq!(result("S pow 3 right 8 ="), "2");
        assert_eq!(result("S sqrt 27 ="), "3");
        assert_eq!(result("log 1000 ="), "3");
        assert_eq!(result("logab 2 right 8 ="), "3");
        assert_eq!(result("S log 3 ="), "1000");
        assert_eq!(result("ln 1 ="), "0");
        assert_eq!(result("1 exp 3 ="), "1000");
        assert_eq!(result("1 exp neg 3 ="), "1/1000");
        assert_eq!(error("sqrt neg 4 ="), CalcError::Math);
        assert_eq!(error("log 0 ="), CalcError::Math);
    }

    #[test]
    fn trigonometry() {
        assert_eq!(result("sin 30 ="), "1/2");
        assert_eq!(result("cos 45 ="), "√2/2");
        assert_eq!(result("tan 60 ="), "√3");
        assert_eq!(error("tan 90 ="), CalcError::Math);
        assert_eq!(result("S sin . 5 ="), "30");
        assert_eq!(result_in(AngleUnit::Rad, "sin frac S exp down 6 ="), "1/2");
        assert_eq!(result_in(AngleUnit::Rad, "S sin . 5 ="), "π/6");
        assert_eq!(result_in(AngleUnit::Gra, "cos 100 ="), "0");
        assert_eq!(result("sin 1 ="), "0.01745240644");
        assert_eq!(result("hyp sin 0 ="), "0");
        assert_eq!(result("hyp S cos 1 ="), "0");
        // DRG▸: 90° in radian mode.
        assert_eq!(result_in(AngleUnit::Rad, "90 S ans 1 ="), "π/2");
        // 1°30′ = 1.5°
        assert_eq!(result("1 dms 30 dms ="), "3/2");
        assert_eq!(result("1 dms 30 dms 36 dms ="), "151/100");
    }

    #[test]
    fn combinatorics() {
        assert_eq!(result("5 S recip ="), "120");
        assert_eq!(result("5 S * 2 ="), "20");
        assert_eq!(result("5 S / 2 ="), "10");
        assert_eq!(result("69 S recip ="), "1.711224524×10^98");
        assert_eq!(error("70 S recip ="), CalcError::Math);
        assert_eq!(error("3 . 5 S recip ="), CalcError::Math);
        assert_eq!(error("2 S * 5 ="), CalcError::Math);
    }

    #[test]
    fn polar_and_rectangular() {
        assert_eq!(result("S + 1 S ) 1 ="), "r=√2, θ=45");
        assert_eq!(result("S + neg 1 S ) 0 ="), "r=1, θ=180");
        assert_eq!(result("S - 2 S ) 30 ="), "X=√3, Y=1");
        assert_eq!(error("1 + S + 1 S ) 1 ="), CalcError::Syntax);
    }

    #[test]
    fn ans_and_repeated_equals() {
        let mut calc = run("3 = + 2 =");
        assert_eq!(calc.result_text().unwrap(), "5");
        assert_eq!(calc.editor().root().to_string(), "Ans+2");
        keys(&mut calc, "=");
        assert_eq!(calc.result_text().unwrap(), "7");
        assert_eq!(result("3 = sq ="), "9");
        assert_eq!(result("4 = pow 3 ="), "64");
        // A digit after a result starts fresh.
        assert_eq!(result("3 = 4 ="), "4");
    }

    #[test]
    fn variables_and_memory() {
        assert_eq!(result("5 S rcl neg A neg * 2 ="), "10");
        assert_eq!(result("3 m+ 4 m+ 1 S m+ A m+ ="), "6");
        let calc = run("7 S rcl sin ac rcl sin");
        assert_eq!(calc.result_text().unwrap(), "7");
        assert_eq!(calc.editor().root().to_string(), "D");
        let calc = run("5 S rcl neg S 9 2");
        assert_eq!(calc.memory().get(Var::A), &Value::zero());
    }

    #[test]
    fn errors_jump_to_their_position() {
        let mut calc = run("1 / 0 =");
        assert_eq!(calc.error(), Some(CalcError::Math));
        keys(&mut calc, "5"); // ignored on the error screen
        keys(&mut calc, "left");
        assert_eq!(calc.editor().to_string(), "1|÷0");
        let mut calc = run("1 + =");
        assert_eq!(calc.error(), Some(CalcError::Syntax));
        keys(&mut calc, "right");
        assert_eq!(calc.editor().to_string(), "1+|");
        keys(&mut calc, "ac");
        assert!(calc.editor().is_empty());
        let mut calc = Calculator::new();
        for _ in 0..30 {
            calc.press(Key::LParen);
        }
        keys(&mut calc, "1 =");
        assert_eq!(calc.error(), Some(CalcError::Stack));
    }

    #[test]
    fn editing_after_a_result() {
        assert_eq!(result("1 + 2 = left del 5 ="), "6");
        let calc = run("1 + 2 = right");
        assert_eq!(calc.editor().to_string(), "|1+2");
    }

    #[test]
    fn history_replay() {
        let mut calc = run("1 + 1 = 2 * 3 = up");
        assert_eq!(calc.editor().root().to_string(), "1+1");
        assert_eq!(calc.result_text().unwrap(), "2");
        keys(&mut calc, "down");
        assert_eq!(calc.result_text().unwrap(), "6");
        keys(&mut calc, "ac up");
        assert_eq!(calc.editor().root().to_string(), "2×3");
    }

    #[test]
    fn display_toggles() {
        let mut calc = run("2 / 3 =");
        keys(&mut calc, "sd");
        assert_eq!(calc.result_text().unwrap(), "0.6666666667");
        keys(&mut calc, "sd");
        assert_eq!(calc.result_text().unwrap(), "2/3");
        assert_eq!(result("2 / 3 S ="), "0.6666666667");

        let mut calc = run("1234 = eng");
        assert_eq!(calc.result_text().unwrap(), "1.234×10^3");
        keys(&mut calc, "eng");
        assert_eq!(calc.result_text().unwrap(), "0.001234×10^6");
        assert_eq!(result("1234 = S eng"), "1234×10^0");

        let mut calc = Calculator::new();
        calc.settings_mut().format.fraction = FractionStyle::Mixed;
        keys(&mut calc, "S frac 1 right 1 right 2 =");
        assert_eq!(calc.result_text().unwrap(), "1 1/2");
    }

    #[test]
    fn random_and_rounding() {
        let mut calc = Calculator::new();
        calc.reseed(42);
        for _ in 0..20 {
            keys(&mut calc, "S . =");
            let r = calc.memory().ans.as_rational().expect("Ran# is exact");
            assert!(r.signum() >= 0 && r.to_f64() < 1.0 && 1000 % r.denom() == 0);
            keys(&mut calc, "A . 1 S ) 6 =");
            let n = calc.memory().ans.as_rational().unwrap();
            assert!(n.is_integer() && (1..=6).contains(&n.numer()));
        }
        let mut calc = Calculator::new();
        calc.settings_mut().format.number = NumberFormat::Fix(2);
        keys(&mut calc, "S 0 1 / 3 ) * 3 =");
        assert_eq!(calc.result_text().unwrap(), "0.99");
    }
}

//! The top-level state machine: key presses in, display state out.

use serde::{Deserialize, Serialize};

use crate::ast::LocatedError;
use crate::error::{CalcError, CalcResult};
use crate::eval::{Env, Outcome, evaluate};
use crate::format::{Formatted, ResultForm, format_dms, format_eng, format_value};
use crate::input::{Cursor, Editor, Func, Item, Row, TemplateKind, Token, Var};
use crate::key::{Action, Key, Modifier, action, hyperbolic, var_for_key};
use crate::menu::{self, MenuAction, MenuId, MenuView};
use crate::parse::parse;
use crate::state::{CalcMode, Memory, Rng, Settings};
use crate::stats::StatData;
use crate::table::{Table, TableRow, TableStage, x_values};
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
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResultView {
    pub outcome: Outcome,
    pub form: ResultForm,
    /// Engineering-notation step, once ENG has been pressed.
    pub eng: Option<i32>,
    /// Shown as degrees, minutes and seconds (the °′″ key).
    pub dms: bool,
}

impl ResultView {
    fn new(outcome: Outcome, form: ResultForm) -> Self {
        Self { outcome, form, eng: None, dms: false }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub enum Phase {
    #[default]
    Editing,
    Result(ResultView),
    Error(LocatedError),
}

/// Which screen keys and the display belong to. Menus open on top of these.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Screen {
    /// Calculations: COMP mode, and STAT mode outside the data editor.
    #[default]
    Calc,
    StatEditor,
    Table,
}

/// The STAT editor's highlighted cell and the value being typed into it.
#[derive(Clone, Debug, Default)]
pub struct StatEditor {
    pub row: usize,
    pub column: usize,
    pub input: Editor,
    /// An error from the last entry, shown until the next key.
    pub error: Option<CalcError>,
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
    mode: CalcMode,
    screen: Screen,
    menu: Option<MenuId>,
    stats: StatData,
    stat_editor: StatEditor,
    table: Table,
}

/// What survives switching the calculator off and on: setup, memory, mode,
/// STAT data and the TABLE setup (but not the input, result or history).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub settings: Settings,
    pub memory: Memory,
    pub mode: CalcMode,
    pub stats: StatData,
    pub table_function: Row,
    pub table_start: Value,
    pub table_end: Value,
    pub table_step: Value,
}

/// The tokens a template key types in LineIO, which has no templates.
fn linear_tokens(kind: TemplateKind) -> &'static [Token] {
    match kind {
        TemplateKind::Fraction | TemplateKind::MixedFraction => &[Token::FracBar],
        TemplateKind::Power => &[Token::PowOpen],
        TemplateKind::Pow10 => &[Token::Digit(1), Token::Digit(0), Token::PowOpen],
        TemplateKind::ExpE => &[Token::Euler, Token::PowOpen],
        TemplateKind::Sqrt => &[Token::Func(Func::Sqrt)],
        TemplateKind::CubeRoot => &[Token::Func(Func::Cbrt)],
        TemplateKind::Root => &[Token::RootOpen],
        TemplateKind::LogBase => &[Token::Func(Func::Log)],
        TemplateKind::Abs => &[Token::Func(Func::Abs)],
    }
}

/// Plain typing into an input line: the STAT editor's cells and the TABLE prompts.
fn edit(editor: &mut Editor, action: Action, natural: bool) {
    match action {
        Action::Insert(token) => {
            editor.insert_token(token);
        }
        Action::Template(kind) if natural => {
            editor.insert_template(kind);
        }
        Action::Template(kind) => {
            for &token in linear_tokens(kind) {
                editor.insert_token(token);
            }
        }
        Action::Left => editor.move_left(),
        Action::Right => editor.move_right(),
        Action::Up => {
            editor.move_vertical(false);
        }
        Action::Down => {
            editor.move_vertical(true);
        }
        Action::Del => {
            editor.delete();
        }
        _ => {}
    }
}

impl Calculator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reseed(&mut self, seed: u64) {
        self.rng = Rng::seeded(seed);
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            settings: self.settings,
            memory: self.memory.clone(),
            mode: self.mode,
            stats: self.stats.clone(),
            table_function: self.table.function.clone(),
            table_start: self.table.start.clone(),
            table_end: self.table.end.clone(),
            table_step: self.table.step.clone(),
        }
    }

    /// A calculator switched on with `snapshot`'s state, showing a blank
    /// calculation screen (or f(X)= in TABLE mode).
    pub fn restore(snapshot: Snapshot) -> Self {
        let mut calc = Self { settings: snapshot.settings, memory: snapshot.memory, ..Self::default() };
        calc.stats = snapshot.stats;
        calc.stats.freq = calc.settings.stat_freq;
        calc.table.function = snapshot.table_function;
        calc.table.start = snapshot.table_start;
        calc.table.end = snapshot.table_end;
        calc.table.step = snapshot.table_step;
        calc.set_mode(snapshot.mode);
        // STAT mode starts on the calculation screen; SHIFT 1 2 opens the data.
        if snapshot.mode == CalcMode::Stat {
            calc.screen = Screen::Calc;
        }
        calc
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

    pub fn mode(&self) -> CalcMode {
        self.mode
    }

    pub fn screen(&self) -> Screen {
        self.screen
    }

    /// The open menu, if any.
    pub fn menu(&self) -> Option<MenuView> {
        self.menu.map(|id| menu::view(id, self.stats.kind))
    }

    pub fn stats(&self) -> &StatData {
        &self.stats
    }

    pub fn stat_editor(&self) -> &StatEditor {
        &self.stat_editor
    }

    pub fn table(&self) -> &Table {
        &self.table
    }

    /// A value in the current display format, as STAT and TABLE cells show it.
    pub fn format(&self, value: &Value) -> Formatted {
        format_value(value, &self.settings.format, ResultForm::Standard)
    }

    /// How many calculations ▲ can replay.
    pub fn history_len(&self) -> usize {
        self.history.len()
    }

    pub fn error(&self) -> Option<CalcError> {
        match &self.phase {
            Phase::Error(e) => Some(e.error),
            _ => None,
        }
    }

    pub fn result_lines(&self) -> Option<Vec<ResultLine>> {
        let Phase::Result(view) = &self.phase else { return None };
        let show = |v: &Value| match (view.eng, view.dms.then(|| format_dms(v.to_f64())).flatten()) {
            (_, Some(dms)) => Formatted::Dms(dms),
            (Some(step), None) => Formatted::Decimal(format_eng(v.to_f64(), step)),
            (None, None) => format_value(v, &self.settings.format, view.form),
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
        if let Key::Shift | Key::Alpha = key {
            let pressed = if key == Key::Shift { Modifier::Shift } else { Modifier::Alpha };
            self.modifier = if self.modifier == pressed { Modifier::None } else { pressed };
            return;
        }
        let modifier = std::mem::take(&mut self.modifier);
        let action = action(key, modifier);

        // These menus open from any screen, replacing any open menu.
        match action {
            Action::Mode => return self.open_menu(MenuId::Mode),
            Action::Setup => return self.open_menu(MenuId::Setup),
            Action::ClrMenu => return self.open_menu(MenuId::Clr),
            Action::DrgMenu => return self.open_menu(MenuId::Drg),
            _ => {}
        }
        if let Some(menu) = self.menu {
            return self.menu_key(menu, key);
        }
        match self.screen {
            Screen::Calc => self.calc_key(key, modifier, action),
            Screen::StatEditor => self.stat_editor_key(action),
            Screen::Table => self.table_key(action),
        }
    }

    // ---- Menus ----

    fn open_menu(&mut self, id: MenuId) {
        self.menu = Some(id);
        self.pending = Pending::None;
    }

    fn menu_key(&mut self, menu: MenuId, key: Key) {
        match key {
            Key::Ac | Key::On => self.menu = None,
            Key::Up | Key::Down => {
                if let Some(next) = menu::page(menu, key == Key::Down) {
                    self.menu = Some(next);
                }
            }
            Key::Digit(d) => {
                if let Some(choice) = menu::select(menu, d, self.stats.kind) {
                    self.menu = None;
                    self.menu_action(choice);
                }
            }
            _ => {}
        }
    }

    fn menu_action(&mut self, choice: MenuAction) {
        match choice {
            MenuAction::Mode(mode) => self.set_mode(mode),
            MenuAction::Open(id) => self.menu = Some(id),
            MenuAction::StatType(kind) => {
                self.stats = StatData::new(kind, self.settings.stat_freq);
                self.stat_editor = StatEditor::default();
                self.set_mode(CalcMode::Stat);
            }
            MenuAction::Natural(natural) => {
                self.settings.format.natural = natural;
                self.clear_entry();
            }
            MenuAction::Angle(unit) => self.settings.angle = unit,
            MenuAction::Number(number) => self.settings.format.number = number,
            MenuAction::Fraction(style) => self.settings.format.fraction = style,
            MenuAction::StatFreq(on) => {
                self.settings.stat_freq = on;
                self.stats.freq = on;
                self.stat_editor.column = self.stat_editor.column.min(self.stats.columns().len() - 1);
            }
            MenuAction::Insert(token) => match self.screen {
                Screen::Calc => self.insert_token(token),
                Screen::StatEditor => {
                    self.stat_editor.input.insert_token(token);
                }
                Screen::Table => {
                    self.editor.insert_token(token);
                }
            },
            MenuAction::Clear(choice) => self.clear_memory(choice),
            MenuAction::StatEditor => self.screen = Screen::StatEditor,
        }
    }

    fn set_mode(&mut self, mode: CalcMode) {
        self.mode = mode;
        self.clear_entry();
        self.screen = match mode {
            CalcMode::Comp => Screen::Calc,
            CalcMode::Stat => Screen::StatEditor,
            CalcMode::Table => {
                self.show_table_stage(TableStage::Function);
                Screen::Table
            }
        };
    }

    // ---- Calculation screen ----

    fn calc_key(&mut self, key: Key, modifier: Modifier, action: Action) {
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
            Pending::None => {}
        }
        self.perform(action);
    }

    fn perform(&mut self, action: Action) {
        match action {
            // °′″ after a result switches it to degrees, minutes and seconds.
            Action::Insert(Token::Dms) if matches!(self.phase, Phase::Result(_)) => {
                if let Phase::Result(view) = &mut self.phase {
                    view.dms = !view.dms;
                    view.eng = None;
                }
            }
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
                    view.dms = false;
                }
            }
            Action::Eng(step) => {
                if let Phase::Result(view) = &mut self.phase {
                    // The first press shows plain engineering form (ENG) or one step down (SHIFT ENG).
                    view.eng = Some(match view.eng {
                        None => step.min(0),
                        Some(s) => s + step,
                    });
                    view.dms = false;
                }
            }
            Action::MemoryAdd { subtract } => self.memory_add(subtract),
            Action::Sto => self.pending = Pending::Sto,
            Action::Rcl => self.pending = Pending::Rcl,
            Action::Hyp => self.pending = Pending::Hyp,
            Action::StatMenu => {
                if self.mode == CalcMode::Stat {
                    self.open_menu(MenuId::Stat);
                }
            }
            Action::Mode | Action::Setup | Action::DrgMenu | Action::ClrMenu | Action::Nothing => {}
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
                | Token::FracBar
                | Token::PowOpen
                | Token::RootOpen
                | Token::EstX
                | Token::EstY
        );
        self.start_new_if_result(continues_ans);
        self.editor.insert_token(token);
    }

    fn insert_template(&mut self, kind: TemplateKind) {
        if !self.settings.format.natural {
            for &token in linear_tokens(kind) {
                self.insert_token(token);
            }
            return;
        }
        self.start_new_if_result(kind == TemplateKind::Power);
        self.editor.insert_template(kind);
    }

    fn clear_entry(&mut self) {
        self.editor.clear();
        self.phase = Phase::Editing;
        self.pending = Pending::None;
        self.replay = None;
    }

    /// CLR: 1 resets the setup, 2 clears memory, 3 does both and returns to COMP.
    fn clear_memory(&mut self, choice: u8) {
        if choice != 2 {
            self.settings = Settings::default();
            self.stats.freq = false;
        }
        if choice != 1 {
            self.memory = Memory::default();
        }
        if choice == 3 {
            self.history.clear();
            self.stats = StatData::default();
            self.table = Table::default();
            self.set_mode(CalcMode::Comp);
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
            self.phase = Phase::Result(ResultView::new(entry.outcome.clone(), ResultForm::Standard));
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
        let stats = (self.mode == CalcMode::Stat).then_some(&self.stats);
        let mut env = Env { settings: &self.settings, memory: &self.memory, rng: &mut self.rng, stats };
        match parse(&row).and_then(|expr| evaluate(&expr, &mut env)) {
            Ok(outcome) => {
                self.memory.ans = outcome.primary().clone();
                if self.history.len() == MAX_HISTORY {
                    self.history.remove(0);
                }
                self.history.push(HistoryEntry { row, outcome: outcome.clone() });
                self.phase = Phase::Result(ResultView::new(outcome.clone(), form));
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
        self.phase = Phase::Result(ResultView::new(Outcome::Single(value), ResultForm::Standard));
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

    /// Evaluates a standalone input: a STAT cell or a TABLE range value.
    fn evaluate_input(&mut self, row: &Row) -> CalcResult<Value> {
        let mut env = Env { settings: &self.settings, memory: &self.memory, rng: &mut self.rng, stats: None };
        parse(row).and_then(|expr| evaluate(&expr, &mut env)).map(|o| o.primary().clone()).map_err(|e| e.error)
    }

    // ---- STAT editor ----

    fn stat_editor_key(&mut self, action: Action) {
        if self.stat_editor.error.take().is_some() {
            return;
        }
        let natural = self.settings.format.natural;
        let last_column = self.stats.columns().len() - 1;
        let rows = self.stats.rows.len();
        let typing = !self.stat_editor.input.is_empty();
        let ed = &mut self.stat_editor;
        match action {
            Action::Equals => self.commit_stat_cell(),
            Action::Ac if typing => ed.input.clear(),
            Action::Ac => {
                self.screen = Screen::Calc;
                self.clear_entry();
            }
            // DEL on a cell deletes its row.
            Action::Del if !typing => {
                let row = ed.row;
                self.stats.delete_row(row);
                self.stat_editor.row = row.min(self.stats.rows.len());
            }
            Action::Up if !typing => ed.row = ed.row.saturating_sub(1),
            Action::Down if !typing => ed.row = (ed.row + 1).min(rows),
            Action::Left if !typing => ed.column = ed.column.saturating_sub(1),
            Action::Right if !typing => ed.column = (ed.column + 1).min(last_column),
            Action::StatMenu => self.open_menu(MenuId::Stat),
            other => edit(&mut ed.input, other, natural),
        }
    }

    /// Stores the typed value in the highlighted cell and moves down.
    fn commit_stat_cell(&mut self) {
        if self.stat_editor.input.is_empty() {
            return;
        }
        let row = self.stat_editor.input.root().clone();
        let column = self.stats.columns()[self.stat_editor.column];
        let result = self.evaluate_input(&row).and_then(|v| self.stats.set(self.stat_editor.row, column, v));
        match result {
            Ok(()) => {
                self.stat_editor.input.clear();
                self.stat_editor.row = (self.stat_editor.row + 1).min(self.stats.rows.len());
            }
            Err(e) => self.stat_editor.error = Some(e),
        }
    }

    // ---- TABLE ----

    fn table_key(&mut self, action: Action) {
        if self.table.error.take().is_some() {
            return self.show_table_stage(TableStage::Function);
        }
        if self.table.stage == TableStage::View {
            let t = &mut self.table;
            match action {
                Action::Up => t.selected = t.selected.saturating_sub(1),
                Action::Down => t.selected = (t.selected + 1).min(t.rows.len().saturating_sub(1)),
                Action::Left => t.column = 0,
                Action::Right => t.column = 1,
                Action::Ac => self.show_table_stage(TableStage::Function),
                _ => {}
            }
            return;
        }
        match action {
            Action::Equals => self.table_advance(),
            Action::Ac if self.table.stage == TableStage::Function => self.editor.clear(),
            Action::Ac => self.show_table_stage(TableStage::Function),
            other => edit(&mut self.editor, other, self.settings.format.natural),
        }
    }

    fn show_table_stage(&mut self, stage: TableStage) {
        self.table.stage = stage;
        self.editor =
            if stage == TableStage::Function { Editor::from_row(self.table.function.clone()) } else { Editor::new() };
        self.phase = Phase::Editing;
    }

    /// = at a TABLE prompt: keep the input (an empty range prompt keeps
    /// its current value) and go to the next step.
    fn table_advance(&mut self) {
        let stage = self.table.stage;
        if stage == TableStage::Function {
            if !self.editor.is_empty() {
                self.table.function = self.editor.root().clone();
                self.show_table_stage(TableStage::Start);
            }
            return;
        }
        if !self.editor.is_empty() {
            let row = self.editor.root().clone();
            match self.evaluate_input(&row) {
                Ok(v) => match stage {
                    TableStage::Start => self.table.start = v,
                    TableStage::End => self.table.end = v,
                    _ => self.table.step = v,
                },
                Err(e) => {
                    self.table.error = Some(e);
                    return;
                }
            }
        }
        match stage {
            TableStage::Start => self.show_table_stage(TableStage::End),
            TableStage::End => self.show_table_stage(TableStage::Step),
            _ => match self.build_table() {
                Ok(rows) => {
                    self.table.rows = rows;
                    self.table.selected = 0;
                    self.table.column = 0;
                    self.table.stage = TableStage::View;
                }
                Err(e) => self.table.error = Some(e),
            },
        }
    }

    fn build_table(&mut self) -> CalcResult<Vec<TableRow>> {
        let xs = x_values(&self.table.start, &self.table.end, &self.table.step)?;
        let expr = parse(&self.table.function).map_err(|e| e.error)?;
        let mut rows = Vec::with_capacity(xs.len());
        for x in xs {
            let mut memory = self.memory.clone();
            memory.set(Var::X, x.clone());
            let mut env = Env { settings: &self.settings, memory: &memory, rng: &mut self.rng, stats: None };
            let fx = evaluate(&expr, &mut env).map(|o| o.primary().clone()).map_err(|e| e.error);
            rows.push(TableRow { x, fx });
        }
        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::{FractionStyle, NumberFormat};
    use crate::state::AngleUnit;
    use crate::table::TableStage;

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
                "mode" => Key::Mode,
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

    #[test]
    fn setup_menu() {
        let mut calc = run("S mode 4");
        assert_eq!(calc.settings().angle, AngleUnit::Rad);
        assert!(calc.menu().is_none());
        keys(&mut calc, "S mode 6");
        assert_eq!(calc.menu().unwrap().prompt, Some("Fix 0~9?"));
        keys(&mut calc, "3 1 / 3 =");
        assert_eq!(calc.result_text().unwrap(), "1/3");
        keys(&mut calc, "sd");
        assert_eq!(calc.result_text().unwrap(), "0.333");
        keys(&mut calc, "S mode down 1");
        assert_eq!(calc.settings().format.fraction, FractionStyle::Mixed);
        // AC closes a menu without choosing.
        keys(&mut calc, "mode ac");
        assert!(calc.menu().is_none());
        assert_eq!(calc.mode(), CalcMode::Comp);
    }

    #[test]
    fn line_io() {
        let mut calc = run("S mode 2");
        assert!(!calc.settings().format.natural);
        keys(&mut calc, "1 frac 2 + 1 frac 3 =");
        assert_eq!(calc.editor().root().to_string(), "1⌟2+1⌟3");
        assert_eq!(calc.result_text().unwrap(), "5/6");
        let line = |script: &str| result(&format!("S mode 2 {script}"));
        assert_eq!(line("sqrt 2 ) ="), "1.414213562");
        assert_eq!(line("2 pow 10 ) ="), "1024");
        assert_eq!(line("3 S pow 8 ) ="), "2");
        assert_eq!(line("1 frac 1 frac 2 ="), "3/2");
        assert_eq!(line("neg 1 frac 2 ="), "-1/2");
        assert_eq!(line("S exp ="), "3.141592654");
        assert_eq!(line("S log 3 ="), "1000");
    }

    #[test]
    fn degrees_minutes_seconds_display() {
        let mut calc = run("2 . 258 =");
        keys(&mut calc, "dms");
        assert_eq!(calc.result_text().unwrap(), "2°15′28.8″");
        keys(&mut calc, "dms");
        assert_eq!(calc.result_text().unwrap(), "1129/500");
    }

    fn stat_session(kind: &str) -> Calculator {
        run(&format!("mode 2 {kind}"))
    }

    #[test]
    fn single_variable_statistics() {
        let mut calc = stat_session("1");
        assert_eq!(calc.screen(), Screen::StatEditor);
        keys(&mut calc, "1 = 2 = 3 = 4 =");
        assert_eq!(calc.stats().rows.len(), 4);
        assert_eq!(calc.stat_editor().row, 4);
        keys(&mut calc, "ac");
        assert_eq!(calc.screen(), Screen::Calc);
        keys(&mut calc, "S 1 4 2 =");
        assert_eq!(calc.result_text().unwrap(), "5/2");
        keys(&mut calc, "S 1 4 1 =");
        assert_eq!(calc.result_text().unwrap(), "4");
        keys(&mut calc, "S 1 3 1 =");
        assert_eq!(calc.result_text().unwrap(), "30");
        keys(&mut calc, "S 1 5 2 =");
        assert_eq!(calc.result_text().unwrap(), "4");
        // Back to the editor; DEL removes the highlighted row.
        keys(&mut calc, "S 1 2 up del");
        assert_eq!(calc.stats().rows.len(), 3);
        keys(&mut calc, "ac S 1 4 2 =");
        assert_eq!(calc.result_text().unwrap(), "2");
    }

    #[test]
    fn frequencies() {
        let mut calc = run("S mode down 3 1 mode 2 1");
        assert_eq!(calc.stats().columns().len(), 2);
        keys(&mut calc, "1 = 5 = up right 3 =");
        keys(&mut calc, "ac S 1 4 2 =");
        assert_eq!(calc.result_text().unwrap(), "4");
    }

    #[test]
    fn linear_regression() {
        let mut calc = stat_session("2");
        keys(&mut calc, "1 = 2 = 3 = 4 = up up up up right 3 = 5 = 7 = 9 = ac");
        let text = |calc: &mut Calculator, script: &str| {
            keys(calc, script);
            calc.result_text().unwrap()
        };
        assert_eq!(text(&mut calc, "S 1 5 1 ="), "1");
        assert_eq!(text(&mut calc, "S 1 5 2 ="), "2");
        assert_eq!(text(&mut calc, "S 1 5 3 ="), "1");
        assert_eq!(text(&mut calc, "10 S 1 5 5 ="), "21");
        assert_eq!(text(&mut calc, "21 S 1 5 4 ="), "10");
        assert_eq!(text(&mut calc, "S 1 3 5 ="), "70");
    }

    #[test]
    fn stat_entry_errors() {
        let mut calc = stat_session("1");
        keys(&mut calc, "1 / 0 =");
        assert_eq!(calc.stat_editor().error, Some(CalcError::Math));
        keys(&mut calc, "ac");
        assert_eq!(calc.stat_editor().error, None);
        assert!(calc.stats().rows.is_empty());
    }

    #[test]
    fn table_mode() {
        let mut calc = run("mode 3");
        assert_eq!(calc.screen(), Screen::Table);
        assert_eq!(calc.table().stage, TableStage::Function);
        // f(X) = X² + 1 from 1 to 3.
        keys(&mut calc, "A ) sq + 1 = 1 = 3 = =");
        let table = calc.table();
        assert_eq!(table.stage, TableStage::View);
        let fx: Vec<_> = table.rows.iter().map(|r| r.fx.clone().unwrap()).collect();
        assert_eq!(fx, vec![Value::integer(2), Value::integer(5), Value::integer(10)]);
        keys(&mut calc, "down down down");
        assert_eq!(calc.table().selected, 2);
        // AC returns to f(X) with the function kept; = = = = reuses the range.
        keys(&mut calc, "ac");
        assert_eq!(calc.editor().root().to_string(), "X²+1");
        keys(&mut calc, "= = = =");
        assert_eq!(calc.table().rows.len(), 3);
    }

    #[test]
    fn table_errors() {
        let mut calc = run("mode 3 A ) = 1 = 100 = =");
        assert_eq!(calc.table().error, Some(CalcError::Memory));
        keys(&mut calc, "ac");
        assert_eq!(calc.table().stage, TableStage::Function);
        // Rows that fail show their error without stopping the table.
        let calc = run("mode 3 1 / A ) = neg 1 = 1 = =");
        let errors: Vec<_> = calc.table().rows.iter().map(|r| r.fx.is_err()).collect();
        assert_eq!(errors, vec![false, true, false]);
    }

    #[test]
    fn snapshots_round_trip() {
        // Exact values, a surd, a decimal, settings, STAT data and a TABLE function.
        let mut calc = run("S mode 4 S mode 6 2 2 / 3 S rcl neg sqrt 2 S rcl dms 1 . 5 sin 1 = m+");
        keys(&mut calc, "mode 2 2 1 = 2 = up up right 3 = 5 = ac");
        let saved = ron::to_string(&calc.snapshot()).unwrap();
        let restored = Calculator::restore(ron::from_str(&saved).unwrap());
        assert_eq!(restored.snapshot(), calc.snapshot());
        assert_eq!(restored.mode(), CalcMode::Stat);
        assert_eq!(restored.screen(), Screen::Calc);
        assert_eq!(restored.memory().get(Var::A), &Value::rational(crate::value::Rational::new(2, 3).unwrap()));
        assert_eq!(restored.memory().get(Var::B), &Value::integer(2).sqrt().unwrap());
        assert!(!restored.memory().get(Var::M).is_exact());
        let mut restored = restored;
        // B = 2, shown in the restored Fix 2 format.
        keys(&mut restored, "S 1 5 2 =");
        assert_eq!(restored.result_text().unwrap(), "2.00");

        let table = run("mode 3 A ) sq = 2 = 4 = =");
        let restored = Calculator::restore(ron::from_str(&ron::to_string(&table.snapshot()).unwrap()).unwrap());
        assert_eq!(restored.screen(), Screen::Table);
        assert_eq!(restored.editor().root().to_string(), "X²");
        assert_eq!(restored.table().start, Value::integer(2));
    }

    #[test]
    fn tampered_snapshots_are_rejected() {
        let saved = ron::to_string(&Calculator::new().snapshot()).unwrap();
        assert!(ron::from_str::<Snapshot>(&saved).is_ok());
        // Unreduced fractions are normalized; impossible values are refused.
        let parse_value = |s: &str| ron::from_str::<Value>(s);
        let half = parse_value(r#"Surd([(coef: (num: "2", den: "4"), radicand: 1)])"#).unwrap();
        assert_eq!(half, Value::rational(crate::value::Rational::new(1, 2).unwrap()));
        assert!(parse_value(r#"Surd([(coef: (num: "1", den: "0"), radicand: 1)])"#).is_err());
        assert!(parse_value(r#"Surd([(coef: (num: "1", den: "1"), radicand: 8)])"#).is_err());
        assert!(parse_value("Decimal(1e300)").is_err());
        assert!(ron::from_str::<crate::input::Template>("(kind: Fraction, slots: [()])").is_err());
    }

    #[test]
    fn clear_all_resets_mode() {
        let calc = run("S mode 2 mode 3 S 9 3");
        assert_eq!(calc.mode(), CalcMode::Comp);
        assert!(calc.settings().format.natural);
    }
}

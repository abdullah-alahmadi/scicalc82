//! The top-level state machine: key presses in, display state out.

use serde::{Deserialize, Serialize};

use crate::ast::LocatedError;
use crate::eqn::{Equations, Solution};
use crate::error::{CalcError, CalcResult};
use crate::eval::{Env, Outcome, evaluate};
use crate::format::{
    Formatted, ResultForm, format_dms, format_eng, format_matrix, format_polar, format_rect, format_value,
};
use crate::input::{Cursor, Editor, Func, Item, Row, TemplateKind, Token, Var};
use crate::key::{Action, Key, Modifier, action, hyperbolic, var_for_key};
use crate::matrix::{MatId, Matrices, Matrix};
use crate::menu::{self, MenuAction, MenuId, MenuView};
use crate::number::Number;
use crate::parse::{ComplexForm, parse, parse_statement};
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
    /// A trailing ▸r∠θ or ▸a+bi in the input (CMPLX mode).
    pub complex_form: Option<ComplexForm>,
}

impl ResultView {
    fn new(outcome: Outcome, form: ResultForm) -> Self {
        Self { outcome, form, eng: None, dms: false, complex_form: None }
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
    /// Entering a matrix's cells (MATRIX mode).
    MatrixEditor,
    /// Entering equation coefficients (EQN mode).
    EqnEditor,
    /// Showing an equation's solutions one at a time (EQN mode).
    EqnSolution,
}

/// The coefficient editor's highlighted cell and the value being typed.
#[derive(Clone, Debug, Default)]
pub struct EqnEditor {
    pub row: usize,
    pub col: usize,
    pub input: Editor,
    /// An error from the last entry, shown until the next key.
    pub error: Option<CalcError>,
}

#[derive(Clone, Debug)]
struct EqnState {
    result: CalcResult<Solution>,
    /// Which root is showing.
    index: usize,
    form: ResultForm,
}

/// What the solution screen shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EqnView {
    Root { label: &'static str, value: Formatted, index: usize, count: usize },
    NoSolution,
    Infinite,
    Error(CalcError),
}

/// The matrix being edited, its highlighted cell, and the value being typed.
#[derive(Clone, Debug)]
pub struct MatrixEditor {
    pub id: MatId,
    pub row: usize,
    pub col: usize,
    pub input: Editor,
    /// An error from the last entry, shown until the next key.
    pub error: Option<CalcError>,
}

impl MatrixEditor {
    fn new(id: MatId) -> Self {
        Self { id, row: 0, col: 0, input: Editor::new(), error: None }
    }
}

impl Default for MatrixEditor {
    fn default() -> Self {
        Self::new(MatId::A)
    }
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
    complex_form: Option<ComplexForm>,
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
    matrices: Matrices,
    matrix_editor: MatrixEditor,
    equations: Equations,
    eqn_editor: EqnEditor,
    eqn_solution: Option<EqnState>,
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
    #[serde(default)]
    pub matrices: Matrices,
    #[serde(default)]
    pub equations: Equations,
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
            matrices: self.matrices.clone(),
            equations: self.equations.clone(),
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
        calc.matrices = snapshot.matrices;
        calc.equations = snapshot.equations;
        calc.set_mode(snapshot.mode);
        // MATRIX and EQN modes start on their screens, not their prompts.
        calc.menu = None;
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

    pub fn matrices(&self) -> &Matrices {
        &self.matrices
    }

    pub fn matrix_editor(&self) -> &MatrixEditor {
        &self.matrix_editor
    }

    pub fn equations(&self) -> &Equations {
        &self.equations
    }

    pub fn eqn_editor(&self) -> &EqnEditor {
        &self.eqn_editor
    }

    /// The solution screen's content, while it is showing.
    pub fn eqn_view(&self) -> Option<EqnView> {
        let state = self.eqn_solution.as_ref()?;
        Some(match &state.result {
            Err(e) => EqnView::Error(*e),
            Ok(Solution::NoSolution) => EqnView::NoSolution,
            Ok(Solution::Infinite) => EqnView::Infinite,
            Ok(Solution::Roots(roots)) => {
                let root = &roots[state.index];
                let value = if root.value.is_complex() && self.settings.complex_polar {
                    match root.value.to_polar(self.settings.angle) {
                        Ok((r, theta)) => format_polar(&r, &theta, &self.settings.format, state.form),
                        Err(_) => format_rect(root.value.re(), &root.value.im(), &self.settings.format, state.form),
                    }
                } else if root.value.is_complex() {
                    format_rect(root.value.re(), &root.value.im(), &self.settings.format, state.form)
                } else {
                    format_value(root.value.re(), &self.settings.format, state.form)
                };
                EqnView::Root { label: root.label, value, index: state.index, count: roots.len() }
            }
        })
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
        // Complex results (and anything with ▸r∠θ) as a+bi or r∠θ.
        let show_number = |n: &Number| {
            let polar = match view.complex_form {
                Some(form) => form == ComplexForm::Polar,
                None => self.settings.complex_polar && n.is_complex(),
            };
            let (settings, form) = (&self.settings.format, view.form);
            match n {
                Number::Real(v) if !polar => show(v),
                _ if polar => match n.to_polar(self.settings.angle) {
                    Ok((r, theta)) => format_polar(&r, &theta, settings, form),
                    Err(_) => format_rect(n.re(), &n.im(), settings, form),
                },
                _ => format_rect(n.re(), &n.im(), settings, form),
            }
        };
        let line = |label, v| ResultLine { label, value: show(v) };
        Some(match &view.outcome {
            Outcome::Single(n) => vec![ResultLine { label: None, value: show_number(n) }],
            Outcome::Polar { r, theta } => vec![line(Some("r"), r), line(Some("θ"), theta)],
            Outcome::Rect { x, y } => vec![line(Some("X"), x), line(Some("Y"), y)],
            Outcome::Matrix(m) => {
                vec![ResultLine { label: None, value: format_matrix(m, &self.settings.format, view.form) }]
            }
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
            Screen::MatrixEditor => self.matrix_editor_key(action),
            Screen::EqnEditor => self.eqn_editor_key(action),
            Screen::EqnSolution => self.eqn_solution_key(action),
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
                Screen::MatrixEditor => {
                    self.matrix_editor.input.insert_token(token);
                }
                Screen::EqnEditor => {
                    self.eqn_editor.input.insert_token(token);
                }
                Screen::EqnSolution => {}
            },
            MenuAction::Clear(choice) => self.clear_memory(choice),
            MenuAction::StatEditor => self.screen = Screen::StatEditor,
            MenuAction::ComplexPolar(polar) => self.settings.complex_polar = polar,
            MenuAction::DefineMatrix(id, rows, cols) => {
                *self.matrices.slot_mut(id) = Matrix::zeros(rows, cols);
                self.open_matrix_editor(id);
            }
            MenuAction::EqnType(kind) => {
                if self.equations.kind() != kind {
                    self.equations = Equations::new(kind);
                }
                self.eqn_editor = EqnEditor::default();
                self.eqn_solution = None;
                self.screen = Screen::EqnEditor;
            }
            MenuAction::EditMatrix(id) => {
                if self.matrices.slot(id).is_some() {
                    self.open_matrix_editor(id);
                } else {
                    self.menu = Some(MenuId::MatDim { id, more: false });
                }
            }
        }
    }

    fn set_mode(&mut self, mode: CalcMode) {
        self.mode = mode;
        self.clear_entry();
        self.screen = match mode {
            CalcMode::Comp | CalcMode::Cmplx => Screen::Calc,
            // Like the device, MATRIX mode starts by asking which matrix to set up.
            CalcMode::Matrix => {
                self.menu = Some(MenuId::MatSelect { define: true });
                Screen::Calc
            }
            // EQN mode asks for the equation type, then edits its coefficients.
            CalcMode::Eqn => {
                self.menu = Some(MenuId::EqnType);
                self.eqn_solution = None;
                Screen::EqnEditor
            }
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
        self.perform(self.mode_action(action));
    }

    /// Keys that act differently in CMPLX mode: ENG types i there, and the
    /// CMPLX-only ∠ and CMPLX menu do nothing in other modes.
    fn mode_action(&self, action: Action) -> Action {
        let cmplx = self.mode == CalcMode::Cmplx;
        match action {
            Action::Eng(1) if cmplx => Action::Insert(Token::Imag),
            Action::Insert(Token::Angle) | Action::CmplxMenu if !cmplx => Action::Nothing,
            other => other,
        }
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
            Action::CmplxMenu => self.open_menu(MenuId::Cmplx),
            Action::MatrixMenu => {
                if self.mode == CalcMode::Matrix {
                    self.open_menu(MenuId::Matrix);
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
                | Token::Angle
                | Token::ToPolar
                | Token::ToRect
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
            self.matrices = Matrices::default();
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
            let view = ResultView {
                complex_form: entry.complex_form,
                ..ResultView::new(entry.outcome.clone(), ResultForm::Standard)
            };
            self.phase = Phase::Result(view);
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
        let complex = self.mode == CalcMode::Cmplx;
        let matrices = (self.mode == CalcMode::Matrix).then_some(&self.matrices);
        let mut env =
            Env { settings: &self.settings, memory: &self.memory, rng: &mut self.rng, stats, complex, matrices };
        let result = parse_statement(&row)
            .and_then(|(expr, complex_form)| evaluate(&expr, &mut env).map(|outcome| (outcome, complex_form)));
        match result {
            Ok((outcome, complex_form)) => {
                match &outcome {
                    Outcome::Matrix(m) => self.matrices.ans = Some(m.clone()),
                    other => self.memory.ans = other.primary().expect("non-matrix outcomes have a number"),
                }
                if self.history.len() == MAX_HISTORY {
                    self.history.remove(0);
                }
                self.history.push(HistoryEntry { row, outcome: outcome.clone(), complex_form });
                self.phase = Phase::Result(ResultView { complex_form, ..ResultView::new(outcome.clone(), form) });
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
    fn current_value(&mut self) -> Option<Number> {
        match &self.phase {
            Phase::Result(view) => view.outcome.primary(),
            _ if self.editor.is_empty() => Some(self.memory.ans.clone()),
            _ => self.execute(ResultForm::Standard).and_then(|o| o.primary()),
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
        let mut env = Env {
            settings: &self.settings,
            memory: &self.memory,
            rng: &mut self.rng,
            stats: None,
            complex: false,
            matrices: None,
        };
        let outcome = parse(row).and_then(|expr| evaluate(&expr, &mut env)).map_err(|e| e.error)?;
        outcome.primary().ok_or(CalcError::Math)?.into_real()
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

    // ---- Matrix editor ----

    fn open_matrix_editor(&mut self, id: MatId) {
        self.matrix_editor = MatrixEditor::new(id);
        self.screen = Screen::MatrixEditor;
    }

    fn matrix_editor_key(&mut self, action: Action) {
        if self.matrix_editor.error.take().is_some() {
            return;
        }
        let Some(m) = self.matrices.slot(self.matrix_editor.id) else {
            self.screen = Screen::Calc;
            return;
        };
        let (rows, cols) = (m.rows(), m.cols());
        let natural = self.settings.format.natural;
        let typing = !self.matrix_editor.input.is_empty();
        let ed = &mut self.matrix_editor;
        match action {
            Action::Equals => self.commit_matrix_cell(),
            Action::Ac if typing => ed.input.clear(),
            Action::Ac => {
                self.screen = Screen::Calc;
                self.clear_entry();
            }
            Action::Up if !typing => ed.row = ed.row.saturating_sub(1),
            Action::Down if !typing => ed.row = (ed.row + 1).min(rows - 1),
            Action::Left if !typing => ed.col = ed.col.saturating_sub(1),
            Action::Right if !typing => ed.col = (ed.col + 1).min(cols - 1),
            Action::MatrixMenu => self.open_menu(MenuId::Matrix),
            other => edit(&mut ed.input, other, natural),
        }
    }

    /// Stores the typed value in the highlighted cell and moves to the next
    /// cell, row by row.
    fn commit_matrix_cell(&mut self) {
        if self.matrix_editor.input.is_empty() {
            return;
        }
        let row = self.matrix_editor.input.root().clone();
        match self.evaluate_input(&row) {
            Ok(value) => {
                let ed = &mut self.matrix_editor;
                let Some(m) = self.matrices.slot_mut(ed.id) else { return };
                m.set(ed.row, ed.col, value);
                ed.input.clear();
                if ed.col + 1 < m.cols() {
                    ed.col += 1;
                } else if ed.row + 1 < m.rows() {
                    ed.row += 1;
                    ed.col = 0;
                }
            }
            Err(e) => self.matrix_editor.error = Some(e),
        }
    }

    // ---- EQN ----

    fn eqn_editor_key(&mut self, action: Action) {
        if self.eqn_editor.error.take().is_some() {
            return;
        }
        let kind = self.equations.kind();
        let natural = self.settings.format.natural;
        let typing = !self.eqn_editor.input.is_empty();
        let ed = &mut self.eqn_editor;
        match action {
            Action::Equals if typing => self.commit_eqn_cell(),
            // = with nothing typed solves, as on the device.
            Action::Equals => {
                self.eqn_solution =
                    Some(EqnState { result: self.equations.solve(), index: 0, form: ResultForm::Standard });
                self.screen = Screen::EqnSolution;
            }
            Action::Ac => ed.input.clear(),
            Action::Up if !typing => ed.row = ed.row.saturating_sub(1),
            Action::Down if !typing => ed.row = (ed.row + 1).min(kind.rows() - 1),
            Action::Left if !typing => ed.col = ed.col.saturating_sub(1),
            Action::Right if !typing => ed.col = (ed.col + 1).min(kind.cols() - 1),
            other => edit(&mut ed.input, other, natural),
        }
    }

    /// Stores the typed coefficient and moves to the next cell, row by row.
    fn commit_eqn_cell(&mut self) {
        let row = self.eqn_editor.input.root().clone();
        match self.evaluate_input(&row) {
            Ok(value) => {
                let kind = self.equations.kind();
                let ed = &mut self.eqn_editor;
                self.equations.set(ed.row, ed.col, value);
                ed.input.clear();
                if ed.col + 1 < kind.cols() {
                    ed.col += 1;
                } else if ed.row + 1 < kind.rows() {
                    ed.row += 1;
                    ed.col = 0;
                }
            }
            Err(e) => self.eqn_editor.error = Some(e),
        }
    }

    /// ▲/▼ move between solutions; = steps forward and, after the last,
    /// returns to the coefficients, as does AC.
    fn eqn_solution_key(&mut self, action: Action) {
        let Some(state) = &mut self.eqn_solution else {
            self.screen = Screen::EqnEditor;
            return;
        };
        let count = match &state.result {
            Ok(Solution::Roots(roots)) => roots.len(),
            _ => 0,
        };
        match action {
            Action::Down if state.index + 1 < count => state.index += 1,
            Action::Up => state.index = state.index.saturating_sub(1),
            Action::Equals if state.index + 1 < count => state.index += 1,
            Action::SToD => {
                state.form = match state.form {
                    ResultForm::Standard => ResultForm::Decimal,
                    ResultForm::Decimal => ResultForm::Standard,
                }
            }
            Action::Equals | Action::Ac => {
                self.eqn_solution = None;
                self.screen = Screen::EqnEditor;
            }
            _ => {}
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
            memory.set(Var::X, Number::Real(x.clone()));
            let mut env = Env {
                settings: &self.settings,
                memory: &memory,
                rng: &mut self.rng,
                stats: None,
                complex: false,
                matrices: None,
            };
            let fx = evaluate(&expr, &mut env)
                .map_err(|e| e.error)
                .and_then(|o| o.primary().ok_or(CalcError::Math)?.into_real());
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
                "abs" => Key::Abs,
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
        assert_eq!(calc.memory().get(Var::A), &Number::zero());
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
            let r = calc.memory().ans.re().as_rational().expect("Ran# is exact");
            assert!(r.signum() >= 0 && r.to_f64() < 1.0 && 1000 % r.denom() == 0);
            keys(&mut calc, "A . 1 S ) 6 =");
            let n = calc.memory().ans.re().as_rational().unwrap();
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
        run(&format!("mode 3 {kind}"))
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
        let mut calc = run("S mode down 3 1 mode 3 1");
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
        let mut calc = run("mode 4");
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
        let mut calc = run("mode 4 A ) = 1 = 100 = =");
        assert_eq!(calc.table().error, Some(CalcError::Memory));
        keys(&mut calc, "ac");
        assert_eq!(calc.table().stage, TableStage::Function);
        // Rows that fail show their error without stopping the table.
        let calc = run("mode 4 1 / A ) = neg 1 = 1 = =");
        let errors: Vec<_> = calc.table().rows.iter().map(|r| r.fx.is_err()).collect();
        assert_eq!(errors, vec![false, true, false]);
    }

    fn cmplx(script: &str) -> String {
        result(&format!("mode 2 {script}"))
    }

    #[test]
    fn complex_arithmetic() {
        // In CMPLX mode the ENG key types i.
        assert_eq!(cmplx("1 + 2 eng ="), "1+2i");
        assert_eq!(cmplx("( 1 + eng ) * ( 1 - eng ) ="), "2");
        assert_eq!(cmplx("( 1 + eng ) sq ="), "2i");
        assert_eq!(cmplx("1 / eng ="), "-i");
        assert_eq!(cmplx("( 3 + 4 eng ) / ( 1 + 2 eng ) ="), "11/5-2/5i");
        assert_eq!(cmplx("eng pow 4 ="), "1");
        assert_eq!(cmplx("3 eng - 3 eng ="), "0");
        assert_eq!(cmplx("1 / 2 + sqrt 3 right eng / 2 ="), "1/2+√3/2i");
        // Implied multiplication binds tighter than ÷: 1÷2i = 1/(2i).
        assert_eq!(cmplx("1 / 2 eng ="), "-1/2i");
        assert_eq!(cmplx("sqrt neg 4 ="), "2i");
    }

    #[test]
    fn polar_form() {
        // SHIFT (−) types ∠; angles follow the angle unit.
        assert_eq!(cmplx("2 S neg 30 ="), "√3+i");
        assert_eq!(cmplx("2 S neg 90 ="), "2i");
        assert_eq!(cmplx("1 + 2 S neg 180 ="), "-1");
        // CMPLX menu (SHIFT 2): 3 ▸r∠θ, 4 ▸a+bi.
        assert_eq!(cmplx("1 + eng S 2 3 ="), "√2∠45");
        assert_eq!(cmplx("neg 1 - eng S 2 3 ="), "√2∠-135");
        assert_eq!(cmplx("2 S neg 30 S 2 4 ="), "√3+i");
        assert_eq!(result_in(AngleUnit::Rad, "mode 2 1 + eng S 2 3 ="), "√2∠π/4");
        // SETUP page 2, item 4: show complex results as r∠θ by default.
        let mut calc = run("S mode down 4 2 mode 2");
        assert!(calc.settings().complex_polar);
        keys(&mut calc, "3 + 4 eng =");
        assert_eq!(calc.result_text().unwrap(), "5∠53.13010235");
        // Real results stay plain unless ▸r∠θ asks otherwise.
        keys(&mut calc, "2 =");
        assert_eq!(calc.result_text().unwrap(), "2");
    }

    #[test]
    fn complex_functions() {
        assert_eq!(cmplx("abs 3 + 4 eng ="), "5");
        assert_eq!(cmplx("S 2 1 1 + eng ) ="), "45");
        assert_eq!(cmplx("S 2 1 neg 1 ) ="), "180");
        assert_eq!(cmplx("S 2 2 1 + 2 eng ) ="), "1-2i");
        // Real-only functions reject complex numbers, as on the device.
        assert_eq!(error("mode 2 sin eng ) ="), CalcError::Math);
        assert_eq!(error("mode 2 log eng ) ="), CalcError::Math);
        assert_eq!(error("mode 2 eng pow 1 frac 2 ="), CalcError::Math);
        assert_eq!(error("mode 2 sqrt eng ="), CalcError::Math);
    }

    #[test]
    fn complex_memory() {
        // Ans and variables hold complex numbers.
        assert_eq!(cmplx("1 + eng S rcl neg ac A neg * 2 ="), "2+2i");
        assert_eq!(cmplx("1 + eng = ans * ans ="), "2i");
        assert_eq!(cmplx("eng m+ eng m+ A m+ ="), "2i");
        // ▸r∠θ after a result continues from Ans.
        assert_eq!(cmplx("eng = S 2 3 ="), "1∠90");
    }

    #[test]
    fn complex_input_only_in_cmplx_mode() {
        // In COMP, ENG is ENG and ∠ is ignored.
        assert_eq!(result("2 S neg 30 ="), "230");
        assert_eq!(result("1234 = eng"), "1.234×10^3");
        assert!(run("S 2").menu().is_none());
        assert_eq!(error("sqrt neg 4 ="), CalcError::Math);
        // A complex Ans can't feed real-only STAT or TABLE inputs.
        let mut calc = run("mode 2 eng = mode 3 1 ans =");
        assert_eq!(calc.stat_editor().error, Some(CalcError::Math));
        keys(&mut calc, "ac");
    }

    /// MATRIX mode with MatA = [[1, 2], [3, 4]] and MatB = the 2×2 identity.
    /// MODE 5 asks which matrix to size; 5 is 2×2.
    fn matrix_session() -> Calculator {
        run("mode 5 1 5 1 = 2 = 3 = 4 = ac S 4 1 2 5 1 = 0 = 0 = 1 = ac")
    }

    fn matrix(script: &str) -> String {
        let mut calc = matrix_session();
        keys(&mut calc, script);
        calc.result_text().unwrap_or_else(|| panic!("no result for {script:?}: {:?}", calc.phase()))
    }

    #[test]
    fn defining_matrices() {
        let calc = matrix_session();
        assert_eq!(calc.mode(), CalcMode::Matrix);
        assert_eq!(calc.screen(), Screen::Calc);
        let a = calc.matrices().get(MatId::A).unwrap();
        assert_eq!((a.rows(), a.cols()), (2, 2));
        assert_eq!(a.get(1, 0), &Value::integer(3));
        // SHIFT 4 3 types MatA.
        assert_eq!(matrix("S 4 3 ="), "[[1, 2], [3, 4]]");
        // The size menu's second page has the 1×n sizes.
        let mut calc = run("mode 5 3 down 1 7 = 8 = 9 = ac S 4 5 =");
        assert_eq!(calc.result_text().unwrap(), "[[7, 8, 9]]");
        // Data (SHIFT 4 2) reopens a matrix to change a cell.
        keys(&mut calc, "S 4 2 3 right 0 = ac S 4 5 =");
        assert_eq!(calc.result_text().unwrap(), "[[7, 0, 9]]");
    }

    #[test]
    fn matrix_arithmetic() {
        assert_eq!(matrix("S 4 3 + S 4 4 ="), "[[2, 2], [3, 5]]");
        assert_eq!(matrix("S 4 3 - S 4 4 ="), "[[0, 2], [3, 3]]");
        assert_eq!(matrix("S 4 3 * S 4 3 ="), "[[7, 10], [15, 22]]");
        assert_eq!(matrix("S 4 3 sq ="), "[[7, 10], [15, 22]]");
        assert_eq!(matrix("2 S 4 3 ="), "[[2, 4], [6, 8]]");
        assert_eq!(matrix("S 4 3 / 2 ="), "[[1/2, 1], [3/2, 2]]");
        assert_eq!(matrix("neg S 4 3 ="), "[[-1, -2], [-3, -4]]");
        assert_eq!(matrix("S 4 3 recip ="), "[[-2, 1], [3/2, -1/2]]");
        assert_eq!(matrix("S 4 3 * S 4 3 recip ="), "[[1, 0], [0, 1]]");
        assert_eq!(matrix("S 4 3 pow 3 ="), "[[37, 54], [81, 118]]");
        assert_eq!(matrix("abs neg S 4 3 ="), "[[1, 2], [3, 4]]");
    }

    #[test]
    fn matrix_functions() {
        assert_eq!(matrix("S 4 7 S 4 3 ) ="), "-2");
        assert_eq!(matrix("S 4 8 S 4 3 ) ="), "[[1, 3], [2, 4]]");
        // det gives a number, which becomes Ans; matrix results become MatAns.
        let mut calc = matrix_session();
        keys(&mut calc, "S 4 7 S 4 3 ) = S 4 3 * 2 =");
        assert_eq!(calc.memory().ans, Number::Real(Value::integer(-2)));
        keys(&mut calc, "S 4 6 + S 4 6 =");
        assert_eq!(calc.result_text().unwrap(), "[[4, 8], [12, 16]]");
    }

    #[test]
    fn matrix_errors() {
        let error_of = |script: &str| {
            let mut calc = matrix_session();
            keys(&mut calc, script);
            calc.error().unwrap_or_else(|| panic!("no error for {script:?}"))
        };
        // MatC is 3×1.
        let mut calc = matrix_session();
        keys(&mut calc, "S 4 1 3 3 1 = 2 = 3 = ac S 4 3 * S 4 5 =");
        assert_eq!(calc.error(), Some(CalcError::Dimension));
        assert_eq!(error_of("S 4 3 + 1 ="), CalcError::Math);
        assert_eq!(error_of("S 4 5 ="), CalcError::Dimension); // MatC not defined
        assert_eq!(error_of("sin S 4 3 ) ="), CalcError::Math);
        assert_eq!(error_of("S 4 7 2 ) ="), CalcError::Math);
        // A singular matrix has no inverse.
        let mut calc = run("mode 5 1 5 1 = 2 = 2 = 4 = ac S 4 3 recip =");
        assert_eq!(calc.error(), Some(CalcError::Math));
        keys(&mut calc, "ac");
        // Matrices can't be stored in number variables.
        keys(&mut calc, "S 4 3 = S rcl neg");
        assert_eq!(calc.memory().get(Var::A), &Number::zero());
    }

    #[test]
    fn matrices_only_in_matrix_mode() {
        assert!(run("S 4").menu().is_none());
        // In COMP, SHIFT 4 does nothing, so this just types 1.
        assert_eq!(result("S 4 1 ="), "1");
        let calc = Calculator::restore(ron::from_str(&ron::to_string(&matrix_session().snapshot()).unwrap()).unwrap());
        assert_eq!(calc.mode(), CalcMode::Matrix);
        assert!(calc.menu().is_none(), "restored MATRIX mode starts on the calculation screen");
        assert_eq!(calc.matrices().get(MatId::B).unwrap(), &Matrix::identity(2).unwrap());
    }

    /// Shows each solution in turn: `label=value`, pressing = between them.
    fn eqn_roots(script: &str) -> Vec<String> {
        let mut calc = run(&format!("mode 6 {script} ="));
        let mut shown = Vec::new();
        while let Some(view) = calc.eqn_view() {
            shown.push(match view {
                EqnView::Root { label, value, .. } => format!("{label}={value}"),
                other => format!("{other:?}"),
            });
            keys(&mut calc, "=");
        }
        assert_eq!(calc.screen(), Screen::EqnEditor, "= after the last solution returns to the coefficients");
        shown
    }

    #[test]
    fn simultaneous_equations() {
        // 1: anX+bnY=cn. x + y = 3, x − y = 1
        assert_eq!(eqn_roots("1 1 = 1 = 3 = 1 = neg 1 = 1 ="), ["X=2", "Y=1"]);
        assert_eq!(eqn_roots("1 3 = 0 = 1 = 0 = 3 = 2 ="), ["X=1/3", "Y=2/3"]);
        // 2: three unknowns.
        assert_eq!(eqn_roots("2 1 = 1 = 1 = 6 = 0 = 2 = 5 = neg 4 = 2 = 5 = neg 1 = 27 ="), ["X=5", "Y=3", "Z=-2"]);
        assert_eq!(eqn_roots("1 1 = 1 = 1 = 2 = 2 = 3 ="), ["NoSolution"]);
        assert_eq!(eqn_roots("1 1 = 1 = 1 = 2 = 2 = 2 ="), ["Infinite"]);
    }

    #[test]
    fn polynomial_equations() {
        // 3: aX²+bX+c=0
        assert_eq!(eqn_roots("3 1 = neg 3 = 2 ="), ["X1=2", "X2=1"]);
        assert_eq!(eqn_roots("3 1 = 0 = neg 2 ="), ["X1=√2", "X2=-√2"]);
        assert_eq!(eqn_roots("3 1 = 0 = 1 ="), ["X1=i", "X2=-i"]);
        assert_eq!(eqn_roots("3 1 = neg 2 = 1 ="), ["X=1"]);
        // 4: aX³+bX²+cX+d=0
        assert_eq!(eqn_roots("4 1 = 0 = 0 = neg 1 ="), ["X1=1", "X2=-1/2+√3/2i", "X3=-1/2-√3/2i"]);
        assert_eq!(eqn_roots("4 1 = 0 = neg 2 = 0 ="), ["X1=√2", "X2=0", "X3=-√2"]);
        assert_eq!(eqn_roots("4 0 = 1 = 1 = 1 ="), ["Error(Math)"]);
    }

    #[test]
    fn eqn_screens() {
        let mut calc = run("mode 6 3 1 = neg 3 = 2 = =");
        assert_eq!(calc.screen(), Screen::EqnSolution);
        // ▼ and ▲ move between solutions; S⇔D shows decimals.
        keys(&mut calc, "down down");
        assert!(matches!(calc.eqn_view(), Some(EqnView::Root { index: 1, count: 2, .. })));
        keys(&mut calc, "up");
        assert!(matches!(calc.eqn_view(), Some(EqnView::Root { index: 0, .. })));
        let mut calc = run("mode 6 3 2 = 0 = neg 1 = = sd");
        assert!(matches!(calc.eqn_view(), Some(EqnView::Root { value, .. }) if value.to_string() == "0.7071067812"));
        // AC returns to the coefficients, which are kept; coefficients may be expressions.
        keys(&mut calc, "ac");
        assert_eq!(calc.screen(), Screen::EqnEditor);
        assert_eq!(calc.equations().get(0, 0), &Value::integer(2));
        let calc = run("mode 6 3 1 / 2 = sqrt 4 = 0 =");
        assert_eq!(calc.equations().get(0, 0), &Value::rational(crate::value::Rational::new(1, 2).unwrap()));
        assert_eq!(calc.equations().get(0, 1), &Value::integer(2));
        // The equations survive a restart, opening on the coefficient editor.
        let calc = run("mode 6 4 1 = 2 =");
        let restored = Calculator::restore(ron::from_str(&ron::to_string(&calc.snapshot()).unwrap()).unwrap());
        assert_eq!(restored.screen(), Screen::EqnEditor);
        assert!(restored.menu().is_none());
        assert_eq!(restored.equations().get(0, 1), &Value::integer(2));
    }

    #[test]
    fn snapshots_round_trip() {
        // Exact values, a surd, a decimal, settings, STAT data and a TABLE function.
        let mut calc = run("S mode 4 S mode 6 2 2 / 3 S rcl neg sqrt 2 S rcl dms 1 . 5 sin 1 = m+");
        keys(&mut calc, "mode 3 2 1 = 2 = up up right 3 = 5 = ac");
        let saved = ron::to_string(&calc.snapshot()).unwrap();
        let restored = Calculator::restore(ron::from_str(&saved).unwrap());
        assert_eq!(restored.snapshot(), calc.snapshot());
        assert_eq!(restored.mode(), CalcMode::Stat);
        assert_eq!(restored.screen(), Screen::Calc);
        assert_eq!(
            restored.memory().get(Var::A),
            &Number::Real(Value::rational(crate::value::Rational::new(2, 3).unwrap()))
        );
        assert_eq!(restored.memory().get(Var::B), &Number::Real(Value::integer(2).sqrt().unwrap()));
        assert!(!restored.memory().get(Var::M).re().is_exact());
        let mut restored = restored;
        // B = 2, shown in the restored Fix 2 format.
        keys(&mut restored, "S 1 5 2 =");
        assert_eq!(restored.result_text().unwrap(), "2.00");

        let table = run("mode 4 A ) sq = 2 = 4 = =");
        let restored = Calculator::restore(ron::from_str(&ron::to_string(&table.snapshot()).unwrap()).unwrap());
        assert_eq!(restored.screen(), Screen::Table);
        assert_eq!(restored.editor().root().to_string(), "X²");
        assert_eq!(restored.table().start, Value::integer(2));
    }

    #[test]
    fn complex_memory_is_saved() {
        let calc = run("mode 2 1 + 2 eng S rcl neg");
        let saved = ron::to_string(&calc.snapshot()).unwrap();
        let restored = Calculator::restore(ron::from_str(&saved).unwrap());
        assert_eq!(restored.memory().get(Var::A), &Number::from_parts(Value::integer(1), Value::integer(2)));
        assert_eq!(restored.mode(), CalcMode::Cmplx);
        // Saves from before complex support store plain values; they still load.
        let old_memory = r#"(vars: (Surd([(coef: (num: "5", den: "1"), radicand: 1)]), Surd([]), Surd([]), Surd([]), Surd([]), Surd([]), Surd([]), Surd([]), Surd([])), ans: Surd([]))"#;
        let memory: Memory = ron::from_str(old_memory).unwrap();
        assert_eq!(memory.get(Var::A), &Number::Real(Value::integer(5)));
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
        let calc = run("S mode 2 mode 4 S 9 3");
        assert_eq!(calc.mode(), CalcMode::Comp);
        assert!(calc.settings().format.natural);
    }
}

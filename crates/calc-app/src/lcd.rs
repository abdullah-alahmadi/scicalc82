//! The display: status indicators plus whichever screen is active — the
//! calculation lines, a menu, the STAT editor, or TABLE mode.

use calc_core::calculator::{Pending, Phase, ResultLine, Screen};
use calc_core::format::{FormatSettings, Formatted, NumberFormat, format_decimal};
use calc_core::input::{Editor, Var};
use calc_core::key::Modifier;
use calc_core::menu::MenuView;
use calc_core::state::{AngleUnit, CalcMode};
use calc_core::table::TableStage;
use calc_core::{CalcError, Calculator, Value};
use eframe::egui::{Align2, Color32, Painter, Rect, vec2};

use crate::dots;
use crate::math::layout::{CursorBar, layout};
use crate::math::nodes::{Node, formatted_nodes, input_nodes, label_nodes, result_nodes};
use crate::math::paint::{EguiMeasure, paint, paint_cursor};
use crate::theme::{self, View};

const INPUT_SIZE: f32 = 17.0;
/// Height of the input area in design units; the result area fills the rest.
const INPUT_HEIGHT: f32 = 48.0;
const RESULT_SIZE: f32 = 20.0;
const MIN_RESULT_SIZE: f32 = 11.0;
const INDICATOR_SIZE: f32 = 9.5;
const MENU_SIZE: f32 = 13.0;
const CELL_SIZE: f32 = 11.5;

/// The glass area in design coordinates.
pub const GLASS: (f32, f32, f32, f32) = (34.0, 64.0, 292.0, 112.0);

/// A rectangle inside the glass, in design units relative to its top-left.
fn glass_rect(view: View, x: f32, y: f32, w: f32, h: f32) -> Rect {
    let (gx, gy, ..) = GLASS;
    view.rect(gx + x, gy + y, w, h)
}

pub fn draw(painter: &Painter, view: View, calc: &Calculator, cursor_visible: bool) {
    let (x, y, w, h) = GLASS;
    painter.rect_filled(view.rect(x - 10.0, y - 10.0, w + 20.0, h + 20.0), view.len(10.0), theme::BEZEL);
    let glass = view.rect(x, y, w, h);
    painter.rect_filled(glass, view.len(4.0), theme::LCD_BG);
    let painter = painter.with_clip_rect(glass);

    draw_indicators(&painter, view, calc);
    if let Some(menu) = calc.menu() {
        return draw_menu(&painter, view, &menu);
    }
    match calc.screen() {
        Screen::Calc => draw_calc(&painter, view, calc, cursor_visible),
        Screen::StatEditor => draw_stat_editor(&painter, view, calc, cursor_visible),
        Screen::Table => draw_table(&painter, view, calc, cursor_visible),
    }
}

fn draw_indicators(painter: &Painter, view: View, calc: &Calculator) {
    let settings = calc.settings();
    let angle = match settings.angle {
        AngleUnit::Deg => "D",
        AngleUnit::Rad => "R",
        AngleUnit::Gra => "G",
    };
    let format = match settings.format {
        FormatSettings { number: NumberFormat::Fix(_), .. } => "FIX",
        FormatSettings { number: NumberFormat::Sci(_), .. } => "SCI",
        _ => "",
    };
    let on_calc = calc.screen() == Screen::Calc && calc.menu().is_none();
    let indicators = [
        (4.0, "S", calc.modifier() == Modifier::Shift),
        (16.0, "A", calc.modifier() == Modifier::Alpha),
        (28.0, "M", !calc.memory().get(Var::M).is_zero()),
        (42.0, "STO", calc.pending() == Pending::Sto),
        (66.0, "RCL", calc.pending() == Pending::Rcl),
        (92.0, "hyp", calc.pending() == Pending::Hyp),
        (124.0, angle, true),
        (140.0, format, !format.is_empty()),
        (166.0, "STAT", calc.mode() == CalcMode::Stat),
        (220.0, "Math", settings.format.natural),
        (260.0, "▲", on_calc && calc.history_len() > 0),
    ];
    for (dx, label, on) in indicators {
        if on {
            text(painter, view, dx, 2.0, label, INDICATOR_SIZE, Align2::LEFT_TOP, theme::LCD_INK);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn text(painter: &Painter, view: View, x: f32, y: f32, s: &str, size: f32, align: Align2, color: Color32) {
    let (gx, gy, ..) = GLASS;
    dots::paint_aligned(painter, view.pos(gx + x, gy + y), align, s, view.len(size), color);
}

/// A menu label, left-aligned and vertically centered on `y` (glass
/// coordinates). Drawn through the math renderer so x̄ and ŷ get their marks.
fn label(painter: &Painter, view: View, x: f32, y: f32, s: &str, size: f32) {
    let laid = layout(&label_nodes(s), size, &measure(painter, view));
    let (gx, gy, ..) = GLASS;
    let baseline = y + (laid.ascent - laid.descent) / 2.0;
    paint(painter, &laid, view.pos(gx + x, gy + baseline), view.len(1.0), theme::LCD_INK);
}

fn measure<'a>(painter: &'a Painter, view: View) -> EguiMeasure<'a> {
    EguiMeasure { painter, scale: view.len(1.0) }
}

// ---- Calculation screen ----

fn draw_calc(painter: &Painter, view: View, calc: &Calculator, cursor_visible: bool) {
    if let Phase::Error(err) = calc.phase() {
        return draw_error(painter, view, err.error, true);
    }
    let editing = matches!(calc.phase(), Phase::Editing);
    let area = glass_rect(view, 6.0, 15.0, GLASS.2 - 12.0, INPUT_HEIGHT);
    draw_editor(painter, view, area, calc.editor(), editing, editing && cursor_visible);
    if let Some(lines) = calc.result_lines() {
        let top = 15.0 + INPUT_HEIGHT;
        let area = glass_rect(view, 6.0, top, GLASS.2 - 12.0, GLASS.3 - top - 2.0);
        draw_result(painter, view, area, &lines, calc.settings().format.natural);
    }
}

/// Draws an input line. While `editing` it scrolls to follow the cursor;
/// otherwise it shows the start of the expression.
fn draw_editor(painter: &Painter, view: View, area: Rect, editor: &Editor, editing: bool, show_cursor: bool) {
    let nodes = input_nodes(editor.root(), editing.then(|| editor.cursor()));
    let laid = layout(&nodes, INPUT_SIZE, &measure(painter, view));
    let scale = view.len(1.0);

    // Scroll sideways to keep the cursor in view.
    let cursor = laid.cursor.unwrap_or(CursorBar { x: 0.0, top: -laid.ascent, bottom: 0.0 });
    let margin = 8.0;
    let width = area.width() / scale;
    let offset = (cursor.x - (width - margin)).max(0.0);

    // Top-aligned, unless the expression is taller than the line: then
    // keep the cursor's row visible.
    let height = area.height() / scale;
    let mut baseline = (laid.ascent + 1.0).max(INPUT_SIZE * 1.15);
    if baseline + laid.descent > height {
        baseline = baseline.min(height - cursor.bottom - 1.0).max(-cursor.top + 1.0);
    }
    let origin = area.min + vec2(-offset, baseline) * scale;
    let clipped = painter.with_clip_rect(area);
    paint(&clipped, &laid, origin, scale, theme::LCD_INK);
    if show_cursor {
        paint_cursor(&clipped, &laid, origin, scale, theme::LCD_INK);
    }
}

/// Lays out `nodes` as large as fits in `area` (shrinking from `size`) and
/// paints them right-aligned, at the bottom or vertically centered.
#[allow(clippy::too_many_arguments)]
fn fit(painter: &Painter, view: View, area: Rect, nodes: &[Node], size: f32, min: f32, bottom: bool, color: Color32) {
    let scale = view.len(1.0);
    let (width, height) = (area.width() / scale, area.height() / scale);
    let mut size = size;
    let mut laid = layout(nodes, size, &measure(painter, view));
    while (laid.width > width || laid.ascent + laid.descent > height) && size > min {
        size -= 0.5;
        laid = layout(nodes, size, &measure(painter, view));
    }
    let baseline = if bottom { height - laid.descent } else { (height + laid.ascent - laid.descent) / 2.0 };
    let origin = area.min + vec2(width - laid.width, baseline) * scale;
    paint(&painter.with_clip_rect(area), &laid, origin, scale, color);
}

fn draw_result(painter: &Painter, view: View, area: Rect, lines: &[ResultLine], natural: bool) {
    fit(painter, view, area, &result_nodes(lines, natural), RESULT_SIZE, MIN_RESULT_SIZE, true, theme::LCD_INK);
}

fn draw_error(painter: &Painter, view: View, error: CalcError, goto: bool) {
    let center = GLASS.2 / 2.0;
    text(painter, view, center, 38.0, &error.to_string(), 20.0, Align2::CENTER_CENTER, theme::LCD_INK);
    text(painter, view, 60.0, 70.0, "[AC]  :Cancel", 12.0, Align2::LEFT_CENTER, theme::LCD_INK);
    if goto {
        text(painter, view, 60.0, 88.0, "[◀][▶]:Goto", 12.0, Align2::LEFT_CENTER, theme::LCD_INK);
    }
}

// ---- Menus ----

fn draw_menu(painter: &Painter, view: View, menu: &MenuView) {
    let ink = theme::LCD_INK;
    if let Some(prompt) = menu.prompt {
        text(painter, view, 10.0, 52.0, prompt, 18.0, Align2::LEFT_CENTER, ink);
        return;
    }
    let mut top = 26.0;
    if let Some(title) = menu.title {
        text(painter, view, 8.0, top, title, MENU_SIZE, Align2::LEFT_CENTER, ink);
        top += 20.0;
    }
    let column_x = [8.0, GLASS.2 / 2.0 + 4.0];
    for (i, item) in menu.items.iter().enumerate() {
        let y = top + (i / 2) as f32 * 20.0;
        label(painter, view, column_x[i % 2], y, item, MENU_SIZE);
    }
    let right = GLASS.2 - 6.0;
    if menu.more_above {
        text(painter, view, right, 26.0, "▲", 10.0, Align2::RIGHT_CENTER, ink);
    }
    if menu.more_below {
        text(painter, view, right, GLASS.3 - 10.0, "▼", 10.0, Align2::RIGHT_CENTER, ink);
    }
}

// ---- Tables (STAT editor and TABLE view) ----

/// A cell as the editors show it: always decimal, shrunk to fit.
fn cell_nodes(value: &Value, number: NumberFormat) -> Vec<Node> {
    formatted_nodes(&Formatted::Decimal(format_decimal(value.to_f64(), number)), true)
}

struct Grid<'a> {
    headers: Vec<&'static str>,
    /// Rows to draw: (row number, cells), with `None` for a blank cell.
    rows: Vec<(usize, Vec<Option<Vec<Node>>>)>,
    selected: (usize, usize),
    painter: &'a Painter,
}

const GRID_TOP: f32 = 15.0;
const GRID_ROW: f32 = 15.0;
const VISIBLE_ROWS: usize = 3;

/// The first row to show so that `selected` stays in view.
fn first_visible(selected: usize) -> usize {
    selected.saturating_sub(VISIBLE_ROWS - 1)
}

fn draw_grid(view: View, grid: &Grid<'_>) {
    let painter = grid.painter;
    let ink = theme::LCD_INK;
    let (left, width) = (28.0, GLASS.2 - 32.0);
    let column_w = width / grid.headers.len() as f32;
    for (c, header) in grid.headers.iter().enumerate() {
        let x = left + column_w * (c as f32 + 0.5);
        text(painter, view, x, GRID_TOP + 6.0, header, 10.0, Align2::CENTER_CENTER, ink);
    }
    for (i, (number, cells)) in grid.rows.iter().enumerate() {
        let y = GRID_TOP + 13.0 + i as f32 * GRID_ROW;
        // Nudged up to line up with the cells' baselines.
        let number_y = y + GRID_ROW / 2.0 - 1.5;
        text(painter, view, 4.0, number_y, &number.to_string(), 10.0, Align2::LEFT_CENTER, ink);
        for (c, cell) in cells.iter().enumerate() {
            let rect = glass_rect(view, left + column_w * c as f32 + 1.0, y, column_w - 2.0, GRID_ROW - 1.0);
            let highlighted = (*number - 1, c) == grid.selected;
            let color = if highlighted {
                painter.rect_filled(rect, view.len(1.5), ink);
                theme::LCD_BG
            } else {
                ink
            };
            if let Some(nodes) = cell {
                let inner = rect.shrink2(vec2(view.len(3.0), view.len(1.0)));
                fit(painter, view, inner, nodes, CELL_SIZE, 7.0, false, color);
            }
        }
    }
}

/// Top of the line under a grid, in glass coordinates.
const BOTTOM_TOP: f32 = GRID_TOP + 13.0 + VISIBLE_ROWS as f32 * GRID_ROW + 2.0;

/// The line under a grid: what's being typed, or the full value of the cell.
fn bottom_area(view: View) -> Rect {
    glass_rect(view, 6.0, BOTTOM_TOP, GLASS.2 - 12.0, GLASS.3 - BOTTOM_TOP - 2.0)
}

fn draw_stat_editor(painter: &Painter, view: View, calc: &Calculator, cursor_visible: bool) {
    let stats = calc.stats();
    let ed = calc.stat_editor();
    let columns = stats.columns();
    let number = calc.settings().format.number;
    let first = first_visible(ed.row);
    let rows = (first..first + VISIBLE_ROWS)
        .filter(|&r| r <= stats.rows.len() && r < calc_core::stats::MAX_ROWS)
        .map(|r| (r + 1, columns.iter().map(|&c| stats.get(r, c).map(|v| cell_nodes(v, number))).collect()))
        .collect();
    let headers = columns.iter().map(|c| c.label()).collect();
    draw_grid(view, &Grid { headers, rows, selected: (ed.row, ed.column), painter });

    let area = bottom_area(view);
    if let Some(error) = ed.error {
        let y = (BOTTOM_TOP + GLASS.3) / 2.0;
        text(painter, view, GLASS.2 - 8.0, y, &error.to_string(), 13.0, Align2::RIGHT_CENTER, theme::LCD_INK);
    } else if !ed.input.is_empty() {
        draw_editor(painter, view, area, &ed.input, true, cursor_visible);
    } else if let Some(value) = columns.get(ed.column).and_then(|&c| stats.get(ed.row, c)) {
        let nodes = formatted_nodes(&calc.format(value), calc.settings().format.natural);
        fit(painter, view, area, &nodes, 16.0, 9.0, false, theme::LCD_INK);
    }
}

fn draw_table(painter: &Painter, view: View, calc: &Calculator, cursor_visible: bool) {
    let table = calc.table();
    if let Some(error) = table.error {
        return draw_error(painter, view, error, false);
    }
    let ink = theme::LCD_INK;
    let natural = calc.settings().format.natural;
    match table.stage {
        TableStage::Function => {
            text(painter, view, 6.0, 24.0, table.stage.prompt(), 14.0, Align2::LEFT_CENTER, ink);
            let area = glass_rect(view, 6.0, 34.0, GLASS.2 - 12.0, INPUT_HEIGHT);
            draw_editor(painter, view, area, calc.editor(), true, cursor_visible);
        }
        TableStage::View => {
            let number = calc.settings().format.number;
            let first = first_visible(table.selected);
            let rows = table
                .rows
                .iter()
                .enumerate()
                .skip(first)
                .take(VISIBLE_ROWS)
                .map(|(i, row)| {
                    let fx = match &row.fx {
                        Ok(v) => cell_nodes(v, number),
                        Err(_) => vec![Node::Text("ERROR".into())],
                    };
                    (i + 1, vec![Some(cell_nodes(&row.x, number)), Some(fx)])
                })
                .collect();
            draw_grid(
                view,
                &Grid { headers: vec!["X", "F(X)"], rows, selected: (table.selected, table.column), painter },
            );
            let Some(row) = table.rows.get(table.selected) else { return };
            let nodes = match (table.column, &row.fx) {
                (0, _) => formatted_nodes(&calc.format(&row.x), natural),
                (_, Ok(v)) => formatted_nodes(&calc.format(v), natural),
                (_, Err(e)) => vec![Node::Text(e.to_string())],
            };
            fit(painter, view, bottom_area(view), &nodes, 16.0, 9.0, false, ink);
        }
        stage => {
            text(painter, view, 6.0, 24.0, "Table Range", 13.0, Align2::LEFT_CENTER, ink);
            text(painter, view, 6.0, 44.0, stage.prompt(), 14.0, Align2::LEFT_CENTER, ink);
            let area = glass_rect(view, 6.0, 56.0, GLASS.2 - 12.0, GLASS.3 - 58.0);
            let editor = calc.editor();
            if editor.is_empty() {
                // Show the current setting; typing replaces it.
                if let Some(value) = table.range_value(stage) {
                    let nodes = formatted_nodes(&calc.format(value), natural);
                    fit(painter, view, area, &nodes, 16.0, 9.0, false, ink);
                }
            } else {
                draw_editor(painter, view, area, editor, true, cursor_visible);
            }
        }
    }
}

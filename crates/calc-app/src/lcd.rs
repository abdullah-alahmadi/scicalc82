//! The display: status indicators, the input line and the result line.

use calc_core::calculator::{Pending, Phase, ResultLine};
use calc_core::format::{FormatSettings, NumberFormat};
use calc_core::input::Var;
use calc_core::key::Modifier;
use calc_core::state::AngleUnit;
use calc_core::{CalcError, Calculator};
use eframe::egui::{Align2, Painter, vec2};

use crate::math::layout::{CursorBar, layout};
use crate::math::nodes::{input_nodes, result_nodes};
use crate::math::paint::{EguiMeasure, paint, paint_cursor};
use crate::theme::{self, View};

const INPUT_SIZE: f32 = 17.0;
/// Height of the input area in design units; the result area fills the rest.
const INPUT_HEIGHT: f32 = 48.0;
const RESULT_SIZE: f32 = 20.0;
const MIN_RESULT_SIZE: f32 = 11.0;
const INDICATOR_SIZE: f32 = 9.5;

/// The glass area in design coordinates.
pub const GLASS: (f32, f32, f32, f32) = (34.0, 64.0, 292.0, 112.0);

pub fn draw(painter: &Painter, view: View, calc: &Calculator, cursor_visible: bool) {
    let (x, y, w, h) = GLASS;
    painter.rect_filled(view.rect(x - 10.0, y - 10.0, w + 20.0, h + 20.0), view.len(10.0), theme::BEZEL);
    let glass = view.rect(x, y, w, h);
    painter.rect_filled(glass, view.len(4.0), theme::LCD_BG);
    let painter = painter.with_clip_rect(glass);

    draw_indicators(&painter, view, calc);
    if let Phase::Error(err) = calc.phase() {
        draw_error(&painter, view, err.error);
        return;
    }
    let editing = matches!(calc.phase(), Phase::Editing);
    draw_input(&painter, view, calc, editing, cursor_visible);
    match calc.pending() {
        Pending::DrgMenu => draw_menu(&painter, view, "1:°   2:r   3:g"),
        Pending::ClrMenu => draw_menu(&painter, view, "CLR 1:Setup 2:Mem 3:All"),
        _ => {
            if let Some(lines) = calc.result_lines() {
                draw_result(&painter, view, &lines);
            }
        }
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
    let indicators = [
        (4.0, "S", calc.modifier() == Modifier::Shift),
        (16.0, "A", calc.modifier() == Modifier::Alpha),
        (28.0, "M", !calc.memory().get(Var::M).is_zero()),
        (42.0, "STO", calc.pending() == Pending::Sto),
        (66.0, "RCL", calc.pending() == Pending::Rcl),
        (92.0, "hyp", calc.pending() == Pending::Hyp),
        (124.0, angle, true),
        (140.0, format, !format.is_empty()),
        (220.0, "Math", true),
        (260.0, "▲", calc.history_len() > 0),
    ];
    let (gx, gy, ..) = GLASS;
    for (dx, label, on) in indicators {
        if on {
            painter.text(
                view.pos(gx + dx, gy + 2.0),
                Align2::LEFT_TOP,
                label,
                view.lcd_font(INDICATOR_SIZE),
                theme::LCD_INK,
            );
        }
    }
}

fn measure<'a>(painter: &'a Painter, view: View) -> EguiMeasure<'a> {
    EguiMeasure { painter, scale: view.len(1.0) }
}

/// Draws the input. While editing it scrolls to follow the cursor; otherwise
/// (a result is showing) it shows the start of the expression.
fn draw_input(painter: &Painter, view: View, calc: &Calculator, editing: bool, cursor_visible: bool) {
    let (gx, gy, gw, _) = GLASS;
    let area = view.rect(gx + 6.0, gy + 15.0, gw - 12.0, INPUT_HEIGHT);
    let editor = calc.editor();
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
    if editing && cursor_visible {
        paint_cursor(&clipped, &laid, origin, scale, theme::LCD_INK);
    }
}

fn draw_result(painter: &Painter, view: View, lines: &[ResultLine]) {
    let (gx, gy, gw, gh) = GLASS;
    let top = gy + 15.0 + INPUT_HEIGHT;
    let area = view.rect(gx + 6.0, top, gw - 12.0, gy + gh - top - 2.0);
    let scale = view.len(1.0);
    let (width, height) = (area.width() / scale, area.height() / scale);
    let nodes = result_nodes(lines);
    // Shrink long or tall results (such as Pol/Rec pairs) until they fit.
    let mut size = RESULT_SIZE;
    let mut laid = layout(&nodes, size, &measure(painter, view));
    while (laid.width > width || laid.ascent + laid.descent > height) && size > MIN_RESULT_SIZE {
        size -= 1.0;
        laid = layout(&nodes, size, &measure(painter, view));
    }
    let origin = area.max - vec2(laid.width, laid.descent) * scale;
    paint(painter, &laid, origin, scale, theme::LCD_INK);
}

fn draw_menu(painter: &Painter, view: View, text: &str) {
    let (gx, gy, _, gh) = GLASS;
    painter.text(view.pos(gx + 8.0, gy + gh - 28.0), Align2::LEFT_CENTER, text, view.lcd_font(14.0), theme::LCD_INK);
}

fn draw_error(painter: &Painter, view: View, error: CalcError) {
    let (gx, gy, gw, _) = GLASS;
    let center_x = gx + gw / 2.0;
    painter.text(
        view.pos(center_x, gy + 38.0),
        Align2::CENTER_CENTER,
        error.to_string(),
        view.lcd_font(20.0),
        theme::LCD_INK,
    );
    let hints = view.lcd_font(12.0);
    let left = view.pos(gx + 60.0, gy + 70.0);
    painter.text(left, Align2::LEFT_CENTER, "[AC]  :Cancel", hints.clone(), theme::LCD_INK);
    painter.text(left + vec2(0.0, view.len(18.0)), Align2::LEFT_CENTER, "[◀][▶]:Goto", hints, theme::LCD_INK);
}

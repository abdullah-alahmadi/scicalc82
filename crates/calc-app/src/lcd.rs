//! The display: status indicators, the input line and the result line.

use calc_core::calculator::{Pending, Phase, ResultLine};
use calc_core::format::{FormatSettings, Formatted, NumberFormat};
use calc_core::input::Var;
use calc_core::key::Modifier;
use calc_core::state::AngleUnit;
use calc_core::{CalcError, Calculator};
use eframe::egui::{Align2, Painter, Stroke, pos2, vec2};

use crate::linear;
use crate::rich::{self, Script, Span};
use crate::theme::{self, View};

const INPUT_SIZE: f32 = 19.0;
const RESULT_SIZE: f32 = 24.0;
const MIN_RESULT_SIZE: f32 = 13.0;
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
    draw_input(&painter, view, calc, editing && cursor_visible);
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

fn draw_input(painter: &Painter, view: View, calc: &Calculator, show_cursor: bool) {
    let (gx, gy, gw, _) = GLASS;
    let area = view.rect(gx + 6.0, gy + 20.0, gw - 12.0, 30.0);
    let editor = calc.editor();
    let line = linear::line(editor.root(), Some(editor.cursor()));
    let font = view.lcd_font(INPUT_SIZE);
    let before = rich::layout(painter, &line.before, &font, theme::LCD_INK);
    let all: Vec<Span> = line.before.iter().chain(&line.after).cloned().collect();
    let full = rich::layout(painter, &all, &font, theme::LCD_INK);

    // Scroll horizontally so the cursor stays in view.
    let cursor_x = before.size().x;
    let margin = view.len(10.0);
    let offset = (cursor_x - (area.width() - margin)).max(0.0);
    let origin = pos2(area.left() - offset, area.center().y - full.size().y / 2.0);
    painter.with_clip_rect(area).galley(origin, full.clone(), theme::LCD_INK);

    if show_cursor {
        // Sized from the font rather than the text, which may be empty.
        let x = origin.x + cursor_x;
        let half = view.len(INPUT_SIZE) * 0.5;
        let (top, bottom) = (area.center().y - half, area.center().y + half);
        painter.line_segment([pos2(x, top), pos2(x, bottom)], Stroke::new(view.len(1.6), theme::LCD_INK));
    }
}

fn formatted_spans(f: &Formatted) -> Vec<Span> {
    match f {
        Formatted::Decimal(d) => {
            let sign = if d.negative { "-" } else { "" };
            let mut spans = vec![Span::new(format!("{sign}{}", d.mantissa), Script::Normal)];
            if let Some(e) = d.exponent {
                spans.push(Span::new("×10", Script::Normal));
                spans.push(Span::new(e.to_string(), Script::Super));
            }
            spans
        }
        other => vec![Span::new(other.to_string(), Script::Normal)],
    }
}

fn result_spans(lines: &[ResultLine]) -> Vec<Span> {
    let mut spans = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if i > 0 {
            spans.push(Span::new(",  ", Script::Normal));
        }
        if let Some(label) = line.label {
            spans.push(Span::new(format!("{label}="), Script::Normal));
        }
        spans.extend(formatted_spans(&line.value));
    }
    spans
}

fn draw_result(painter: &Painter, view: View, lines: &[ResultLine]) {
    let (gx, gy, gw, gh) = GLASS;
    let area = view.rect(gx + 6.0, gy + 56.0, gw - 12.0, gh - 60.0);
    let spans = result_spans(lines);
    // Shrink long results (such as Pol/Rec pairs) to fit the line.
    let mut size = RESULT_SIZE;
    let mut galley = rich::layout(painter, &spans, &view.lcd_font(size), theme::LCD_INK);
    while galley.size().x > area.width() && size > MIN_RESULT_SIZE {
        size -= 1.0;
        galley = rich::layout(painter, &spans, &view.lcd_font(size), theme::LCD_INK);
    }
    let pos = pos2(area.right() - galley.size().x, area.center().y - galley.size().y / 2.0);
    painter.galley(pos, galley, theme::LCD_INK);
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

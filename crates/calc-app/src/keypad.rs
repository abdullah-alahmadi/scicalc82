//! The keys: layout, legends, drawing and clicks.

use calc_core::Calculator;
use calc_core::key::{Key, Modifier};
use eframe::egui::{Align2, Color32, Pos2, Rect, Sense, Shape, Stroke, StrokeKind, Ui, pos2, vec2};

use crate::rich;
use crate::theme::{self, View};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Style {
    Control,
    Func,
    Num,
    Accent,
}

/// One key. Labels use the markup in [`rich::markup`].
#[derive(Clone, Copy)]
struct KeySpec {
    key: Key,
    label: &'static str,
    shift: Option<&'static str>,
    alpha: Option<&'static str>,
    style: Style,
}

const fn spec(
    key: Key,
    label: &'static str,
    shift: Option<&'static str>,
    alpha: Option<&'static str>,
    style: Style,
) -> Option<KeySpec> {
    Some(KeySpec { key, label, shift, alpha, style })
}

const fn func(
    key: Key,
    label: &'static str,
    shift: Option<&'static str>,
    alpha: Option<&'static str>,
) -> Option<KeySpec> {
    spec(key, label, shift, alpha, Style::Func)
}

const fn num(
    key: Key,
    label: &'static str,
    shift: Option<&'static str>,
    alpha: Option<&'static str>,
) -> Option<KeySpec> {
    spec(key, label, shift, alpha, Style::Num)
}

/// The upper block: six small keys per row; the cursor pad takes the
/// middle of the first two rows.
const UPPER: [[Option<KeySpec>; 6]; 5] = [
    [
        spec(Key::Shift, "SHIFT", None, None, Style::Control),
        spec(Key::Alpha, "ALPHA", None, None, Style::Control),
        None,
        None,
        spec(Key::Mode, "MODE", Some("SETUP"), None, Style::Control),
        spec(Key::On, "ON", None, None, Style::Control),
    ],
    [
        func(Key::Abs, "Abs", None, None),
        func(Key::Cube, "x³", None, None),
        None,
        None,
        func(Key::Recip, "x^−1", Some("x!"), None),
        func(Key::LogBase, "log_□|□", None, None),
    ],
    [
        func(Key::Fraction, "□⁄□", Some("□□⁄□"), None),
        func(Key::Sqrt, "√□", Some("∛□"), None),
        func(Key::Square, "x²", None, None),
        func(Key::Power, "x^□", Some("^x|√□"), None),
        func(Key::Log, "log", Some("10^x"), None),
        func(Key::Ln, "ln", Some("e^x"), None),
    ],
    [
        func(Key::Neg, "(−)", None, Some("A")),
        func(Key::Dms, "°′″", None, Some("B")),
        func(Key::Hyp, "hyp", None, Some("C")),
        func(Key::Sin, "sin", Some("sin^−1"), Some("D")),
        func(Key::Cos, "cos", Some("cos^−1"), Some("E")),
        func(Key::Tan, "tan", Some("tan^−1"), Some("F")),
    ],
    [
        func(Key::Rcl, "RCL", Some("STO"), None),
        func(Key::Eng, "ENG", Some("←"), None),
        func(Key::LParen, "(", Some("%"), None),
        func(Key::RParen, ")", Some(","), Some("X")),
        func(Key::SToD, "S⇔D", None, Some("Y")),
        func(Key::MPlus, "M+", Some("M−"), Some("M")),
    ],
];

/// The lower block: five large keys per row.
const LOWER: [[Option<KeySpec>; 5]; 4] = [
    [
        num(Key::Digit(7), "7", None, None),
        num(Key::Digit(8), "8", None, None),
        num(Key::Digit(9), "9", Some("CLR"), None),
        spec(Key::Del, "DEL", None, None, Style::Accent),
        spec(Key::Ac, "AC", None, None, Style::Accent),
    ],
    [
        num(Key::Digit(4), "4", None, None),
        num(Key::Digit(5), "5", None, None),
        num(Key::Digit(6), "6", None, None),
        num(Key::Times, "×", Some("nPr"), None),
        num(Key::Divide, "÷", Some("nCr"), None),
    ],
    [
        num(Key::Digit(1), "1", Some("STAT"), None),
        num(Key::Digit(2), "2", None, None),
        num(Key::Digit(3), "3", None, None),
        num(Key::Plus, "+", Some("Pol"), None),
        num(Key::Minus, "−", Some("Rec"), None),
    ],
    [
        num(Key::Digit(0), "0", Some("Rnd"), None),
        num(Key::Point, ".", Some("Ran#"), Some("RanInt")),
        num(Key::Exp10, "×10^x", Some("π"), Some("e")),
        num(Key::Ans, "Ans", Some("DRG▸"), None),
        num(Key::Equals, "=", Some("≈"), None),
    ],
];

// Design-space geometry.
const LEFT: f32 = 20.0;
const WIDTH: f32 = 320.0;
const LEGEND: f32 = 13.0;
const UPPER_TOPS: [f32; 5] = [194.0, 236.0, 278.0, 320.0, 362.0];
const UPPER_KEY: (f32, f32) = (44.0, 24.0);
const LOWER_TOPS: [f32; 4] = [410.0, 464.0, 518.0, 572.0];
const LOWER_KEY: (f32, f32) = (52.0, 34.0);

fn key_rect(view: View, row_top: f32, col: usize, cols: usize, size: (f32, f32)) -> Rect {
    let col_w = WIDTH / cols as f32;
    let x = LEFT + col_w * col as f32 + (col_w - size.0) / 2.0;
    view.rect(x, row_top + LEGEND, size.0, size.1)
}

fn pad_rect(view: View) -> Rect {
    let col_w = WIDTH / 6.0;
    let (x0, x1) = (LEFT + 2.0 * col_w + 4.0, LEFT + 4.0 * col_w - 4.0);
    let (y0, y1) = (UPPER_TOPS[0] + 8.0, UPPER_TOPS[1] + LEGEND + UPPER_KEY.1 + 2.0);
    view.rect(x0, y0, x1 - x0, y1 - y0)
}

/// Draws the keypad and returns the keys clicked this frame. `flash` is a
/// key pressed from the keyboard, drawn as held down.
pub fn draw(ui: &Ui, view: View, calc: &Calculator, flash: Option<Key>) -> Vec<Key> {
    let mut clicked = Vec::new();
    for (r, row) in UPPER.iter().enumerate() {
        for (c, spec) in row.iter().enumerate() {
            if let Some(spec) = spec {
                let rect = key_rect(view, UPPER_TOPS[r], c, 6, UPPER_KEY);
                draw_key(ui, view, spec, rect, calc, flash, &mut clicked);
            }
        }
    }
    for (r, row) in LOWER.iter().enumerate() {
        for (c, spec) in row.iter().enumerate() {
            if let Some(spec) = spec {
                let rect = key_rect(view, LOWER_TOPS[r], c, 5, LOWER_KEY);
                draw_key(ui, view, spec, rect, calc, flash, &mut clicked);
            }
        }
    }
    draw_pad(ui, view, flash, &mut clicked);
    clicked
}

fn draw_key(
    ui: &Ui,
    view: View,
    spec: &KeySpec,
    rect: Rect,
    calc: &Calculator,
    flash: Option<Key>,
    clicked: &mut Vec<Key>,
) {
    let response = ui.interact(rect, ui.id().with(("key", spec.key)), Sense::click());
    if response.clicked() {
        clicked.push(spec.key);
    }
    let held = response.is_pointer_button_down_on() || flash == Some(spec.key);
    let painter = ui.painter();
    let radius = view.len(if spec.style == Style::Num || spec.style == Style::Accent { 7.0 } else { 6.0 });

    // Shadow below, key face on top; a held key sinks into its shadow.
    painter.rect_filled(rect.translate(vec2(0.0, view.len(2.5))), radius, theme::KEY_SHADOW);
    let face = if held { rect.translate(vec2(0.0, view.len(1.5))) } else { rect };
    let (fill, text) = match spec.style {
        Style::Control => (theme::KEY_CONTROL, theme::KEY_TEXT_LIGHT),
        Style::Func => (theme::KEY_FUNC, theme::KEY_TEXT_LIGHT),
        Style::Num => (theme::KEY_NUM, theme::KEY_TEXT_DARK),
        Style::Accent => (theme::KEY_ACCENT, theme::KEY_TEXT_LIGHT),
    };
    let fill = if held {
        shade(fill, -22)
    } else if response.hovered() {
        lighten(fill)
    } else {
        fill
    };
    painter.rect_filled(face, radius, fill);

    // SHIFT and ALPHA light up while active.
    let active = match spec.key {
        Key::Shift => Some((calc.modifier() == Modifier::Shift, theme::SHIFT_LEGEND)),
        Key::Alpha => Some((calc.modifier() == Modifier::Alpha, theme::ALPHA_LEGEND)),
        _ => None,
    };
    let text = match active {
        Some((true, color)) => {
            painter.rect_stroke(face, radius, Stroke::new(view.len(2.0), color), StrokeKind::Inside);
            color
        }
        Some((false, color)) => color,
        None => text,
    };

    let chars = spec.label.chars().filter(|c| !"^_|".contains(*c)).count();
    let size = match (spec.style, chars) {
        (Style::Control, _) => 9.0,
        (Style::Num | Style::Accent, 0..=2) => 19.0,
        (Style::Num | Style::Accent, _) => 13.0,
        (_, 0..=3) => 13.0,
        _ => 11.0,
    };
    let galley = rich::layout(painter, &rich::markup(spec.label), &view.label_font(size), text);
    painter.galley(face.center() - galley.size() / 2.0, galley, text);

    let legend_font = view.label_font(9.5);
    let legend_y = rect.top() - view.len(2.0);
    let legend = |markup: &str, anchor: Pos2, align: Align2, color: Color32| {
        let galley = rich::layout(painter, &rich::markup(markup), &legend_font, color);
        let pos = align.anchor_size(anchor, galley.size()).min;
        painter.galley(pos, galley, color);
    };
    match (spec.shift, spec.alpha) {
        (Some(s), Some(a)) => {
            legend(s, pos2(rect.left() - view.len(2.0), legend_y), Align2::LEFT_BOTTOM, theme::SHIFT_LEGEND);
            legend(a, pos2(rect.right() + view.len(2.0), legend_y), Align2::RIGHT_BOTTOM, theme::ALPHA_LEGEND);
        }
        (Some(s), None) => legend(s, pos2(rect.center().x, legend_y), Align2::CENTER_BOTTOM, theme::SHIFT_LEGEND),
        (None, Some(a)) => legend(a, pos2(rect.right(), legend_y), Align2::RIGHT_BOTTOM, theme::ALPHA_LEGEND),
        (None, None) => {}
    }
}

fn shade(c: Color32, delta: i16) -> Color32 {
    let adjust = |v: u8| (i16::from(v) + delta).clamp(0, 255) as u8;
    Color32::from_rgb(adjust(c.r()), adjust(c.g()), adjust(c.b()))
}

fn lighten(c: Color32) -> Color32 {
    shade(c, 18)
}

/// The REPLAY pad: one control with four arrow zones.
fn draw_pad(ui: &Ui, view: View, flash: Option<Key>, clicked: &mut Vec<Key>) {
    let rect = pad_rect(view);
    let response = ui.interact(rect, ui.id().with("replay-pad"), Sense::click());
    let direction = |p: Pos2| {
        let d = p - rect.center();
        let (nx, ny) = (d.x / rect.width(), d.y / rect.height());
        if nx.abs() > ny.abs() {
            if nx < 0.0 { Key::Left } else { Key::Right }
        } else if ny < 0.0 {
            Key::Up
        } else {
            Key::Down
        }
    };
    if response.clicked()
        && let Some(p) = response.interact_pointer_pos()
    {
        clicked.push(direction(p));
    }
    let held = if response.is_pointer_button_down_on() {
        response.interact_pointer_pos().map(direction)
    } else {
        flash.filter(|k| matches!(k, Key::Left | Key::Right | Key::Up | Key::Down))
    };

    let painter = ui.painter();
    let radius = view.len(30.0);
    painter.rect_filled(rect.translate(vec2(0.0, view.len(2.5))), radius, theme::KEY_SHADOW);
    painter.rect_filled(rect, radius, if response.hovered() { lighten(theme::PAD) } else { theme::PAD });
    painter.text(rect.center(), Align2::CENTER_CENTER, "REPLAY", view.label_font(7.5), theme::BEZEL);

    let c = rect.center();
    let (hx, hy) = (rect.width() / 2.0 - view.len(12.0), rect.height() / 2.0 - view.len(10.0));
    let s = view.len(6.0);
    let arrows = [
        (Key::Up, [pos2(c.x, c.y - hy - s), pos2(c.x - s, c.y - hy + s * 0.4), pos2(c.x + s, c.y - hy + s * 0.4)]),
        (Key::Down, [pos2(c.x, c.y + hy + s), pos2(c.x + s, c.y + hy - s * 0.4), pos2(c.x - s, c.y + hy - s * 0.4)]),
        (Key::Left, [pos2(c.x - hx - s, c.y), pos2(c.x - hx + s * 0.4, c.y + s), pos2(c.x - hx + s * 0.4, c.y - s)]),
        (Key::Right, [pos2(c.x + hx + s, c.y), pos2(c.x + hx - s * 0.4, c.y - s), pos2(c.x + hx - s * 0.4, c.y + s)]),
    ];
    for (key, points) in arrows {
        let color = if held == Some(key) { theme::SHIFT_LEGEND } else { theme::PAD_ARROW };
        painter.add(Shape::convex_polygon(points.to_vec(), color, Stroke::NONE));
    }
}

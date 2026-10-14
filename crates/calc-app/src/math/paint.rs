//! Drawing laid-out math with egui.

use eframe::egui::{Color32, Painter, Pos2, Shape, Stroke, StrokeKind, pos2};

use super::layout::{Laid, Measure, Prim};
use crate::dots;

/// Measures with the LCD's dot-matrix font at the current screen scale.
pub struct EguiMeasure<'a> {
    pub painter: &'a Painter,
    /// Screen points per design unit.
    pub scale: f32,
}

impl Measure for EguiMeasure<'_> {
    fn width(&self, text: &str, size: f32) -> f32 {
        dots::width(self.painter, text, size * self.scale) / self.scale
    }
}

/// Paints `laid` with its baseline starting at `origin` (screen points).
pub fn paint(painter: &Painter, laid: &Laid, origin: Pos2, scale: f32, color: Color32) {
    let at = |p: Pos2| origin + p.to_vec2() * scale;
    for prim in &laid.prims {
        match prim {
            Prim::Text { pos, text, size } => dots::paint(painter, at(*pos), text, size * scale, color),
            Prim::Line { from, to, width } => {
                painter.line_segment([at(*from), at(*to)], Stroke::new(width * scale, color));
            }
            Prim::Path { points, width } => {
                let points = points.iter().map(|p| at(*p)).collect();
                painter.add(Shape::line(points, Stroke::new(width * scale, color)));
            }
            Prim::SlotBox { rect, width } => {
                let screen = eframe::egui::Rect::from_min_max(at(rect.min), at(rect.max));
                painter.rect_stroke(
                    screen,
                    0.0,
                    Stroke::new(width * scale, color.gamma_multiply(0.75)),
                    StrokeKind::Middle,
                );
            }
        }
    }
}

/// Draws the cursor bar of `laid`, if it has one.
pub fn paint_cursor(painter: &Painter, laid: &Laid, origin: Pos2, scale: f32, color: Color32) {
    if let Some(c) = laid.cursor {
        let x = origin.x + c.x * scale;
        let (top, bottom) = (origin.y + c.top * scale, origin.y + c.bottom * scale);
        painter.line_segment([pos2(x, top), pos2(x, bottom)], Stroke::new(1.6 * scale, color));
    }
}

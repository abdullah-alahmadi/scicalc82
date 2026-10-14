//! Drawing laid-out math with egui.

use eframe::egui::{Color32, FontId, Painter, Pos2, Shape, Stroke, StrokeKind, pos2, vec2};

use super::layout::{Laid, Measure, Prim};

/// Measures with the LCD font at the current screen scale.
pub struct EguiMeasure<'a> {
    pub painter: &'a Painter,
    /// Screen points per design unit.
    pub scale: f32,
}

fn font(size: f32) -> FontId {
    FontId::monospace(size)
}

impl Measure for EguiMeasure<'_> {
    fn width(&self, text: &str, size: f32) -> f32 {
        let galley = self.painter.layout_no_wrap(text.to_owned(), font(size * self.scale), Color32::BLACK);
        galley.size().x / self.scale
    }
}

/// Paints `laid` with its baseline starting at `origin` (screen points).
pub fn paint(painter: &Painter, laid: &Laid, origin: Pos2, scale: f32, color: Color32) {
    let at = |p: Pos2| origin + p.to_vec2() * scale;
    for prim in &laid.prims {
        match prim {
            Prim::Text { pos, text, size } => {
                let galley = painter.layout_no_wrap(text.clone(), font(size * scale), color);
                // Align the glyph baseline, not the galley's top edge.
                let baseline = galley
                    .rows
                    .first()
                    .and_then(|row| row.row.glyphs.first().map(|g| row.pos.y + g.pos.y))
                    .unwrap_or(galley.size().y * 0.8);
                painter.galley(at(*pos) - vec2(0.0, baseline), galley, color);
            }
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

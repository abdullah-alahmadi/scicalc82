//! Text with superscript and subscript runs, laid out as one egui galley.

use std::sync::Arc;

use eframe::egui::text::{LayoutJob, TextFormat};
use eframe::egui::{Align, Color32, FontId, Galley, Painter};

/// Size of super- and subscripts relative to the base text.
const SCRIPT_SCALE: f32 = 0.62;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Script {
    Normal,
    Super,
    Sub,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    pub text: String,
    pub script: Script,
}

impl Span {
    pub fn new(text: impl Into<String>, script: Script) -> Self {
        Self { text: text.into(), script }
    }
}

/// Parses a key-label markup: `^` starts a superscript, `_` a subscript,
/// and `|` returns to normal text. `"x^-1"` is x with a raised −1.
pub fn markup(s: &str) -> Vec<Span> {
    let mut spans = Vec::new();
    let mut script = Script::Normal;
    let mut text = String::new();
    for c in s.chars() {
        let next = match c {
            '^' => Script::Super,
            '_' => Script::Sub,
            '|' => Script::Normal,
            _ => {
                text.push(c);
                continue;
            }
        };
        if !text.is_empty() {
            spans.push(Span::new(std::mem::take(&mut text), script));
        }
        script = next;
    }
    if !text.is_empty() {
        spans.push(Span::new(text, script));
    }
    spans
}

pub fn layout(painter: &Painter, spans: &[Span], font: &FontId, color: Color32) -> Arc<Galley> {
    let mut job = LayoutJob::default();
    for span in spans {
        let (size, valign) = match span.script {
            Script::Normal => (font.size, Align::Center),
            Script::Super => (font.size * SCRIPT_SCALE, Align::TOP),
            Script::Sub => (font.size * SCRIPT_SCALE, Align::BOTTOM),
        };
        let format = TextFormat {
            font_id: FontId::new(size, font.family.clone()),
            color,
            valign,
            line_height: Some(font.size * 1.2),
            ..Default::default()
        };
        job.append(&span.text, 0.0, format);
    }
    painter.layout_job(job)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_markup() {
        assert_eq!(markup("x^-1"), vec![Span::new("x", Script::Normal), Span::new("-1", Script::Super)]);
        assert_eq!(
            markup("log_□|□"),
            vec![Span::new("log", Script::Normal), Span::new("□", Script::Sub), Span::new("□", Script::Normal)]
        );
        assert_eq!(markup("^x|√□"), vec![Span::new("x", Script::Super), Span::new("√□", Script::Normal)]);
    }
}

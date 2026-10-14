//! Colors, fonts and the fixed design-space coordinate system.
//!
//! Everything is laid out in a 360×650 design space and scaled uniformly to
//! fit the window, so the calculator keeps its proportions at any size.

use eframe::egui::{Color32, FontFamily, FontId, Pos2, Rect, vec2};

pub const DESIGN_W: f32 = 360.0;
pub const DESIGN_H: f32 = 650.0;

pub const BACKDROP: Color32 = Color32::from_rgb(0x1b, 0x1d, 0x21);
pub const BODY: Color32 = Color32::from_rgb(0x2a, 0x2e, 0x35);
pub const FACEPLATE: Color32 = Color32::from_rgb(0x3a, 0x3f, 0x48);
pub const BRAND: Color32 = Color32::from_rgb(0xd8, 0xdc, 0xe2);
pub const BRAND_FAINT: Color32 = Color32::from_rgb(0x8f, 0x96, 0xa1);
pub const SOLAR: Color32 = Color32::from_rgb(0x3b, 0x2c, 0x27);
pub const SOLAR_LINE: Color32 = Color32::from_rgb(0x5a, 0x45, 0x3c);

pub const BEZEL: Color32 = Color32::from_rgb(0x1f, 0x22, 0x27);
pub const LCD_BG: Color32 = Color32::from_rgb(0xc4, 0xcf, 0xbc);
pub const LCD_INK: Color32 = Color32::from_rgb(0x1e, 0x25, 0x20);

pub const KEY_SHADOW: Color32 = Color32::from_rgb(0x14, 0x16, 0x1a);
pub const KEY_CONTROL: Color32 = Color32::from_rgb(0x4b, 0x52, 0x5d);
pub const KEY_FUNC: Color32 = Color32::from_rgb(0x22, 0x25, 0x2b);
pub const KEY_NUM: Color32 = Color32::from_rgb(0xe4, 0xe6, 0xea);
pub const KEY_ACCENT: Color32 = Color32::from_rgb(0xd9, 0x7a, 0x2b);
pub const KEY_TEXT_LIGHT: Color32 = Color32::from_rgb(0xf2, 0xf3, 0xf5);
pub const KEY_TEXT_DARK: Color32 = Color32::from_rgb(0x1f, 0x22, 0x27);
pub const SHIFT_LEGEND: Color32 = Color32::from_rgb(0xe6, 0xb4, 0x22);
pub const ALPHA_LEGEND: Color32 = Color32::from_rgb(0xef, 0x5b, 0x55);
pub const CMPLX_LEGEND: Color32 = Color32::from_rgb(0xb4, 0x8e, 0xf0);
pub const PAD: Color32 = Color32::from_rgb(0x55, 0x5c, 0x68);
pub const PAD_ARROW: Color32 = Color32::from_rgb(0xdd, 0xe0, 0xe5);

/// Maps design-space coordinates to screen points.
#[derive(Clone, Copy, Debug)]
pub struct View {
    origin: Pos2,
    scale: f32,
}

impl View {
    /// Fits the design space inside `available`, centered.
    pub fn fit(available: Rect) -> Self {
        let scale = (available.width() / DESIGN_W).min(available.height() / DESIGN_H);
        let size = vec2(DESIGN_W, DESIGN_H) * scale;
        Self { origin: available.center() - size / 2.0, scale }
    }

    pub fn pos(&self, x: f32, y: f32) -> Pos2 {
        self.origin + vec2(x, y) * self.scale
    }

    pub fn rect(&self, x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect::from_min_size(self.pos(x, y), vec2(w, h) * self.scale)
    }

    pub fn len(&self, v: f32) -> f32 {
        v * self.scale
    }

    pub fn label_font(&self, size: f32) -> FontId {
        FontId::new(self.len(size), FontFamily::Proportional)
    }
}

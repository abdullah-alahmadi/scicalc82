//! The window icon, drawn in code: a small calculator in the app's colors.

use eframe::egui::{Color32, IconData};

use crate::theme;

const SIZE: u32 = 64;
/// Samples per pixel along each axis, for smooth edges.
const SUPERSAMPLE: u32 = 4;

/// A rounded rectangle (x0, y0, x1, y1, radius) and its color.
type Shape = ((f32, f32, f32, f32, f32), Color32);

fn shapes() -> Vec<Shape> {
    let mut shapes = vec![
        ((6.0, 2.0, 58.0, 62.0, 9.0), theme::BODY),
        ((11.0, 8.0, 53.0, 25.0, 3.0), theme::LCD_BG),
        // A result on the display.
        ((32.0, 17.0, 34.5, 21.0, 0.5), theme::LCD_INK),
        ((36.5, 17.0, 39.0, 21.0, 0.5), theme::LCD_INK),
        ((41.0, 17.0, 48.0, 21.0, 0.5), theme::LCD_INK),
    ];
    for row in 0..3 {
        for col in 0..4 {
            let (x, y) = (12.0 + col as f32 * 10.5, 31.0 + row as f32 * 10.0);
            let color = if row == 0 && col == 3 { theme::KEY_ACCENT } else { theme::KEY_NUM };
            shapes.push(((x, y, x + 8.0, y + 7.0, 1.5), color));
        }
    }
    shapes
}

fn inside(x: f32, y: f32, (x0, y0, x1, y1, r): (f32, f32, f32, f32, f32)) -> bool {
    if x < x0 || x > x1 || y < y0 || y > y1 {
        return false;
    }
    let (cx, cy) = (x.clamp(x0 + r, x1 - r), y.clamp(y0 + r, y1 - r));
    (x - cx).powi(2) + (y - cy).powi(2) <= r * r
}

pub fn icon() -> IconData {
    let shapes = shapes();
    let mut rgba = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    let samples = (SUPERSAMPLE * SUPERSAMPLE) as f32;
    for py in 0..SIZE {
        for px in 0..SIZE {
            let (mut r, mut g, mut b, mut covered) = (0.0, 0.0, 0.0, 0.0);
            for sy in 0..SUPERSAMPLE {
                for sx in 0..SUPERSAMPLE {
                    let x = px as f32 + (sx as f32 + 0.5) / SUPERSAMPLE as f32;
                    let y = py as f32 + (sy as f32 + 0.5) / SUPERSAMPLE as f32;
                    // The topmost shape under the sample wins.
                    if let Some((_, c)) = shapes.iter().rev().find(|(rect, _)| inside(x, y, *rect)) {
                        r += f32::from(c.r());
                        g += f32::from(c.g());
                        b += f32::from(c.b());
                        covered += 1.0;
                    }
                }
            }
            if covered == 0.0 {
                rgba.extend([0, 0, 0, 0]);
            } else {
                let avg = |v: f32| (v / covered).round() as u8;
                rgba.extend([avg(r), avg(g), avg(b), (covered / samples * 255.0).round() as u8]);
            }
        }
    }
    IconData { rgba, width: SIZE, height: SIZE }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_has_expected_shape() {
        let icon = icon();
        assert_eq!(icon.rgba.len(), (SIZE * SIZE * 4) as usize);
        let pixel = |x: u32, y: u32| &icon.rgba[((y * SIZE + x) * 4) as usize..][..4];
        assert_eq!(pixel(0, 0)[3], 0, "corners are transparent");
        assert_eq!(pixel(32, 12), &[theme::LCD_BG.r(), theme::LCD_BG.g(), theme::LCD_BG.b(), 255]);
        assert_eq!(pixel(15, 34)[..3], [theme::KEY_NUM.r(), theme::KEY_NUM.g(), theme::KEY_NUM.b()]);
    }
}

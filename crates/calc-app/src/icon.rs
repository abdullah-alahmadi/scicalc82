//! The window icon. Explorer's .exe icon comes from the same pixels via `build.rs`.

use eframe::egui::IconData;

use crate::icon_pixels;

const SIZE: u32 = 64;

pub fn icon() -> IconData {
    IconData { rgba: icon_pixels::rgba(SIZE), width: SIZE, height: SIZE }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme;
    use eframe::egui::Color32;

    #[test]
    fn icon_has_expected_shape() {
        let icon = icon();
        assert_eq!(icon.rgba.len(), (SIZE * SIZE * 4) as usize);
        let pixel = |x: u32, y: u32| &icon.rgba[((y * SIZE + x) * 4) as usize..][..4];
        assert_eq!(pixel(0, 0)[3], 0, "corners are transparent");
        assert_eq!(pixel(32, 12)[..3], icon_pixels::LCD_BG);
        assert_eq!(pixel(32, 12)[3], 255);
        assert_eq!(pixel(15, 34)[..3], icon_pixels::KEY_NUM);
    }

    #[test]
    fn icon_colors_match_the_theme() {
        let rgb = |c: [u8; 3]| Color32::from_rgb(c[0], c[1], c[2]);
        assert_eq!(rgb(icon_pixels::BODY), theme::BODY);
        assert_eq!(rgb(icon_pixels::LCD_BG), theme::LCD_BG);
        assert_eq!(rgb(icon_pixels::LCD_INK), theme::LCD_INK);
        assert_eq!(rgb(icon_pixels::KEY_NUM), theme::KEY_NUM);
        assert_eq!(rgb(icon_pixels::KEY_ACCENT), theme::KEY_ACCENT);
    }

    const FAVICON: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/favicon.svg");

    /// The web build's favicon is a committed file; keep it in step with the icon.
    #[test]
    fn favicon_matches_the_icon() {
        let committed = std::fs::read_to_string(FAVICON).unwrap_or_default();
        assert!(
            committed == icon_pixels::svg(),
            "assets/favicon.svg is out of date; run `cargo test -p calc-app write_favicon -- --ignored`"
        );
    }

    #[test]
    #[ignore = "rewrites assets/favicon.svg"]
    fn write_favicon() {
        std::fs::write(FAVICON, icon_pixels::svg()).expect("write favicon.svg");
    }

    #[test]
    fn scales_to_any_size() {
        for size in [16, 32, 256] {
            let pixels = icon_pixels::rgba(size);
            assert_eq!(pixels.len(), (size * size * 4) as usize);
            // A clear part of the display (grid point 20,12) is the LCD color at every size.
            let p = ((size * 12 / 64) * size + size * 20 / 64) as usize * 4;
            assert_eq!(pixels[p..p + 3], icon_pixels::LCD_BG, "size {size}");
        }
    }
}

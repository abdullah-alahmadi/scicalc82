//! The window icon. Explorer's .exe icon (`build.rs`) and the installers'
//! icons (`icons/`) come from the same pixels.

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

    /// The installers' icons (see `packaging/`): PNGs for Linux, .ico for
    /// the Windows installer and .icns for the macOS app.
    #[test]
    #[ignore = "rewrites the icons/ folder"]
    fn write_package_icons() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("icons");
        std::fs::create_dir_all(&dir).expect("create icons/");
        for size in [32, 48, 64, 128, 256, 512] {
            std::fs::write(dir.join(format!("{size}x{size}.png")), png(size)).expect("write png");
        }
        std::fs::write(dir.join("icon.ico"), icon_pixels::ico()).expect("write icon.ico");
        // .icns: a header, then (type, length, PNG data) for each size.
        let mut body = Vec::new();
        for (kind, size) in
            [(b"ic11", 32), (b"ic12", 64), (b"ic07", 128), (b"ic08", 256), (b"ic09", 512), (b"ic10", 1024)]
        {
            let data = png(size);
            body.extend(kind);
            body.extend((data.len() as u32 + 8).to_be_bytes());
            body.extend(data);
        }
        let mut icns = b"icns".to_vec();
        icns.extend((body.len() as u32 + 8).to_be_bytes());
        icns.extend(body);
        std::fs::write(dir.join("icon.icns"), icns).expect("write icon.icns");
    }

    fn png(size: u32) -> Vec<u8> {
        let image = image::RgbaImage::from_raw(size, size, icon_pixels::rgba(size)).expect("icon pixels");
        let mut out = std::io::Cursor::new(Vec::new());
        image.write_to(&mut out, image::ImageFormat::Png).expect("encode png");
        out.into_inner()
    }

    #[test]
    fn ico_lists_every_size() {
        let ico = icon_pixels::ico();
        assert_eq!(ico[..4], [0, 0, 1, 0], "an icon file");
        assert_eq!(u16::from_le_bytes([ico[4], ico[5]]), 6);
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

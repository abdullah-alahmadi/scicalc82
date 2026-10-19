//! The app icon as raw RGBA pixels: a small calculator.
//!
//! Plain Rust with no dependencies, so that the app (window icon), `build.rs`
//! (the .exe icon Explorer shows) and the installers' icons all come from the
//! same code.

/// Colors, matching `theme.rs` (checked by a test there).
pub const BODY: [u8; 3] = [0x2a, 0x2e, 0x35];
pub const LCD_BG: [u8; 3] = [0xc4, 0xcf, 0xbc];
pub const LCD_INK: [u8; 3] = [0x1e, 0x25, 0x20];
pub const KEY_NUM: [u8; 3] = [0xe4, 0xe6, 0xea];
pub const KEY_ACCENT: [u8; 3] = [0xd9, 0x7a, 0x2b];
/// Icon only: a light rim so the dark body stands out on dark taskbars.
pub const RIM: [u8; 3] = [0x6b, 0x72, 0x7e];

/// Shapes are laid out on a 64×64 grid and scaled to the requested size.
const GRID: f32 = 64.0;
/// Samples per pixel along each axis, for smooth edges.
const SUPERSAMPLE: u32 = 4;

/// A rounded rectangle (x0, y0, x1, y1, radius) and its color.
type Shape = ((f32, f32, f32, f32, f32), [u8; 3]);

fn shapes() -> Vec<Shape> {
    let mut shapes = vec![
        ((5.0, 1.0, 59.0, 63.0, 10.0), RIM),
        ((6.0, 2.0, 58.0, 62.0, 9.0), BODY),
        ((11.0, 8.0, 53.0, 25.0, 3.0), LCD_BG),
        // A result on the display.
        ((32.0, 17.0, 34.5, 21.0, 0.5), LCD_INK),
        ((36.5, 17.0, 39.0, 21.0, 0.5), LCD_INK),
        ((41.0, 17.0, 48.0, 21.0, 0.5), LCD_INK),
    ];
    for row in 0..3 {
        for col in 0..4 {
            let (x, y) = (12.0 + col as f32 * 10.5, 31.0 + row as f32 * 10.0);
            let color = if row == 0 && col == 3 { KEY_ACCENT } else { KEY_NUM };
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

/// The icon as an SVG document: the web build's favicon, kept current by a test.
#[cfg(test)]
pub fn svg() -> String {
    let rects: String = shapes()
        .iter()
        .map(|((x0, y0, x1, y1, r), [red, green, blue])| {
            format!(
                "  <rect x=\"{x0}\" y=\"{y0}\" width=\"{}\" height=\"{}\" rx=\"{r}\" fill=\"#{red:02x}{green:02x}{blue:02x}\"/>
",
                x1 - x0,
                y1 - y0
            )
        })
        .collect();
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {GRID} {GRID}\">
{rects}</svg>
"
    )
}

/// The icon at `size`×`size` pixels: RGBA rows, top to bottom, with
/// straight (unpremultiplied) alpha.
pub fn rgba(size: u32) -> Vec<u8> {
    let shapes = shapes();
    let scale = GRID / size as f32;
    let samples = (SUPERSAMPLE * SUPERSAMPLE) as f32;
    let mut out = Vec::with_capacity((size * size * 4) as usize);
    for py in 0..size {
        for px in 0..size {
            let (mut r, mut g, mut b, mut covered) = (0.0, 0.0, 0.0, 0.0);
            for sy in 0..SUPERSAMPLE {
                for sx in 0..SUPERSAMPLE {
                    let x = (px as f32 + (sx as f32 + 0.5) / SUPERSAMPLE as f32) * scale;
                    let y = (py as f32 + (sy as f32 + 0.5) / SUPERSAMPLE as f32) * scale;
                    // The topmost shape under the sample wins.
                    if let Some((_, c)) = shapes.iter().rev().find(|(rect, _)| inside(x, y, *rect)) {
                        r += f32::from(c[0]);
                        g += f32::from(c[1]);
                        b += f32::from(c[2]);
                        covered += 1.0;
                    }
                }
            }
            if covered == 0.0 {
                out.extend([0, 0, 0, 0]);
            } else {
                let avg = |v: f32| (v / covered).round() as u8;
                out.extend([avg(r), avg(g), avg(b), (covered / samples * 255.0).round() as u8]);
            }
        }
    }
    out
}

/// Icon sizes Windows picks from for different views and DPI settings.
#[cfg_attr(not(test), allow(dead_code))]
const ICO_SIZES: [u32; 6] = [16, 24, 32, 48, 64, 256];

/// One icon image in .ico form: a 32-bit bitmap (rows bottom to top, BGRA)
/// followed by an all-zero AND mask, since the alpha channel does the work.
#[cfg_attr(not(test), allow(dead_code))]
fn ico_image(size: u32) -> Vec<u8> {
    let rgba = rgba(size);
    let mask_row = size.div_ceil(32) * 4;
    let pixel_bytes = size * size * 4;
    let mut out = Vec::new();
    // BITMAPINFOHEADER; the height counts the bitmap and the mask together.
    for value in [40, size, size * 2] {
        out.extend(value.to_le_bytes());
    }
    out.extend(1u16.to_le_bytes()); // planes
    out.extend(32u16.to_le_bytes()); // bits per pixel
    for value in [0, pixel_bytes + mask_row * size, 0, 0, 0, 0] {
        out.extend(u32::to_le_bytes(value));
    }
    for y in (0..size).rev() {
        for x in 0..size {
            let p = ((y * size + x) * 4) as usize;
            out.extend([rgba[p + 2], rgba[p + 1], rgba[p], rgba[p + 3]]);
        }
    }
    out.resize(out.len() + (mask_row * size) as usize, 0);
    out
}

/// The icon as a Windows .ico file, for the .exe (`build.rs`) and the installer.
#[cfg_attr(not(test), allow(dead_code))]
pub fn ico() -> Vec<u8> {
    let images: Vec<Vec<u8>> = ICO_SIZES.iter().map(|&s| ico_image(s)).collect();
    let mut out = Vec::new();
    for value in [0u16, 1, ICO_SIZES.len() as u16] {
        out.extend(value.to_le_bytes()); // reserved, type (icon), count
    }
    let mut offset = 6 + 16 * ICO_SIZES.len() as u32;
    for (&size, image) in ICO_SIZES.iter().zip(&images) {
        let dim = if size >= 256 { 0 } else { size as u8 }; // 0 means 256
        out.extend([dim, dim, 0, 0]);
        out.extend(1u16.to_le_bytes()); // planes
        out.extend(32u16.to_le_bytes()); // bits per pixel
        out.extend((image.len() as u32).to_le_bytes());
        out.extend(offset.to_le_bytes());
        offset += image.len() as u32;
    }
    images.into_iter().for_each(|image| out.extend(image));
    out
}

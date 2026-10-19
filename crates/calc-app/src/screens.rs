//! Offscreen renders of the calculator in various states, for checking the
//! UI without a visible window (and without a desktop at all).
//!
//! Run with `cargo test -p calc-app render_screens -- --ignored`; the
//! images are written to `target/screens/`.

use std::path::PathBuf;

use calc_core::{Calculator, Key};
use eframe::egui::vec2;
use egui_kittest::Harness;

use crate::app::{draw_calculator, install_fonts};
use crate::theme;

/// The keys described by space-separated words; runs of digits are typed
/// one by one, and `S`/`A` are SHIFT/ALPHA.
fn script_keys(script: &str) -> Vec<Key> {
    let mut keys = Vec::new();
    for word in script.split_whitespace() {
        if word.bytes().all(|b| b.is_ascii_digit()) {
            keys.extend(word.bytes().map(|b| Key::Digit(b - b'0')));
            continue;
        }
        let key = match word {
            "S" => Key::Shift,
            "A" => Key::Alpha,
            "+" => Key::Plus,
            "-" => Key::Minus,
            "*" => Key::Times,
            "/" => Key::Divide,
            "(" => Key::LParen,
            ")" => Key::RParen,
            "." => Key::Point,
            "=" => Key::Equals,
            "sin" => Key::Sin,
            "frac" => Key::Fraction,
            "sqrt" => Key::Sqrt,
            "sq" => Key::Square,
            "pow" => Key::Power,
            "dms" => Key::Dms,
            "ac" => Key::Ac,
            "ans" => Key::Ans,
            "exp" => Key::Exp10,
            "mode" => Key::Mode,
            "neg" => Key::Neg,
            "sd" => Key::SToD,
            "eng" => Key::Eng,
            "abs" => Key::Abs,
            "recip" => Key::Recip,
            "left" => Key::Left,
            "right" => Key::Right,
            "up" => Key::Up,
            "down" => Key::Down,
            _ => panic!("unknown key word {word:?}"),
        };
        keys.push(key);
    }
    keys
}

fn press_script(calc: &mut Calculator, script: &str) {
    script_keys(script).into_iter().for_each(|key| calc.press(key));
}

const SCENES: &[(&str, &str)] = &[
    ("natural", "1 frac 3 right + sqrt 1 frac 2 ="),
    ("cmplx_rect", "mode 2 ( 3 + 4 eng ) / ( 1 + 2 eng ) ="),
    ("cmplx_polar", "mode 2 1 + eng S 2 3 ="),
    ("cmplx_menu", "mode 2 S 2"),
    ("matrix_editor", "mode 5 1 5 1 = 2 = 3 ="),
    ("matrix_inverse", "mode 5 1 5 1 = 2 = 3 = 4 = ac S 4 3 recip ="),
    ("matrix_3x3", "mode 5 1 1 2 = 0 = 1 = 1 = 3 = 2 = 1 = 1 = 2 = ac S 4 3 recip ="),
    ("matrix_menu", "mode 5 1 5 ac S 4"),
    ("eqn_menu", "mode 6"),
    ("vector_editor", "mode 7 1 1 1 = 2 = 3 ="),
    ("vector_menu", "mode 7 1 1 ac S 5"),
    ("vector_cross", "mode 7 1 1 1 = 2 = 3 = ac S 5 1 2 1 4 = 5 = 6 = ac S 5 3 * S 5 4 ="),
    ("vector_dot", "mode 7 1 1 1 = 2 = 3 = ac S 5 1 2 1 4 = 5 = 6 = ac S 5 3 S 5 7 S 5 4 ="),
    ("vector_length", "mode 7 1 1 1 = 2 = 3 = ac S 5 1 2 1 4 = 5 = 6 = ac abs S 5 3 ="),
    ("vector_angle", "mode 7 1 2 1 = 0 = ac S 5 1 2 2 1 = 1 = ac S 5 8 S 5 3 S ) S 5 4 ) ="),
    ("eqn_editor", "mode 6 1 1 = 1 = 3 = 1 = neg 1"),
    ("eqn_quadratic", "mode 6 3 1 = 0 = neg 2 = ="),
    ("eqn_cubic_complex", "mode 6 4 1 = 0 = 0 = neg 1 = = ="),
    ("eqn_no_solution", "mode 6 1 1 = 1 = 1 = 2 = 2 = 3 = ="),
    ("sine", "sin 30 ="),
    ("mode_menu", "mode"),
    ("setup_menu", "S mode"),
    ("setup_page2", "S mode down"),
    ("fix_prompt", "S mode 6"),
    ("drg_menu", "S ans"),
    ("line_io", "S mode 2 1 frac 2 + 1 frac 3 ="),
    ("dms", "2 . 258 = dms"),
    ("stat_editor", "mode 3 2 1 = 2 = 3 = 4 = up up up up right 3 = 5 = 7 . 5 ="),
    ("stat_menu", "mode 3 2 1 = ac S 1"),
    ("stat_var_menu", "mode 3 2 1 = ac S 1 4"),
    ("stat_mean", "mode 3 1 1 = 2 = 3 = 4 = ac S 1 4 2 ="),
    ("stat_estimate", "mode 3 2 1 = 2 = 3 = up up up right 3 = 5 = 7 = ac 10 S 1 5 5 ="),
    ("table_function", "mode 4 A ) sq + 1"),
    ("table_range", "mode 4 A ) sq + 1 ="),
    ("table_view", "mode 4 A ) sq + 1 = = = = down"),
    ("table_error", "mode 4 A ) = 1 = 100 = ="),
];

fn output_dir() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/screens");
    std::fs::create_dir_all(&dir).expect("create target/screens");
    dir
}

/// Renders the calculator after `script`, at `scale` pixels per point.
fn render(script: &str, scale: f32) -> image::RgbaImage {
    let mut calc = Calculator::new();
    press_script(&mut calc, script);
    render_calc(calc, None, scale)
}

/// Renders `calc`, with the `flash` key drawn pressed.
fn render_calc(calc: Calculator, flash: Option<Key>, scale: f32) -> image::RgbaImage {
    let mut harness = Harness::builder()
        .with_size(vec2(theme::DESIGN_W + 20.0, theme::DESIGN_H + 20.0))
        .with_pixels_per_point(scale)
        .build_ui(move |ui| {
            draw_calculator(ui, &calc, flash, true);
        });
    install_fonts(&harness.ctx);
    harness.run_steps(3);
    harness.render().unwrap_or_else(|e| panic!("render: {e}"))
}

#[test]
#[ignore = "writes PNG files; run explicitly"]
fn render_screens() {
    let dir = output_dir();
    for (name, script) in SCENES {
        render(script, 1.5).save(dir.join(format!("{name}.png"))).expect("save png");
    }
}

/// Screens shown in docs/USER_MANUAL.md: (file name, keys, whole calculator?).
const MANUAL_IMAGES: &[(&str, &str, bool)] = &[
    ("overview", "1 frac 3 right + sqrt 1 frac 2 =", true),
    ("editing", "2 frac 3 right + sqrt 8", false),
    ("exact_result", "1 frac 3 right + sqrt 1 frac 2 =", false),
    ("decimal_result", "1 frac 3 right + sqrt 1 frac 2 = sd", false),
    ("trig", "sin 30 =", false),
    ("error", "1 / 0 =", false),
    ("mode_menu", "mode", false),
    ("setup_menu", "S mode", false),
    ("dms", "2 . 258 = dms", false),
    ("cmplx_rect", "mode 2 ( 3 + 4 eng ) / ( 1 + 2 eng ) =", false),
    ("cmplx_polar", "mode 2 1 + eng S 2 3 =", false),
    ("stat_editor", "mode 3 2 1 = 2 = 3 = 4 = up up up up right 3 = 5 = 7 . 5 =", false),
    ("stat_estimate", "mode 3 2 1 = 2 = 3 = up up up right 3 = 5 = 7 = ac 10 S 1 5 5 =", false),
    ("table_view", "mode 4 A ) sq + 1 = = = = down", false),
    ("matrix_editor", "mode 5 1 5 1 = 2 = 3 =", false),
    ("matrix_inverse", "mode 5 1 5 1 = 2 = 3 = 4 = ac S 4 3 recip =", false),
    ("eqn_menu", "mode 6", false),
    ("vector_editor", "mode 7 1 1 1 = 2 = 3 =", false),
    ("vector_cross", "mode 7 1 1 1 = 2 = 3 = ac S 5 1 2 1 4 = 5 = 6 = ac S 5 3 * S 5 4 =", false),
    ("vector_angle", "mode 7 1 2 1 = 0 = ac S 5 1 2 2 1 = 1 = ac S 5 8 S 5 3 S ) S 5 4 ) =", false),
    ("eqn_root", "mode 6 4 1 = 0 = 0 = neg 1 = = =", false),
];

/// Regenerates the manual's images. Display-only images are cropped to
/// the LCD's bezel, with its rounded corners left transparent.
#[test]
#[ignore = "rewrites docs/images; run explicitly"]
fn render_manual_images() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/images");
    std::fs::create_dir_all(&dir).expect("create docs/images");
    let (gx, gy, gw, gh) = crate::lcd::GLASS;
    // The bezel: 10 points around the glass, with 10-point corner radii.
    let margin = 10.0;
    for (name, script, whole) in MANUAL_IMAGES {
        let path = dir.join(format!("{name}.png"));
        if *whole {
            render(script, 1.25).save(path).expect("save png");
            continue;
        }
        let image = render(script, 1.5);
        // Where the body landed, in pixels, and its pixels per design point.
        let (left, top, width) = body_bounds(&image);
        let scale = width / theme::DESIGN_W;
        let px = |v: f32| v * scale;
        let (x, y) = (left + px(gx - margin), top + px(gy - margin));
        let (w, h) = (px(gw + 2.0 * margin), px(gh + 2.0 * margin));
        let crop =
            image::imageops::crop_imm(&image, x.round() as u32, y.round() as u32, w.round() as u32, h.round() as u32);
        round_corners(crop.to_image(), px(margin)).save(path).expect("save png");
    }
}

/// The left and top edges and the width of the calculator body in `image`:
/// the extent of everything that is neither backdrop nor the transparent
/// border the test harness leaves around the drawing.
fn body_bounds(image: &image::RgbaImage) -> (f32, f32, f32) {
    let backdrop = theme::BACKDROP;
    let (mut x0, mut y0, mut x1) = (u32::MAX, u32::MAX, 0);
    for (x, y, pixel) in image.enumerate_pixels() {
        if pixel.0[3] == 255 && pixel.0[..3] != [backdrop.r(), backdrop.g(), backdrop.b()] {
            (x0, y0, x1) = (x0.min(x), y0.min(y), x1.max(x + 1));
        }
    }
    (x0 as f32, y0 as f32, (x1 - x0) as f32)
}

/// Makes everything outside a rounded rectangle of `radius` pixels
/// transparent, blending the edge over one pixel.
fn round_corners(mut image: image::RgbaImage, radius: f32) -> image::RgbaImage {
    let (w, h) = (image.width() as f32, image.height() as f32);
    for (x, y, pixel) in image.enumerate_pixels_mut() {
        let (cx, cy) = (x as f32 + 0.5, y as f32 + 0.5);
        // Distance outside the rounded rectangle, from the nearest corner centre.
        let dx = (radius - cx).max(cx - (w - radius)).max(0.0);
        let dy = (radius - cy).max(cy - (h - radius)).max(0.0);
        let outside = (dx.hypot(dy) - radius + 0.5).clamp(0.0, 1.0);
        pixel.0[3] = (f32::from(pixel.0[3]) * (1.0 - outside)).round() as u8;
    }
    image
}

/// The backdrop around the calculator becomes transparent, so README images
/// sit well on both light and dark pages.
fn without_backdrop(mut image: image::RgbaImage) -> image::RgbaImage {
    let backdrop = theme::BACKDROP;
    for pixel in image.pixels_mut() {
        let [r, g, b, _] = pixel.0;
        if [r, g, b] == [backdrop.r(), backdrop.g(), backdrop.b()] {
            pixel.0 = [0, 0, 0, 0];
        }
    }
    image
}

/// The README's hero image (three calculators side by side) and its typing
/// animation.
#[test]
#[ignore = "rewrites the README images in docs/images; run explicitly"]
fn render_readme_images() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/images");
    let hero = [
        "1 frac 3 right + sqrt 1 frac 2 =",
        "mode 5 1 1 2 = 0 = 1 = 1 = 3 = 2 = 1 = 1 = 2 = ac S 4 3 recip =",
        "mode 6 4 1 = 0 = 0 = neg 1 = = =",
    ];
    let shots: Vec<_> = hero.iter().map(|script| without_backdrop(render(script, 2.0))).collect();
    // Each render has a 10-point margin, so the bodies end up 20 points apart.
    let (w, h) = shots[0].dimensions();
    let mut canvas = image::RgbaImage::new(3 * w, h);
    for (i, shot) in shots.iter().enumerate() {
        image::imageops::overlay(&mut canvas, shot, i as i64 * w as i64, 0);
    }
    canvas.save(dir.join("hero.png")).expect("save hero.png");

    // Typing 1/3 + √(1/2), then S⇔D to the decimal and back.
    let mut frames = Vec::new();
    let mut calc = Calculator::new();
    frames.push((render_calc(calc.clone(), None, 1.0), 900));
    let keys = script_keys("1 frac 3 right + sqrt 1 frac 2 = sd sd");
    let last = keys.len() - 1;
    for (i, key) in keys.into_iter().enumerate() {
        calc.press(key);
        let hold = match key {
            Key::Equals | Key::SToD => 1600,
            _ if i == last => 1600,
            _ => 380,
        };
        frames.push((render_calc(calc.clone(), Some(key), 1.0), 160));
        frames.push((render_calc(calc.clone(), None, 1.0), hold));
    }
    let file = std::fs::File::create(dir.join("demo.gif")).expect("create demo.gif");
    let mut gif = image::codecs::gif::GifEncoder::new_with_speed(file, 10);
    gif.set_repeat(image::codecs::gif::Repeat::Infinite).expect("loop the gif");
    for (image, ms) in frames {
        let delay = image::Delay::from_numer_denom_ms(ms, 1);
        gif.encode_frame(image::Frame::from_parts(without_backdrop(image), 0, 0, delay)).expect("gif frame");
    }
}

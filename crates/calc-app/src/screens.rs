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

/// Presses keys described by space-separated words; runs of digits are typed
/// one by one, and `S`/`A` are SHIFT/ALPHA.
fn press_script(calc: &mut Calculator, script: &str) {
    for word in script.split_whitespace() {
        if word.bytes().all(|b| b.is_ascii_digit()) {
            for b in word.bytes() {
                calc.press(Key::Digit(b - b'0'));
            }
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
            "recip" => Key::Recip,
            "left" => Key::Left,
            "right" => Key::Right,
            "up" => Key::Up,
            "down" => Key::Down,
            _ => panic!("unknown key word {word:?}"),
        };
        calc.press(key);
    }
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
    let mut harness = Harness::builder()
        .with_size(vec2(theme::DESIGN_W + 20.0, theme::DESIGN_H + 20.0))
        .with_pixels_per_point(scale)
        .build_ui(move |ui| {
            draw_calculator(ui, &calc, None, true);
        });
    install_fonts(&harness.ctx);
    harness.run_steps(3);
    harness.render().unwrap_or_else(|e| panic!("render {script:?}: {e}"))
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
    ("eqn_root", "mode 6 4 1 = 0 = 0 = neg 1 = = =", false),
];

/// Regenerates the manual's images. Display-only images are cropped to
/// the LCD and its bezel.
#[test]
#[ignore = "rewrites docs/images; run explicitly"]
fn render_manual_images() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/images");
    std::fs::create_dir_all(&dir).expect("create docs/images");
    // The calculator is drawn at 1:1 design units, offset by the 10-point margin.
    let (gx, gy, gw, gh) = crate::lcd::GLASS;
    let margin = 14.0;
    for (name, script, whole) in MANUAL_IMAGES {
        let path = dir.join(format!("{name}.png"));
        if *whole {
            render(script, 1.25).save(path).expect("save png");
            continue;
        }
        let scale = 1.5;
        let image = render(script, scale);
        let px = |v: f32| (v * scale).round() as u32;
        let (x, y) = (px(10.0 + gx - margin), px(10.0 + gy - margin));
        let (w, h) = (px(gw + 2.0 * margin), px(gh + 2.0 * margin));
        image::imageops::crop_imm(&image, x, y, w, h).to_image().save(path).expect("save png");
    }
}

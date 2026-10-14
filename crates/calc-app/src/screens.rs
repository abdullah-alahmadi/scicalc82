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
    ("sine", "sin 30 ="),
    ("mode_menu", "mode"),
    ("setup_menu", "S mode"),
    ("setup_page2", "S mode down"),
    ("fix_prompt", "S mode 6"),
    ("drg_menu", "S ans"),
    ("line_io", "S mode 2 1 frac 2 + 1 frac 3 ="),
    ("dms", "2 . 258 = dms"),
    ("stat_editor", "mode 2 2 1 = 2 = 3 = 4 = up up up up right 3 = 5 = 7 . 5 ="),
    ("stat_menu", "mode 2 2 1 = ac S 1"),
    ("stat_var_menu", "mode 2 2 1 = ac S 1 4"),
    ("stat_mean", "mode 2 1 1 = 2 = 3 = 4 = ac S 1 4 2 ="),
    ("stat_estimate", "mode 2 2 1 = 2 = 3 = up up up right 3 = 5 = 7 = ac 10 S 1 5 5 ="),
    ("table_function", "mode 3 A ) sq + 1"),
    ("table_range", "mode 3 A ) sq + 1 ="),
    ("table_view", "mode 3 A ) sq + 1 = = = = down"),
    ("table_error", "mode 3 A ) = 1 = 100 = ="),
];

fn output_dir() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/screens");
    std::fs::create_dir_all(&dir).expect("create target/screens");
    dir
}

#[test]
#[ignore = "writes PNG files; run explicitly"]
fn render_screens() {
    let dir = output_dir();
    for (name, script) in SCENES {
        let mut calc = Calculator::new();
        press_script(&mut calc, script);
        let mut harness = Harness::builder()
            .with_size(vec2(theme::DESIGN_W + 20.0, theme::DESIGN_H + 20.0))
            .with_pixels_per_point(1.5)
            .build_ui(move |ui| {
                draw_calculator(ui, &calc, None, true);
            });
        install_fonts(&harness.ctx);
        harness.run_steps(3);
        let image = harness.render().unwrap_or_else(|e| panic!("render {name}: {e}"));
        image.save(dir.join(format!("{name}.png"))).expect("save png");
    }
}

//! PC keyboard shortcuts, as sequences of calculator keys.
//!
//! Typed characters arrive as text, so this follows the keyboard layout;
//! non-printing keys (Enter, arrows, ...) arrive as key events.

use calc_core::key::Key;
use eframe::egui;

/// The calculator keys for a typed character.
pub fn for_char(c: char) -> Vec<Key> {
    let keys: &[Key] = match c {
        '0'..='9' => return vec![Key::Digit(c as u8 - b'0')],
        '+' => &[Key::Plus],
        '-' => &[Key::Minus],
        '*' => &[Key::Times],
        '/' => &[Key::Divide],
        '(' => &[Key::LParen],
        ')' => &[Key::RParen],
        '.' => &[Key::Point],
        ',' => &[Key::Shift, Key::RParen],
        '^' => &[Key::Power],
        '=' => &[Key::Equals],
        '!' => &[Key::Shift, Key::Recip],
        '%' => &[Key::Shift, Key::LParen],
        'E' => &[Key::Exp10],
        'p' => &[Key::Shift, Key::Exp10],
        'e' => &[Key::Alpha, Key::Exp10],
        's' => &[Key::Sin],
        'c' => &[Key::Cos],
        't' => &[Key::Tan],
        'S' => &[Key::Shift, Key::Sin],
        'C' => &[Key::Shift, Key::Cos],
        'T' => &[Key::Shift, Key::Tan],
        'h' => &[Key::Hyp],
        'l' => &[Key::Log],
        'n' => &[Key::Ln],
        'r' => &[Key::Sqrt],
        'q' => &[Key::Square],
        'f' => &[Key::Fraction],
        'a' => &[Key::Ans],
        'd' => &[Key::SToD],
        'm' => &[Key::MPlus],
        // CMPLX mode: i is the ENG key; < is the nearest thing to ∠.
        'i' => &[Key::Eng],
        '<' => &[Key::Shift, Key::Neg],
        _ => &[],
    };
    keys.to_vec()
}

/// The calculator keys for a non-printing key press.
pub fn for_key(key: egui::Key) -> Vec<Key> {
    let k = match key {
        egui::Key::Enter => Key::Equals,
        egui::Key::Backspace => Key::Del,
        egui::Key::Escape | egui::Key::Delete => Key::Ac,
        egui::Key::ArrowLeft => Key::Left,
        egui::Key::ArrowRight => Key::Right,
        egui::Key::ArrowUp => Key::Up,
        egui::Key::ArrowDown => Key::Down,
        _ => return Vec::new(),
    };
    vec![k]
}

/// A short reference shown in the app's tooltip.
pub const HELP: &str = "Keyboard: digits + - * / ( ) . ^ = Enter\n\
Backspace DEL · Esc AC · arrows move/replay\n\
s c t sin cos tan · S C T inverse · h hyp\n\
l log · n ln · r √ · q x² · f fraction · ! x!\n\
p π · e e · E ×10^x · a Ans · d S⇔D · m M+\n\
CMPLX mode: i i · < ∠\n\
Ctrl+C copies the result";

use std::time::Duration;

use calc_core::Calculator;
use calc_core::key::Key;
use eframe::egui::{self, Align2, FontDefinitions, FontFamily, Sense, Stroke};

use crate::theme::{self, View};
use crate::{keymap, keypad, lcd};

pub const APP_NAME: &str = "SciCalc 82";

/// How long a key pressed from the PC keyboard is drawn held down.
const FLASH_SECONDS: f64 = 0.12;
/// Cursor blink half-period.
const BLINK_SECONDS: f64 = 0.5;

pub struct CalcApp {
    calc: Calculator,
    flash: Option<(Key, f64)>,
}

impl CalcApp {
    pub fn new(cc: &eframe::CreationContext<'_>, seed: u64) -> Self {
        install_fonts(&cc.egui_ctx);
        let mut calc = Calculator::new();
        calc.reseed(seed);
        Self { calc, flash: None }
    }

    fn press(&mut self, key: Key, now: f64) {
        self.calc.press(key);
        self.flash = Some((key, now));
    }

    fn handle_keyboard(&mut self, ctx: &egui::Context, now: f64) {
        let events = ctx.input(|i| i.events.clone());
        for event in events {
            let keys = match event {
                egui::Event::Text(text) => text.chars().flat_map(keymap::for_char).collect(),
                egui::Event::Key { key, pressed: true, modifiers, .. } if !modifiers.command => keymap::for_key(key),
                egui::Event::Copy => {
                    if let Some(text) = self.calc.result_text() {
                        ctx.copy_text(text);
                    }
                    Vec::new()
                }
                _ => Vec::new(),
            };
            for key in keys {
                self.press(key, now);
            }
        }
    }
}

fn draw_body(ui: &egui::Ui, view: View) {
    let painter = ui.painter();
    let body = view.rect(0.0, 0.0, theme::DESIGN_W, theme::DESIGN_H);
    painter.rect_filled(body, view.len(26.0), theme::BODY);
    painter.rect_filled(view.rect(8.0, 8.0, theme::DESIGN_W - 16.0, 180.0), view.len(20.0), theme::FACEPLATE);
    painter.rect_stroke(body, view.len(26.0), Stroke::new(view.len(1.5), theme::BEZEL), egui::StrokeKind::Inside);

    // Model name (hover for keyboard help) and the solar panel.
    let brand = painter.text(view.pos(24.0, 30.0), Align2::LEFT_CENTER, APP_NAME, view.label_font(16.0), theme::BRAND);
    painter.text(
        view.pos(24.0, 46.0),
        Align2::LEFT_CENTER,
        "NATURAL DISPLAY",
        view.label_font(7.5),
        theme::BRAND_FAINT,
    );
    ui.interact(brand, ui.id().with("brand"), Sense::hover()).on_hover_text(keymap::HELP);

    let solar = view.rect(232.0, 18.0, 100.0, 28.0);
    painter.rect_filled(solar, view.len(3.0), theme::SOLAR);
    for i in 1..4 {
        let x = solar.left() + solar.width() * i as f32 / 4.0;
        painter.line_segment(
            [egui::pos2(x, solar.top()), egui::pos2(x, solar.bottom())],
            Stroke::new(view.len(1.0), theme::SOLAR_LINE),
        );
    }
}

/// The proportional font lacks symbols such as ⁻ ∛ □; fall back to Hack.
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    fonts.families.entry(FontFamily::Proportional).or_default().push("Hack".to_owned());
    ctx.set_fonts(fonts);
}

/// Draws the whole calculator into `ui` and returns the keys clicked.
pub fn draw_calculator(ui: &egui::Ui, calc: &Calculator, flash: Option<Key>, cursor_visible: bool) -> Vec<Key> {
    let area = ui.max_rect();
    ui.painter().rect_filled(area, 0.0, theme::BACKDROP);
    let view = View::fit(area.shrink(10.0));
    draw_body(ui, view);
    lcd::draw(ui.painter(), view, calc, cursor_visible);
    keypad::draw(ui, view, calc, flash)
}

impl eframe::App for CalcApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let now = ctx.input(|i| i.time);
        self.handle_keyboard(&ctx, now);
        if self.flash.is_some_and(|(_, t)| now - t > FLASH_SECONDS) {
            self.flash = None;
        }

        let cursor_visible = (now / BLINK_SECONDS) as i64 % 2 == 0;
        for key in draw_calculator(ui, &self.calc, self.flash.map(|(k, _)| k), cursor_visible) {
            self.calc.press(key);
        }

        // Keep the cursor blinking and release flashed keys.
        let next = if self.flash.is_some() { FLASH_SECONDS } else { BLINK_SECONDS - now % BLINK_SECONDS };
        ctx.request_repaint_after(Duration::from_secs_f64(next.max(0.02)));
    }
}

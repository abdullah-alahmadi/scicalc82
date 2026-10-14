#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod dots;
mod icon;
mod icon_pixels;
mod keymap;
mod keypad;
mod lcd;
mod math;
mod rich;
#[cfg(test)]
mod screens;
mod theme;

use std::time::{SystemTime, UNIX_EPOCH};

use eframe::egui;

use app::{APP_NAME, CalcApp};

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(APP_NAME)
            .with_app_id("scicalc82")
            .with_icon(icon::icon())
            .with_inner_size([theme::DESIGN_W + 20.0, theme::DESIGN_H + 20.0])
            .with_min_inner_size([240.0, 430.0]),
        ..Default::default()
    };
    let seed = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos() as u64);
    eframe::run_native(APP_NAME, options, Box::new(move |cc| Ok(Box::new(CalcApp::new(cc, seed)))))
}

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui;

const APP_NAME: &str = "SciCalc 82";

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(APP_NAME)
            .with_inner_size([360.0, 680.0])
            .with_resizable(false),
        ..Default::default()
    };
    eframe::run_native(APP_NAME, options, Box::new(|_cc| Ok(Box::<CalcApp>::default())))
}

#[derive(Default)]
struct CalcApp;

impl eframe::App for CalcApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default_margins().show(ui, |ui| {
            ui.heading(APP_NAME);
            ui.label("Calculator UI arrives in milestone 4.");
        });
    }
}

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod dots;
#[cfg(not(target_arch = "wasm32"))]
mod icon;
#[cfg(not(target_arch = "wasm32"))]
mod icon_pixels;
mod keymap;
mod keypad;
mod lcd;
mod math;
mod rich;
#[cfg(test)]
mod screens;
mod theme;

use app::CalcApp;

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    use std::time::{SystemTime, UNIX_EPOCH};

    use app::APP_NAME;
    use eframe::egui;

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

/// The web build: runs in the `<canvas id="calculator">` of `index.html`.
#[cfg(target_arch = "wasm32")]
fn main() {
    use eframe::wasm_bindgen::JsCast as _;
    use eframe::web_sys;

    // SystemTime isn't available in the browser; seed Ran# from JavaScript.
    let seed = (web_sys::js_sys::Math::random() * u64::MAX as f64) as u64;
    wasm_bindgen_futures::spawn_local(async move {
        let document = web_sys::window().and_then(|w| w.document()).expect("running in a web page");
        let canvas = document
            .get_element_by_id("calculator")
            .and_then(|e| e.dyn_into::<web_sys::HtmlCanvasElement>().ok())
            .expect("index.html has <canvas id=\"calculator\">");
        let started = eframe::WebRunner::new()
            .start(canvas, eframe::WebOptions::default(), Box::new(move |cc| Ok(Box::new(CalcApp::new(cc, seed)))))
            .await;
        // Replace the loading message, or show why the app couldn't start.
        if let Some(status) = document.get_element_by_id("status") {
            match started {
                Ok(()) => status.remove(),
                Err(e) => status.set_text_content(Some(&format!("SciCalc 82 could not start: {e:?}"))),
            }
        }
    });
}

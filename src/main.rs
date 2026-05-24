mod app;
mod engine;
mod persistent_cache;

use app::DraughtsApp;
use eframe::egui;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Kish · Turkish Draughts Studio")
            .with_inner_size([1280.0, 860.0])
            .with_min_inner_size([1040.0, 720.0]),
        ..Default::default()
    };

    eframe::run_native(
        "kish_dama_studio",
        options,
        Box::new(|cc| Ok(Box::new(DraughtsApp::new(cc)))),
    )
}

use eframe::egui::Vec2;
use rust_browser::app::BrowserApp;

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size(Vec2::new(1200.0, 800.0))
            .with_min_inner_size(Vec2::new(600.0, 400.0))
            .with_title("IronSurf - Pure Rust Web Browser (Zero Chromium)"),
        ..Default::default()
    };

    eframe::run_native(
        "IronSurf",
        native_options,
        Box::new(|_cc| Ok(Box::new(BrowserApp::new()))),
    )
}

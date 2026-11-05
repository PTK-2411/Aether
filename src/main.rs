use eframe::egui::{self, Vec2};
use rust_browser::app::BrowserApp;

fn setup_custom_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    let font_files = [
        ("liberation_sans", "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf"),
        ("noto_sans", "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf"),
        ("dejavu_sans", "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf"),
        ("noto_symbols", "/usr/share/fonts/truetype/noto/NotoSansSymbols-Regular.ttf"),
        ("noto_symbols2", "/usr/share/fonts/truetype/noto/NotoSansSymbols2-Regular.ttf"),
    ];

    let mut loaded_names = Vec::new();
    for (name, path) in &font_files {
        if let Ok(data) = std::fs::read(path) {
            fonts.font_data.insert(
                name.to_string(),
                std::sync::Arc::new(egui::FontData::from_owned(data)),
            );
            loaded_names.push(name.to_string());
        }
    }

    if !loaded_names.is_empty() {
        // Prepend loaded fonts in priority order to both proportional and monospace
        for name in loaded_names.into_iter().rev() {
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(0, name.clone());
            fonts
                .families
                .entry(egui::FontFamily::Monospace)
                .or_default()
                .insert(0, name);
        }
    }

    ctx.set_fonts(fonts);
}

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size(Vec2::new(1280.0, 800.0))
            .with_min_inner_size(Vec2::new(800.0, 500.0))
            .with_title("Aether")
            .with_decorations(false)
            .with_transparent(true),
        ..Default::default()
    };

    eframe::run_native(
        "Aether",
        native_options,
        Box::new(|cc| {
            setup_custom_fonts(&cc.egui_ctx);
            Ok(Box::new(BrowserApp::new()))
        }),
    )
}

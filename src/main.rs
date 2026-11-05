use eframe::egui::{self, Vec2};
use rust_browser::app::BrowserApp;

fn setup_custom_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    let font_files = [
        ("liberation_sans", "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf"),
        ("liberation_sans_bold", "/usr/share/fonts/truetype/liberation/LiberationSans-Bold.ttf"),
        ("liberation_sans_italic", "/usr/share/fonts/truetype/liberation/LiberationSans-Italic.ttf"),
        ("liberation_sans_bold_italic", "/usr/share/fonts/truetype/liberation/LiberationSans-BoldItalic.ttf"),
        ("liberation_mono", "/usr/share/fonts/truetype/liberation/LiberationMono-Regular.ttf"),
        ("liberation_mono_bold", "/usr/share/fonts/truetype/liberation/LiberationMono-Bold.ttf"),
        ("liberation_mono_italic", "/usr/share/fonts/truetype/liberation/LiberationMono-Italic.ttf"),
        ("liberation_mono_bold_italic", "/usr/share/fonts/truetype/liberation/LiberationMono-BoldItalic.ttf"),
        ("noto_sans", "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf"),
        ("dejavu_sans", "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf"),
        ("dejavu_sans_bold", "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf"),
        ("noto_symbols", "/usr/share/fonts/truetype/noto/NotoSansSymbols-Regular.ttf"),
        ("noto_symbols2", "/usr/share/fonts/truetype/noto/NotoSansSymbols2-Regular.ttf"),
    ];

    let mut loaded = std::collections::HashSet::new();
    for (name, path) in &font_files {
        if let Ok(data) = std::fs::read(path) {
            fonts.font_data.insert(
                name.to_string(),
                std::sync::Arc::new(egui::FontData::from_owned(data)),
            );
            loaded.insert(*name);
        }
    }

    let push = |family: egui::FontFamily, names: &[&str], fonts: &mut egui::FontDefinitions| {
        let entries: Vec<String> = names
            .iter()
            .filter(|n| loaded.contains(*n))
            .map(|n| n.to_string())
            .collect();
        if entries.is_empty() {
            return;
        }
        let list = fonts.families.entry(family).or_default();
        for name in entries.into_iter().rev() {
            list.insert(0, name);
        }
    };

    push(egui::FontFamily::Proportional, &["liberation_sans", "noto_sans", "dejavu_sans"], &mut fonts);
    push(egui::FontFamily::Monospace, &["liberation_mono", "dejavu_sans"], &mut fonts);

    // Named faces used by the layout engine and the painter (src/text_metrics.rs).
    push(egui::FontFamily::Name("bold".into()), &["liberation_sans_bold", "dejavu_sans_bold"], &mut fonts);
    push(egui::FontFamily::Name("italic".into()), &["liberation_sans_italic", "dejavu_sans"], &mut fonts);
    push(
        egui::FontFamily::Name("bold-italic".into()),
        &["liberation_sans_bold_italic", "liberation_sans_bold", "dejavu_sans_bold"],
        &mut fonts,
    );
    push(egui::FontFamily::Name("mono-bold".into()), &["liberation_mono_bold", "liberation_mono"], &mut fonts);
    push(egui::FontFamily::Name("mono-italic".into()), &["liberation_mono_italic", "liberation_mono"], &mut fonts);
    push(
        egui::FontFamily::Name("mono-bold-italic".into()),
        &["liberation_mono_bold_italic", "liberation_mono_bold", "liberation_mono"],
        &mut fonts,
    );

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
            rust_browser::text_metrics::set_font_context(cc.egui_ctx.clone());
            Ok(Box::new(BrowserApp::new()))
        }),
    )
}

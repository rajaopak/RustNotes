use eframe::egui;
use rustnotes::app::{RustNotesApp, load_app_icon};
use rustnotes::crash_handler::setup_panic_handler;

fn main() -> eframe::Result<()> {
    // Fase 7.7: Logging & Crash Handler (tulis log crash ke %APPDATA%\RustNotes\logs\)
    setup_panic_handler();

    // Fase 7.3: File Association & CLI args (jika dipanggil dengan berkas .rnote / .zip)
    let args: Vec<String> = std::env::args().collect();
    let initial_file = if args.len() > 1 {
        let p = std::path::PathBuf::from(&args[1]);
        if p.exists() && p.is_file() {
            Some(p)
        } else {
            None
        }
    } else {
        None
    };

    // Fase 7.1: Muat icon aplikasi ke window viewport
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("RustNotes")
        .with_inner_size([1280.0, 800.0])
        .with_min_inner_size([800.0, 600.0]);

    if let Some(icon) = load_app_icon() {
        viewport = viewport.with_icon(icon);
    }

    let native_options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "RustNotes",
        native_options,
        Box::new(move |cc| {
            setup_custom_fonts(&cc.egui_ctx);
            let mut app = RustNotesApp::default();
            if let Some(path) = initial_file {
                app.open_external_file(&path);
            }
            Ok(Box::new(app))
        }),
    )
}

fn setup_custom_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let emoji_path = std::path::Path::new("C:\\Windows\\Fonts\\seguiemj.ttf");
    if emoji_path.exists()
        && let Ok(font_bytes) = std::fs::read(emoji_path)
    {
        fonts.font_data.insert(
            "seguiemj".to_owned(),
            std::sync::Arc::new(egui::FontData::from_owned(font_bytes)),
        );
        if let Some(prop) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
            prop.push("seguiemj".to_owned());
        }
        if let Some(mono) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
            mono.push("seguiemj".to_owned());
        }
    }
    ctx.set_fonts(fonts);
}

use crate::model::ThemeMode;
use eframe::egui::{self, Color32, Stroke, Visuals};

pub fn resolve_theme_mode(ctx: &egui::Context, theme_mode: ThemeMode) -> ThemeMode {
    match theme_mode {
        ThemeMode::Dark => ThemeMode::Dark,
        ThemeMode::Light => ThemeMode::Light,
        ThemeMode::System => match ctx.system_theme() {
            Some(egui::Theme::Light) => ThemeMode::Light,
            _ => ThemeMode::Dark,
        },
    }
}

pub fn setup_goodnotes_theme(ctx: &egui::Context, theme_mode: ThemeMode) {
    match resolve_theme_mode(ctx, theme_mode) {
        ThemeMode::Dark | ThemeMode::System => {
            let mut visuals = Visuals::dark();
            visuals.panel_fill = Color32::from_rgb(29, 27, 28); // #1d1b1c
            visuals.window_fill = Color32::from_rgb(45, 43, 44); // #2d2b2c
            visuals.extreme_bg_color = Color32::from_rgb(20, 19, 19); // #141313
            visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(45, 43, 44);
            visuals.widgets.noninteractive.bg_stroke =
                Stroke::new(1.0_f32, Color32::from_rgb(60, 57, 57));
            visuals.widgets.inactive.bg_fill = Color32::from_rgb(45, 43, 44);
            visuals.widgets.inactive.bg_stroke =
                Stroke::new(1.0_f32, Color32::from_rgb(60, 57, 57));
            visuals.widgets.hovered.bg_fill = Color32::from_rgb(56, 53, 54);
            visuals.widgets.hovered.bg_stroke =
                Stroke::new(1.5_f32, Color32::from_rgb(76, 147, 236));
            visuals.widgets.active.bg_fill = Color32::from_rgb(0, 114, 206);
            visuals.selection.bg_fill = Color32::from_rgb(39, 54, 91); // #27365b
            visuals.selection.stroke = Stroke::new(1.5_f32, Color32::from_rgb(76, 147, 236));
            ctx.set_visuals(visuals);
        }
        ThemeMode::Light => {
            let mut visuals = Visuals::light();
            visuals.panel_fill = Color32::from_rgb(244, 243, 239); // #f4f3ef
            visuals.window_fill = Color32::from_rgb(252, 251, 249);
            visuals.extreme_bg_color = Color32::from_rgb(255, 255, 255);
            visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(235, 233, 228);
            visuals.widgets.noninteractive.bg_stroke =
                Stroke::new(1.0_f32, Color32::from_rgb(205, 202, 196));
            visuals.widgets.inactive.bg_fill = Color32::from_rgb(232, 230, 225);
            visuals.widgets.inactive.bg_stroke =
                Stroke::new(1.0_f32, Color32::from_rgb(198, 195, 188));
            visuals.widgets.hovered.bg_fill = Color32::from_rgb(218, 224, 235);
            visuals.widgets.hovered.bg_stroke =
                Stroke::new(1.5_f32, Color32::from_rgb(0, 114, 206));
            visuals.widgets.active.bg_fill = Color32::from_rgb(0, 114, 206);
            visuals.selection.bg_fill = Color32::from_rgb(195, 220, 248);
            visuals.selection.stroke = Stroke::new(1.5_f32, Color32::from_rgb(0, 114, 206));
            ctx.set_visuals(visuals);
        }
    }
}

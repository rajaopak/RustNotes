use crate::model::{AppLanguage, ThemeMode};
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

/// Palet warna antarmuka terpadu untuk Dark Theme & Light Theme agar seluruh teks,
/// kartu, bilah mengambang, sidebar, dan kontrol selalu memiliki kontras tajam.
#[derive(Clone, Copy, Debug)]
pub struct UiPalette {
    pub is_dark: bool,
    pub bg_panel: Color32,
    pub bg_sidebar: Color32,
    pub bg_card: Color32,
    pub bg_card_hover: Color32,
    pub bg_card_selected: Color32,
    pub bg_elevated: Color32,
    pub bg_input: Color32,
    pub border_subtle: Color32,
    pub border_strong: Color32,
    pub border_accent: Color32,
    pub text_primary: Color32,
    pub text_secondary: Color32,
    pub text_muted: Color32,
    pub text_on_accent: Color32,
    pub accent: Color32,
    pub accent_soft_bg: Color32,
    pub accent_text: Color32,
    pub btn_neutral_bg: Color32,
    pub btn_neutral_hover: Color32,
    pub btn_neutral_text: Color32,
    pub danger: Color32,
    pub danger_soft_bg: Color32,
    pub success: Color32,
    pub warning: Color32,
}

impl UiPalette {
    pub fn from_mode(mode: ThemeMode) -> Self {
        match mode {
            ThemeMode::Light => Self::light(),
            ThemeMode::Dark | ThemeMode::System => Self::dark(),
        }
    }

    pub fn from_ctx(ctx: &egui::Context, theme_mode: ThemeMode) -> Self {
        Self::from_mode(resolve_theme_mode(ctx, theme_mode))
    }

    pub fn from_visuals(visuals: &Visuals) -> Self {
        if visuals.dark_mode {
            Self::dark()
        } else {
            Self::light()
        }
    }

    pub fn dark() -> Self {
        Self {
            is_dark: true,
            bg_panel: Color32::from_rgb(29, 27, 28),
            bg_sidebar: Color32::from_rgb(25, 24, 25),
            bg_card: Color32::from_rgb(42, 40, 42),
            bg_card_hover: Color32::from_rgb(54, 52, 55),
            bg_card_selected: Color32::from_rgb(39, 54, 91),
            bg_elevated: Color32::from_rgba_unmultiplied(30, 31, 36, 242),
            bg_input: Color32::from_rgb(20, 19, 20),
            border_subtle: Color32::from_rgb(65, 63, 68),
            border_strong: Color32::from_rgb(92, 90, 96),
            border_accent: Color32::from_rgb(76, 147, 236),
            text_primary: Color32::from_rgb(245, 246, 248),
            text_secondary: Color32::from_rgb(200, 204, 212),
            text_muted: Color32::from_rgb(148, 152, 160),
            text_on_accent: Color32::WHITE,
            accent: Color32::from_rgb(0, 114, 206),
            accent_soft_bg: Color32::from_rgb(39, 54, 91),
            accent_text: Color32::from_rgb(104, 172, 250),
            btn_neutral_bg: Color32::from_rgb(52, 56, 66),
            btn_neutral_hover: Color32::from_rgb(68, 74, 88),
            btn_neutral_text: Color32::from_rgb(240, 243, 248),
            danger: Color32::from_rgb(248, 102, 96),
            danger_soft_bg: Color32::from_rgb(175, 55, 55),
            success: Color32::from_rgb(102, 214, 134),
            warning: Color32::from_rgb(245, 185, 66),
        }
    }

    pub fn light() -> Self {
        Self {
            is_dark: false,
            bg_panel: Color32::from_rgb(246, 245, 242),
            bg_sidebar: Color32::from_rgb(238, 237, 233),
            bg_card: Color32::from_rgb(255, 255, 255),
            bg_card_hover: Color32::from_rgb(240, 245, 252),
            bg_card_selected: Color32::from_rgb(219, 234, 254),
            bg_elevated: Color32::from_rgba_unmultiplied(253, 253, 254, 248),
            bg_input: Color32::from_rgb(255, 255, 255),
            border_subtle: Color32::from_rgb(208, 212, 220),
            border_strong: Color32::from_rgb(168, 176, 190),
            border_accent: Color32::from_rgb(0, 114, 206),
            text_primary: Color32::from_rgb(26, 31, 40),
            text_secondary: Color32::from_rgb(68, 76, 90),
            text_muted: Color32::from_rgb(108, 116, 130),
            text_on_accent: Color32::WHITE,
            accent: Color32::from_rgb(0, 114, 206),
            accent_soft_bg: Color32::from_rgb(214, 232, 252),
            accent_text: Color32::from_rgb(0, 92, 175),
            btn_neutral_bg: Color32::from_rgb(225, 229, 236),
            btn_neutral_hover: Color32::from_rgb(208, 216, 228),
            btn_neutral_text: Color32::from_rgb(30, 36, 46),
            danger: Color32::from_rgb(192, 42, 42),
            danger_soft_bg: Color32::from_rgb(198, 52, 52),
            success: Color32::from_rgb(21, 128, 61),
            warning: Color32::from_rgb(180, 110, 10),
        }
    }

    pub fn theme_toggle_label(mode: ThemeMode, lang: AppLanguage) -> &'static str {
        match mode {
            ThemeMode::Dark => lang.tr("🌙 Gelap", "🌙 Dark"),
            ThemeMode::Light => lang.tr("☀ Terang", "☀ Light"),
            ThemeMode::System => lang.tr("💻 Sistem", "💻 System"),
        }
    }
}

pub fn setup_goodnotes_theme(ctx: &egui::Context, theme_mode: ThemeMode) {
    match resolve_theme_mode(ctx, theme_mode) {
        ThemeMode::Dark | ThemeMode::System => {
            let p = UiPalette::dark();
            let mut visuals = Visuals::dark();
            visuals.panel_fill = p.bg_panel;
            visuals.window_fill = Color32::from_rgb(45, 43, 44);
            visuals.window_stroke = Stroke::new(1.0_f32, p.border_subtle);
            visuals.extreme_bg_color = p.bg_input;
            visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(45, 43, 44);
            visuals.widgets.noninteractive.bg_stroke =
                Stroke::new(1.0_f32, Color32::from_rgb(60, 57, 57));
            visuals.widgets.inactive.bg_fill = Color32::from_rgb(45, 43, 44);
            visuals.widgets.inactive.bg_stroke =
                Stroke::new(1.0_f32, Color32::from_rgb(60, 57, 57));
            visuals.widgets.hovered.bg_fill = Color32::from_rgb(56, 53, 54);
            visuals.widgets.hovered.bg_stroke = Stroke::new(1.5_f32, p.border_accent);
            visuals.widgets.active.bg_fill = p.accent;
            visuals.selection.bg_fill = p.accent_soft_bg;
            visuals.selection.stroke = Stroke::new(1.5_f32, p.border_accent);
            ctx.set_visuals(visuals);
        }
        ThemeMode::Light => {
            let p = UiPalette::light();
            let mut visuals = Visuals::light();
            visuals.panel_fill = p.bg_panel;
            visuals.window_fill = Color32::from_rgb(252, 251, 249);
            visuals.window_stroke = Stroke::new(1.0_f32, p.border_subtle);
            visuals.extreme_bg_color = p.bg_input;
            visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(236, 235, 231);
            visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, p.border_subtle);
            visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, p.text_primary);
            visuals.widgets.inactive.bg_fill = p.btn_neutral_bg;
            visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, p.border_subtle);
            visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, p.text_primary);
            visuals.widgets.hovered.bg_fill = p.btn_neutral_hover;
            visuals.widgets.hovered.bg_stroke = Stroke::new(1.5_f32, p.accent);
            visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, p.text_primary);
            visuals.widgets.active.bg_fill = p.accent;
            visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, Color32::WHITE);
            visuals.selection.bg_fill = p.accent_soft_bg;
            visuals.selection.stroke = Stroke::new(1.5_f32, p.accent);
            ctx.set_visuals(visuals);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ui_palette_dark_and_light_contrast_polarity() {
        let dark = UiPalette::dark();
        let light = UiPalette::light();

        assert!(dark.is_dark);
        assert!(!light.is_dark);
        // Dark theme has light primary text over dark panel; Light theme has dark primary text over light panel
        assert!(dark.text_primary.r() > dark.bg_panel.r() + 150);
        assert!(light.bg_panel.r() > light.text_primary.r() + 150);
        assert!(light.bg_card.r() > light.text_secondary.r() + 120);
        assert!(light.btn_neutral_bg.r() > light.btn_neutral_text.r() + 140);
    }
}

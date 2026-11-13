use super::super::state::RustNotesApp;
use super::super::types::ShortcutModalTab;
use crate::model::ShortcutAction;
use crate::storage::{app_storage_dir, save_settings_to_dir};
use eframe::egui::{self, Align, Button, Color32, Layout, Pos2, Rect, RichText, Sense, Vec2};

impl RustNotesApp {
    pub fn render_shortcut_modal(&mut self, ctx: &egui::Context) {
        if !self.show_shortcut_modal {
            return;
        }

        let lang = self.settings.language;

        // Logika Penangkapan Rebinding Tombol saat pengguna sedang merekam shortcut baru
        if let Some(target_action) = self.rebinding_shortcut {
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.rebinding_shortcut = None;
            } else if let Some(new_binding) =
                crate::app::capture_rebinding_key(&ctx.input(|i| i.clone()))
            {
                self.settings.shortcuts.set(target_action, new_binding);
                let _ = save_settings_to_dir(&app_storage_dir(), &self.settings);
                self.rebinding_shortcut = None;
                self.toast_message = Some(
                    lang.tr(
                        "✅ Pintasan keyboard berhasil diperbarui!",
                        "✅ Keyboard shortcut updated successfully!",
                    )
                    .to_string(),
                );
            }
        }

        egui::Window::new(lang.tr(
            "⌨ Pengaturan & Panduan Pintasan Keyboard",
            "⌨ Keyboard Shortcuts & Settings",
        ))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.set_width(520.0);

            // Tab Switcher
            ui.horizontal(|ui| {
                if ui
                    .selectable_label(
                        self.shortcut_modal_tab == ShortcutModalTab::Customize,
                        lang.tr("⚙ Atur Pintasan Keyboard", "⚙ Customize Shortcuts"),
                    )
                    .clicked()
                {
                    self.shortcut_modal_tab = ShortcutModalTab::Customize;
                }
                if ui
                    .selectable_label(
                        self.shortcut_modal_tab == ShortcutModalTab::CheatSheet,
                        lang.tr("📖 Panduan Gestur & Kanvas", "📖 Canvas Gestures & Guide"),
                    )
                    .clicked()
                {
                    self.shortcut_modal_tab = ShortcutModalTab::CheatSheet;
                }
                if ui
                    .selectable_label(
                        self.shortcut_modal_tab == ShortcutModalTab::Inking,
                        lang.tr("✒️ Input Pena & Stylus", "✒️ Pen & Stylus"),
                    )
                    .clicked()
                {
                    self.shortcut_modal_tab = ShortcutModalTab::Inking;
                }
            });
            ui.separator();

            match self.shortcut_modal_tab {
                ShortcutModalTab::Customize => {
                    ui.label(
                        RichText::new(lang.tr(
                            "Klik pada tombol pintasan untuk merekam kombinasi tombol baru dari keyboard.",
                            "Click on any shortcut badge to record a new key combination from your keyboard.",
                        ))
                        .size(11.5)
                        .color(Color32::GRAY),
                    );
                    ui.add_space(4.0);

                    // Filter Pencarian
                    ui.horizontal(|ui| {
                        ui.label("🔍");
                        ui.text_edit_singleline(&mut self.shortcut_search_query);
                        if !self.shortcut_search_query.is_empty() && ui.small_button("✕").clicked()
                        {
                            self.shortcut_search_query.clear();
                        }
                    });
                    ui.add_space(6.0);

                    // Deteksi Konflik Pintasan
                    let mut binding_counts: std::collections::HashMap<String, usize> =
                        std::collections::HashMap::new();
                    for &act in &ShortcutAction::ALL {
                        let str_b = self.settings.shortcuts.get(act).display_string();
                        *binding_counts.entry(str_b).or_insert(0) += 1;
                    }

                    let search = self.shortcut_search_query.trim().to_lowercase();
                    let filtered_actions: Vec<ShortcutAction> = ShortcutAction::ALL
                        .iter()
                        .copied()
                        .filter(|&act| {
                            if search.is_empty() {
                                return true;
                            }
                            let lbl = act.label(lang).to_lowercase();
                            let b_str = self
                                .settings
                                .shortcuts
                                .get(act)
                                .display_string()
                                .to_lowercase();
                            lbl.contains(&search) || b_str.contains(&search)
                        })
                        .collect();

                    egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                        egui::Grid::new("custom_shortcuts_table")
                            .striped(true)
                            .spacing([16.0, 6.0])
                            .min_col_width(120.0)
                            .show(ui, |ui| {
                                ui.strong(lang.tr("Fungsi / Aksi", "Function / Action"));
                                ui.strong(lang.tr("Kombinasi Tombol", "Key Combination"));
                                ui.strong(lang.tr("Reset", "Reset"));
                                ui.end_row();

                                for act in filtered_actions {
                                    ui.label(act.label(lang));

                                    let current_binding = self.settings.shortcuts.get(act);
                                    let disp = current_binding.display_string();
                                    let is_rebinding = self.rebinding_shortcut == Some(act);
                                    let has_conflict =
                                        binding_counts.get(&disp).copied().unwrap_or(0) > 1;

                                    ui.horizontal(|ui| {
                                        let btn_text = if is_rebinding {
                                            RichText::new(lang.tr(
                                                "⏳ Tekan tombol... (Esc batal)",
                                                "⏳ Press key... (Esc cancels)",
                                            ))
                                            .color(Color32::from_rgb(255, 215, 0))
                                            .strong()
                                        } else {
                                            RichText::new(&disp).color(Color32::WHITE).strong()
                                        };

                                        let btn = Button::new(btn_text).fill(if is_rebinding {
                                            Color32::from_rgb(180, 80, 0)
                                        } else if has_conflict {
                                            Color32::from_rgb(140, 40, 40)
                                        } else {
                                            Color32::from_rgb(45, 60, 85)
                                        });

                                        if ui
                                            .add(btn)
                                            .on_hover_text(lang.tr(
                                                "Klik untuk merekam tombol baru",
                                                "Click to record new key",
                                            ))
                                            .clicked()
                                        {
                                            self.rebinding_shortcut = if is_rebinding {
                                                None
                                            } else {
                                                Some(act)
                                            };
                                        }

                                        if has_conflict && !is_rebinding {
                                            ui.label(
                                                RichText::new("⚠")
                                                    .color(Color32::from_rgb(255, 90, 80))
                                                    .size(11.0),
                                            )
                                            .on_hover_text(lang.tr(
                                                "Kombinasi tombol ini dipakai oleh lebih dari satu fungsi",
                                                "This key combination is used by multiple actions",
                                            ));
                                        }
                                    });

                                    // Tombol Reset Individu
                                    let is_default = current_binding == act.default_binding();
                                    if !is_default {
                                        if ui
                                            .small_button("↺")
                                            .on_hover_text(lang.tr(
                                                "Reset aksi ini ke bawaan",
                                                "Reset this action to default",
                                            ))
                                            .clicked()
                                        {
                                            self.settings
                                                .shortcuts
                                                .set(act, act.default_binding());
                                            let _ = save_settings_to_dir(
                                                &app_storage_dir(),
                                                &self.settings,
                                            );
                                        }
                                    } else {
                                        ui.label(
                                            RichText::new("•").size(10.0).color(Color32::DARK_GRAY),
                                        );
                                    }

                                    ui.end_row();
                                }
                            });
                    });

                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(4.0);

                    ui.horizontal(|ui| {
                        if ui
                            .button(lang.tr(
                                "↺ Reset Semua ke Bawaan",
                                "↺ Reset All to Defaults",
                            ))
                            .clicked()
                        {
                            self.settings.shortcuts.reset_all();
                            let _ = save_settings_to_dir(&app_storage_dir(), &self.settings);
                            self.rebinding_shortcut = None;
                            self.toast_message = Some(
                                lang.tr(
                                    "↺ Seluruh pintasan dikembalikan ke bawaan.",
                                    "↺ All shortcuts restored to defaults.",
                                )
                                .to_string(),
                            );
                        }

                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.button(lang.tr("Tutup (Esc)", "Close (Esc)")).clicked() {
                                self.rebinding_shortcut = None;
                                self.show_shortcut_modal = false;
                            }
                            if ui
                                .add(
                                    Button::new(
                                        RichText::new(lang.tr(
                                            "💾 Simpan Pengaturan",
                                            "💾 Save Settings",
                                        ))
                                        .strong()
                                        .color(Color32::WHITE),
                                    )
                                    .fill(Color32::from_rgb(0, 114, 206)),
                                )
                                .clicked()
                            {
                                let _ = save_settings_to_dir(&app_storage_dir(), &self.settings);
                                self.rebinding_shortcut = None;
                                self.show_shortcut_modal = false;
                                self.toast_message = Some(
                                    lang.tr(
                                        "💾 Pengaturan pintasan keyboard disimpan.",
                                        "💾 Keyboard shortcut settings saved.",
                                    )
                                    .to_string(),
                                );
                            }
                        });
                    });
                }

                ShortcutModalTab::CheatSheet => {
                    ui.label(
                        RichText::new(lang.tr(
                            "Panduan Lengkap Gestur Mouse, Pintasan Navigasi & Kanvas",
                            "Full Guide to Mouse Gestures, Navigation & Canvas Shortcuts",
                        ))
                        .strong()
                        .size(13.0),
                    );
                    ui.add_space(4.0);

                    egui::ScrollArea::vertical().max_height(340.0).show(ui, |ui| {
                        egui::Grid::new("shortcut_table")
                            .striped(true)
                            .spacing([24.0, 6.0])
                            .show(ui, |ui| {
                                ui.strong(lang.tr("Pintasan / Gestur", "Shortcut / Gesture"));
                                ui.strong(lang.tr("Fungsi", "Function"));
                                ui.end_row();

                                ui.label("Ctrl + Scroll / Pinch");
                                ui.label(lang.tr(
                                    "Zoom In / Out Sesuai Posisi Kursor Mouse",
                                    "Zoom In / Out Centered at Cursor Position",
                                ));
                                ui.end_row();

                                ui.label("Arrow Keys (↑ ↓ ← →)");
                                ui.label(lang.tr(
                                    "Geser / Pan Kanvas ke 4 Arah (Shift + Panah: Cepat)",
                                    "Pan Canvas in 4 Directions (Shift + Arrow: Fast)",
                                ));
                                ui.end_row();

                                ui.label("Shift + Drag Handle");
                                ui.label(lang.tr(
                                    "Skala Proporsional (Kunci Rasio Aspek)",
                                    "Proportional Scaling (Lock Aspect Ratio)",
                                ));
                                ui.end_row();

                                ui.label("Drag Handle Atas ⟳");
                                ui.label(lang.tr(
                                    "Rotasi Bebas Objek Terpilih (Snap 15° saat dekat)",
                                    "Free Rotation of Selected Objects (15° Snap)",
                                ));
                                ui.end_row();

                                ui.label(lang.tr("Klik Kanan Kanvas", "Right Click Canvas"));
                                ui.label(lang.tr(
                                    "Menu Konteks (Balik H/V, Depan/Belakang, Kunci Objek)",
                                    "Context Menu (Flip H/V, Order, Lock Objects)",
                                ));
                                ui.end_row();

                                ui.label("PageUp / PageDown");
                                ui.label(lang.tr(
                                    "Navigasi Halaman Sebelumnya / Berikutnya",
                                    "Navigate Previous / Next Page",
                                ));
                                ui.end_row();

                                ui.label("Ctrl + Home / End");
                                ui.label(lang.tr(
                                    "Lompat ke Halaman Pertama / Terakhir",
                                    "Jump to First / Last Page",
                                ));
                                ui.end_row();

                                ui.label("Esc");
                                ui.label(lang.tr(
                                    "Tutup Dialog Pop-up / Batalkan Seleksi Objek",
                                    "Close Pop-ups / Deselect Objects",
                                ));
                                ui.end_row();
                            });
                    });

                    ui.add_space(10.0);
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button(lang.tr("Tutup (Esc)", "Close (Esc)")).clicked() {
                            self.show_shortcut_modal = false;
                        }
                    });
                }
                ShortcutModalTab::Inking => {
                    ui.label(
                        RichText::new(lang.tr(
                            "Konfigurasi sensitivitas tekanan stylus, toleransi input, dan palm rejection.",
                            "Configure stylus pressure sensitivity, input thresholds, and palm rejection.",
                        ))
                        .size(11.5)
                        .color(Color32::GRAY),
                    );
                    ui.add_space(8.0);

                    let mut settings_changed = false;

                    ui.group(|ui| {
                        ui.label(
                            RichText::new(lang.tr(
                                "Sensitivitas Tekanan (Pressure Sensitivity)",
                                "Pressure Sensitivity",
                            ))
                            .strong()
                            .size(13.0),
                        );
                        ui.add_space(4.0);

                        if ui
                            .checkbox(
                                &mut self.settings.pressure_sensitivity_enabled,
                                lang.tr(
                                    "Aktifkan Sensitivitas Tekanan",
                                    "Enable Pressure Sensitivity",
                                ),
                            )
                            .changed()
                        {
                            settings_changed = true;
                        }

                        if self.settings.pressure_sensitivity_enabled {
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                ui.label(lang.tr("Tekanan Minimum:", "Minimum Pressure:"));
                                if ui
                                    .add(
                                        egui::Slider::new(
                                            &mut self.settings.pressure_min,
                                            0.05..=0.8,
                                        )
                                        .step_by(0.05),
                                    )
                                    .changed()
                                {
                                    settings_changed = true;
                                }
                            });

                            ui.horizontal(|ui| {
                                ui.label(lang.tr("Faktor Sensitivitas:", "Sensitivity Multiplier:"));
                                if ui
                                    .add(
                                        egui::Slider::new(
                                            &mut self.settings.pressure_multiplier,
                                            0.5..=3.0,
                                        )
                                        .step_by(0.1),
                                    )
                                    .changed()
                                {
                                    settings_changed = true;
                                }
                            });

                            ui.add_space(6.0);
                            ui.label(lang.tr(
                                "Pratinjau Respons Tekanan:",
                                "Pressure Response Preview:",
                            ));
                            let (rect, _) = ui.allocate_exact_size(
                                Vec2::new(ui.available_width(), 16.0),
                                Sense::hover(),
                            );
                            let painter = ui.painter_at(rect);
                            painter.rect_filled(rect, 4.0, Color32::from_rgb(40, 44, 52));
                            let steps = 40;
                            for s in 0..steps {
                                let raw = s as f32 / (steps - 1) as f32;
                                let eff = self.settings.effective_pressure(raw);
                                let x = rect.left() + (rect.width() * raw);
                                let bar_h = rect.height() * eff;
                                let y = rect.bottom() - bar_h;
                                let col = Color32::from_rgb(76, 147, 236);
                                painter.rect_filled(
                                    Rect::from_min_max(
                                        Pos2::new(x, y),
                                        Pos2::new(x + (rect.width() / steps as f32), rect.bottom()),
                                    ),
                                    0.0,
                                    col,
                                );
                            }
                        }
                    });

                    ui.add_space(6.0);
                    ui.group(|ui| {
                        ui.label(
                            RichText::new(lang.tr(
                                "Palm Rejection & Penolakan Sentuhan",
                                "Palm Rejection",
                            ))
                            .strong()
                            .size(13.0),
                        );
                        ui.add_space(4.0);
                        if ui
                            .checkbox(
                                &mut self.canvas_state.palm_rejection_enabled,
                                lang.tr(
                                    "Aktifkan Palm Rejection Otomatis",
                                    "Enable Automatic Palm Rejection",
                                ),
                            )
                            .changed()
                        {
                            self.settings.palm_rejection =
                                self.canvas_state.palm_rejection_enabled;
                            settings_changed = true;
                        }
                    });

                    ui.add_space(6.0);
                    ui.group(|ui| {
                        ui.label(
                            RichText::new(lang.tr(
                                "Highlighter & Transparansi",
                                "Highlighter & Opacity",
                            ))
                            .strong()
                            .size(13.0),
                        );
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.label(lang.tr("Transparansi (Opacity):", "Highlighter Opacity:"));
                            let pct = (self.settings.highlighter_opacity * 100.0).round() as i32;
                            let mut slider_val = self.settings.highlighter_opacity;
                            if ui
                                .add(
                                    egui::Slider::new(&mut slider_val, 0.15..=0.70)
                                        .step_by(0.01)
                                        .text(format!("{pct}%")),
                                )
                                .changed()
                            {
                                self.settings.highlighter_opacity = slider_val;
                                settings_changed = true;
                            }
                        });

                        ui.add_space(4.0);
                        if ui
                            .checkbox(
                                &mut self.settings.highlighter_pressure_enabled,
                                lang.tr(
                                    "Aktifkan Sensitivitas Tekanan pada Highlighter",
                                    "Enable Pressure Sensitivity for Highlighter",
                                ),
                            )
                            .changed()
                        {
                            settings_changed = true;
                        }

                        ui.add_space(6.0);
                        ui.label(lang.tr(
                            "Pratinjau Highlighter:",
                            "Highlighter Preview:",
                        ));
                        let (rect, _) = ui.allocate_exact_size(
                            Vec2::new(ui.available_width(), 24.0),
                            Sense::hover(),
                        );
                        let painter = ui.painter_at(rect);
                        painter.rect_filled(rect, 4.0, Color32::from_rgb(245, 247, 250));
                        // Text sample under highlighter
                        painter.text(
                            rect.left_center() + Vec2::new(12.0, 0.0),
                            egui::Align2::LEFT_CENTER,
                            "RustNotes High-End Inking Engine",
                            egui::FontId::proportional(13.0),
                            Color32::from_rgb(20, 25, 35),
                        );
                        // Highlighter overlay strip
                        let hl_color = Color32::from_rgba_unmultiplied(
                            255,
                            225,
                            50,
                            (self.settings.highlighter_opacity * 255.0).clamp(20.0, 255.0) as u8,
                        );
                        let hl_rect = Rect::from_min_max(
                            rect.left_top() + Vec2::new(8.0, 4.0),
                            rect.right_bottom() - Vec2::new(8.0, 4.0),
                        );
                        painter.rect_filled(hl_rect, 2.0, hl_color);
                    });

                    if settings_changed {
                        self.sync_inking_settings_to_canvas();
                        let _ = save_settings_to_dir(&app_storage_dir(), &self.settings);
                    }

                    ui.add_space(10.0);
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button(lang.tr("Tutup (Esc)", "Close (Esc)")).clicked() {
                            self.show_shortcut_modal = false;
                        }
                        if ui
                            .button(lang.tr("Reset ke Default", "Reset to Defaults"))
                            .clicked()
                        {
                            self.reset_inking_settings_to_defaults();
                            let _ = save_settings_to_dir(&app_storage_dir(), &self.settings);
                        }
                    });
                }
            }
        });
    }

    pub(crate) fn sync_inking_settings_to_canvas(&mut self) {
        self.canvas_state.pressure_sensitivity_enabled = self.settings.pressure_sensitivity_enabled;
        self.canvas_state.pressure_min = self.settings.pressure_min;
        self.canvas_state.pressure_multiplier = self.settings.pressure_multiplier;
        self.canvas_state.palm_rejection_enabled = self.settings.palm_rejection;
        self.canvas_state.highlighter_opacity = self.settings.highlighter_opacity;
        self.canvas_state.highlighter_pressure_enabled = self.settings.highlighter_pressure_enabled;
        self.canvas_state.mesh_cache.clear();
    }

    pub(crate) fn reset_inking_settings_to_defaults(&mut self) {
        let defaults = crate::model::AppSettings::default();
        self.settings.pressure_sensitivity_enabled = defaults.pressure_sensitivity_enabled;
        self.settings.pressure_min = defaults.pressure_min;
        self.settings.pressure_multiplier = defaults.pressure_multiplier;
        self.settings.palm_rejection = defaults.palm_rejection;
        self.settings.highlighter_opacity = defaults.highlighter_opacity;
        self.settings.highlighter_pressure_enabled = defaults.highlighter_pressure_enabled;
        self.sync_inking_settings_to_canvas();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AppLanguage, AppSettings, ColorData, KeyBinding, ThemeMode};
    use crate::storage::load_settings_from_dir;

    #[test]
    fn test_reset_inking_settings_resets_all_fields_to_default_and_clears_mesh_cache() {
        let mut app = RustNotesApp::default();
        let defaults = AppSettings::default();

        // Ubah seluruh pengaturan inking menjauh dari default
        app.settings.pressure_sensitivity_enabled = !defaults.pressure_sensitivity_enabled;
        app.settings.pressure_min = 0.45;
        app.settings.pressure_multiplier = 1.85;
        app.settings.palm_rejection = !defaults.palm_rejection;
        app.settings.highlighter_opacity = 0.65;
        app.settings.highlighter_pressure_enabled = !defaults.highlighter_pressure_enabled;
        app.canvas_state.pressure_sensitivity_enabled = app.settings.pressure_sensitivity_enabled;
        app.canvas_state.pressure_min = 0.45;
        app.canvas_state.pressure_multiplier = 1.85;
        app.canvas_state.palm_rejection_enabled = app.settings.palm_rejection;
        app.canvas_state.highlighter_opacity = 0.65;
        app.canvas_state.highlighter_pressure_enabled = app.settings.highlighter_pressure_enabled;
        app.canvas_state
            .mesh_cache
            .insert(("stroke-1".to_string(), 100), egui::Mesh::default());

        // Ubah pengaturan non-inking untuk memastikan nilainya tetap terjaga
        app.settings.theme_mode = ThemeMode::Light;
        app.settings.pen_widths = vec![1.5, 9.0];
        app.settings.color_swatches = vec![ColorData::from(Color32::from_rgb(12, 34, 56))];
        app.settings.zoom_window_scale = 3.0;
        app.settings.language = AppLanguage::English;
        app.settings.auto_shape_enabled = false;
        let custom_binding = KeyBinding {
            key: "K".to_string(),
            ctrl: true,
            alt: false,
            shift: true,
        };
        app.settings
            .shortcuts
            .set(ShortcutAction::ToolPen, custom_binding.clone());
        let expected_pen_widths = app.settings.pen_widths.clone();
        let expected_swatches = app.settings.color_swatches.clone();

        app.reset_inking_settings_to_defaults();

        assert_eq!(
            app.settings.pressure_sensitivity_enabled,
            defaults.pressure_sensitivity_enabled
        );
        assert_eq!(app.settings.pressure_min, defaults.pressure_min);
        assert_eq!(
            app.settings.pressure_multiplier,
            defaults.pressure_multiplier
        );
        assert_eq!(app.settings.palm_rejection, defaults.palm_rejection);
        assert_eq!(
            app.settings.highlighter_opacity,
            defaults.highlighter_opacity
        );
        assert_eq!(
            app.settings.highlighter_pressure_enabled,
            defaults.highlighter_pressure_enabled
        );

        assert_eq!(
            app.canvas_state.pressure_sensitivity_enabled,
            defaults.pressure_sensitivity_enabled
        );
        assert_eq!(app.canvas_state.pressure_min, defaults.pressure_min);
        assert_eq!(
            app.canvas_state.pressure_multiplier,
            defaults.pressure_multiplier
        );
        assert_eq!(
            app.canvas_state.palm_rejection_enabled,
            defaults.palm_rejection
        );
        assert_eq!(
            app.canvas_state.highlighter_opacity,
            defaults.highlighter_opacity
        );
        assert_eq!(
            app.canvas_state.highlighter_pressure_enabled,
            defaults.highlighter_pressure_enabled
        );
        assert!(app.canvas_state.mesh_cache.is_empty());

        // Non-inking settings harus tetap dipertahankan
        assert_eq!(app.settings.theme_mode, ThemeMode::Light);
        assert_eq!(app.settings.pen_widths, expected_pen_widths);
        assert_eq!(app.settings.color_swatches, expected_swatches);
        assert_eq!(app.settings.zoom_window_scale, 3.0);
        assert_eq!(app.settings.language, AppLanguage::English);
        assert!(!app.settings.auto_shape_enabled);
        assert_eq!(
            app.settings.shortcuts.get(ShortcutAction::ToolPen),
            custom_binding
        );
    }

    #[test]
    fn test_reset_inking_settings_persists_defaults_cleanly_to_dir() {
        let dir = tempfile::tempdir().expect("temp dir");
        let defaults = AppSettings::default();
        let mut app = RustNotesApp::default();

        app.settings.pressure_min = 0.4;
        app.settings.palm_rejection = false;
        app.settings.highlighter_opacity = 0.6;
        app.settings.highlighter_pressure_enabled = true;
        app.settings.theme_mode = ThemeMode::Light;

        app.reset_inking_settings_to_defaults();
        save_settings_to_dir(dir.path(), &app.settings).expect("save reset settings");

        let reloaded = load_settings_from_dir(dir.path());
        assert_eq!(reloaded.pressure_min, defaults.pressure_min);
        assert_eq!(reloaded.pressure_multiplier, defaults.pressure_multiplier);
        assert_eq!(reloaded.palm_rejection, defaults.palm_rejection);
        assert_eq!(reloaded.highlighter_opacity, defaults.highlighter_opacity);
        assert_eq!(
            reloaded.highlighter_pressure_enabled,
            defaults.highlighter_pressure_enabled
        );
        assert_eq!(reloaded.theme_mode, ThemeMode::Light);
    }
}

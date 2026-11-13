mod document_modals;
mod page_modals;
mod settings_modal;

use super::state::RustNotesApp;
use eframe::egui::{self, Align, Color32, Layout, RichText};

impl RustNotesApp {
    pub fn render_editor_modals(&mut self, ctx: &egui::Context, nb_idx: usize) {
        let notebook_id = self.library.notebooks.get(nb_idx).map(|nb| nb.id.clone());

        if let Some(ref notebook_id) = notebook_id {
            self.render_page_resize_modal(ctx, nb_idx, notebook_id);
            self.render_add_page_modal(ctx, nb_idx);
            self.render_in_doc_search_modal(ctx, nb_idx);
            self.render_add_outline_modal(ctx, nb_idx, notebook_id);
            self.render_save_template_modal(ctx, nb_idx);
        }

        // Pengaturan & About bersifat global sehingga tetap tersedia walau nb_idx tidak valid.
        self.render_shortcut_modal(ctx);

        if let Some(ref notebook_id) = notebook_id {
            self.render_ocr_modal(ctx, nb_idx, notebook_id);
        } else {
            self.close_invalid_notebook_modals();
        }

        self.render_about_modal(ctx);
    }

    fn close_invalid_notebook_modals(&mut self) {
        self.show_page_resize_modal = false;
        self.show_add_page_modal = false;
        self.show_in_doc_search = false;
        self.show_add_outline_modal = false;
        self.show_save_template_modal = false;
        self.show_ocr_modal = false;
    }

    fn render_about_modal(&mut self, ctx: &egui::Context) {
        // Fase 7: About Dialog Modal
        if self.show_about_modal {
            egui::Window::new("ℹ️ Tentang RustNotes")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.set_width(380.0);
                    ui.vertical_centered(|ui| {
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new("RustNotes")
                                .size(22.0)
                                .strong()
                                .color(Color32::from_rgb(0, 114, 206)),
                        );
                        ui.label(
                            RichText::new(format!("Versi {}", env!("CARGO_PKG_VERSION")))
                                .size(12.0)
                                .color(Color32::GRAY),
                        );
                        ui.add_space(6.0);
                        ui.label(
                            RichText::new(
                                "Aplikasi Buku Catatan Digital 100% Pure Rust\nGPU Hardware-Accelerated (eframe/egui)",
                            )
                            .size(12.0)
                            .color(Color32::WHITE),
                        );
                    });
                    ui.add_space(12.0);
                    ui.separator();
                    ui.add_space(8.0);

                    egui::Grid::new("about_grid")
                        .spacing([16.0, 6.0])
                        .show(ui, |ui| {
                            ui.label(RichText::new("Format Berkas:").strong());
                            ui.label(".rnote (Zip Archive + JSON + Assets)");
                            ui.end_row();

                            ui.label(RichText::new("Mesin Inking:").strong());
                            ui.label("Bézier Ribbon Mesh + Spatial Grid Index");
                            ui.end_row();

                            ui.label(RichText::new("Penyimpanan:").strong());
                            ui.label("Atomic JSON (%APPDATA%\\RustNotes)");
                            ui.end_row();

                            ui.label(RichText::new("Target OS:").strong());
                            ui.label("Windows 10 / 11 64-bit (Native)");
                            ui.end_row();

                            ui.label(RichText::new("Lisensi:").strong());
                            ui.label("MIT / Apache-2.0 (Zero Electron/Node)");
                            ui.end_row();
                        });

                    ui.add_space(14.0);
                    ui.separator();
                    ui.add_space(8.0);
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button("Tutup").clicked() {
                            self.show_about_modal = false;
                        }
                    });
                });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Notebook, PageBackground, PageTemplate};

    #[test]
    fn test_render_editor_modals_invalid_nb_idx_closes_notebook_modals_and_keeps_global() {
        let mut app = RustNotesApp {
            show_page_resize_modal: true,
            show_add_page_modal: true,
            show_in_doc_search: true,
            show_add_outline_modal: true,
            show_save_template_modal: true,
            show_ocr_modal: true,
            show_shortcut_modal: true,
            show_about_modal: true,
            ..Default::default()
        };

        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            app.render_editor_modals(ctx, 42);
        });

        assert!(!app.show_page_resize_modal);
        assert!(!app.show_add_page_modal);
        assert!(!app.show_in_doc_search);
        assert!(!app.show_add_outline_modal);
        assert!(!app.show_save_template_modal);
        assert!(!app.show_ocr_modal);
        assert!(app.show_shortcut_modal);
        assert!(app.show_about_modal);
    }

    #[test]
    fn test_render_modals_empty_pages_do_not_panic_or_underflow() {
        let mut app = RustNotesApp::default();
        let mut nb = Notebook::new("Empty NB", PageTemplate::Blank, Color32::WHITE);
        nb.pages.clear();
        app.library.notebooks.push(nb);
        app.current_page_index = 5;
        app.resize_target_page_index = 3;

        app.show_page_resize_modal = true;
        app.show_add_page_modal = true;
        app.show_in_doc_search = true;
        app.doc_search_query = "catatan".to_string();
        app.show_add_outline_modal = true;
        app.show_save_template_modal = true;
        app.show_ocr_modal = true;

        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            app.render_editor_modals(ctx, 0);
        });

        assert!(!app.show_page_resize_modal);
        assert!(app.show_add_page_modal);
        assert!(!app.show_add_outline_modal);
        assert!(!app.show_save_template_modal);
        assert!(!app.show_ocr_modal);
        assert!(app.toast_message.is_some());
    }

    #[test]
    fn test_render_modals_single_and_oob_page_indices_clamp_safely() {
        let mut app = RustNotesApp::default();
        let nb = Notebook::new("Single Page NB", PageTemplate::Ruled, Color32::WHITE);
        app.library.notebooks.push(nb);
        app.current_page_index = 99;
        app.resize_target_page_index = 99;

        app.show_page_resize_modal = true;
        app.show_add_page_modal = true;
        app.show_add_outline_modal = true;
        app.show_save_template_modal = true;
        app.show_ocr_modal = true;

        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            app.render_editor_modals(ctx, 0);
        });

        assert!(app.show_page_resize_modal);
        assert_eq!(app.resize_target_page_index, 0);
        assert!(app.show_add_page_modal);
        assert!(app.show_add_outline_modal);
        assert!(app.show_save_template_modal);
        assert!(app.show_ocr_modal);
    }

    #[test]
    fn test_apply_page_resize_and_template_bounds_and_updates_template_background() {
        let mut nb = Notebook::new("Resize NB", PageTemplate::Blank, Color32::WHITE);
        nb.pages[0].background = Some(PageBackground::Template(PageTemplate::Blank));

        page_modals::apply_page_resize_and_template(&mut nb, 0, 800.0, 1000.0, PageTemplate::Grid);
        assert_eq!(nb.pages[0].width, 800.0);
        assert_eq!(nb.pages[0].height, 1000.0);
        assert_eq!(nb.pages[0].template, PageTemplate::Grid);
        assert_eq!(
            nb.pages[0].background,
            Some(PageBackground::Template(PageTemplate::Grid))
        );

        // Indeks di luar batas tidak boleh panic atau merusak halaman yang ada
        page_modals::apply_page_resize_and_template(
            &mut nb,
            99,
            600.0,
            600.0,
            PageTemplate::Dotted,
        );
        assert_eq!(nb.pages[0].width, 800.0);
    }
}

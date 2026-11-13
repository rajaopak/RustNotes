mod document_modals;
mod page_modals;
mod settings_modal;

use super::state::RustNotesApp;
use eframe::egui::{self, Align, Color32, Layout, RichText};

impl RustNotesApp {
    pub fn render_editor_modals(&mut self, ctx: &egui::Context, nb_idx: usize) {
        let notebook_id = self.library.notebooks[nb_idx].id.clone();

        self.render_page_resize_modal(ctx, nb_idx, &notebook_id);
        self.render_add_page_modal(ctx, nb_idx);
        self.render_in_doc_search_modal(ctx, nb_idx);
        self.render_add_outline_modal(ctx, nb_idx, &notebook_id);
        self.render_save_template_modal(ctx, nb_idx);
        self.render_shortcut_modal(ctx);
        self.render_ocr_modal(ctx, nb_idx, &notebook_id);
        self.render_about_modal(ctx);
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

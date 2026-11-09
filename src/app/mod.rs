pub mod cover;
pub mod document_ops;
pub mod editor_navbar;
pub mod editor_sidebar;
pub mod editor_view;
pub mod library_view;
pub mod modals;
pub mod shortcuts;
pub mod state;
pub mod theme;
pub mod types;

pub use cover::*;
pub use shortcuts::*;
pub use state::*;
pub use theme::*;
pub use types::*;

use crate::storage::{LoadStatus, app_storage_dir, save_notebook_to_dir};
use eframe::egui::{self, Align, Color32, Layout, RichText};
use std::time::Instant;

impl eframe::App for RustNotesApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Fase 8.9: Aktifkan AccessKit bawaan egui untuk pembaca layar & aksesibilitas
        ctx.enable_accesskit();

        // Fase 8.5 & G4.4: Sinkronkan status Palm Rejection & AutoShape ke CanvasState
        self.canvas_state.palm_rejection_enabled = self.settings.palm_rejection;
        self.canvas_state.auto_shape_enabled = self.settings.auto_shape_enabled;

        // Apply Goodnotes theme styling (Dark / Light / System OS Windows)
        setup_goodnotes_theme(ctx, self.settings.theme_mode);

        // Fase 6: Debounced per-notebook save (1.5 detik setelah modifikasi terakhir)
        if !self.dirty_notebooks.is_empty() && !self.is_read_only {
            let debounce = std::time::Duration::from_millis(1500);
            let storage_dir = app_storage_dir();
            let now = Instant::now();
            let ready_to_save: Vec<String> = self
                .dirty_notebooks
                .iter()
                .filter(|id| {
                    self.last_modified
                        .get(*id)
                        .map(|t| now.duration_since(*t) >= debounce)
                        .unwrap_or(true)
                })
                .cloned()
                .collect();

            for nb_id in &ready_to_save {
                if let Some(nb) = self.library.notebooks.iter().find(|n| &n.id == nb_id) {
                    let result = save_notebook_to_dir(&storage_dir, nb);
                    if result.is_err() {
                        self.toast_message = Some(format!(
                            "⚠ Gagal menyimpan notebook '{}'. Coba lagi.",
                            nb.title
                        ));
                    }
                }
                self.dirty_notebooks.remove(nb_id);
                self.last_modified.remove(nb_id);
            }

            // Juga perbarui index jika ada yang berubah
            if !ready_to_save.is_empty() {
                self.sync_library_index();
            }

            // Minta repaint jika masih ada yang menunggu debounce
            if !self.dirty_notebooks.is_empty() {
                ctx.request_repaint_after(std::time::Duration::from_millis(300));
            }
        }

        // Render toast / banner if active
        let mut dismiss_toast = false;
        if let Some(msg) = &self.toast_message {
            egui::TopBottomPanel::bottom("status_banner")
                .exact_height(34.0)
                .show(ctx, |ui| {
                    ui.horizontal_centered(|ui| {
                        let is_err = self.is_read_only
                            || matches!(self.load_status, LoadStatus::CorruptedQuarantined { .. });
                        let text_color = if is_err {
                            Color32::from_rgb(255, 120, 120)
                        } else {
                            Color32::from_rgb(120, 220, 140)
                        };
                        ui.label(RichText::new(msg).color(text_color).strong().size(12.0));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.button("✕").clicked() {
                                dismiss_toast = true;
                            }
                        });
                    });
                });
        }
        if dismiss_toast {
            self.toast_message = None;
        }

        self.canvas_state.active_notebook_id = self.active_notebook_id.clone();

        // Fase 3: Handle Drag & Drop Files
        let dropped_files = ctx.input(|i| i.raw.dropped_files.clone());
        for file in dropped_files {
            if let Some(path) = &file.path {
                let ext = path
                    .extension()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                if ext == "pdf" {
                    if let Some(id) = &self.active_notebook_id
                        && let Some(idx) = self.library.notebooks.iter().position(|n| &n.id == id)
                    {
                        self.import_pdf_pages_to_active(idx, path);
                    } else {
                        self.import_pdf_as_notebook(path);
                    }
                } else if ["png", "jpg", "jpeg", "webp"].contains(&ext.as_str())
                    && let Some(id) = &self.active_notebook_id
                    && let Some(idx) = self.library.notebooks.iter().position(|n| &n.id == id)
                    && let Ok(bytes) = std::fs::read(path)
                {
                    self.insert_image_to_page(idx, self.current_page_index, &bytes, &ext, None);
                    self.toast_message =
                        Some("✅ Gambar berhasil disisipkan via Drag & Drop.".to_string());
                }
            }
        }

        if self.active_notebook_id.is_some() {
            self.render_editor_view(ctx);
        } else {
            self.render_library_view(ctx);
        }
    }
}

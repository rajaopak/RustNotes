mod document_modals;
mod page_modals;
mod settings_modal;

use super::state::RustNotesApp;
use eframe::egui::{self, Align, Layout, RichText};

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

    pub(crate) fn render_about_modal(&mut self, ctx: &egui::Context) {
        // Fase 7: About Dialog Modal
        if self.show_about_modal {
            let lang = self.settings.language;
            let palette = super::theme::UiPalette::from_ctx(ctx, self.settings.theme_mode);
            egui::Window::new(lang.tr("ℹ️ Tentang RustNotes", "ℹ️ About RustNotes"))
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.set_width(390.0);
                    ui.vertical_centered(|ui| {
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new("RustNotes")
                                .size(22.0)
                                .strong()
                                .color(palette.accent),
                        );
                        ui.label(
                            RichText::new(format!(
                                "{} {}",
                                lang.tr("Versi", "Version"),
                                env!("CARGO_PKG_VERSION")
                            ))
                            .size(12.0)
                            .color(palette.text_muted),
                        );
                        ui.add_space(6.0);
                        ui.label(
                            RichText::new(lang.tr(
                                "Aplikasi Buku Catatan Digital 100% Pure Rust\nGPU Hardware-Accelerated (eframe/egui)",
                                "100% Pure Rust Digital Notebook Workspace\nGPU Hardware-Accelerated (eframe/egui)",
                            ))
                            .size(12.0)
                            .color(palette.text_primary),
                        );
                    });
                    ui.add_space(12.0);
                    ui.separator();
                    ui.add_space(8.0);

                    egui::Grid::new("about_grid")
                        .spacing([16.0, 6.0])
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new(lang.tr("Format Berkas:", "File Format:")).strong(),
                            );
                            ui.label(".rnote (Zip Archive + JSON + Assets)");
                            ui.end_row();

                            ui.label(
                                RichText::new(lang.tr("Mesin Inking:", "Inking Engine:")).strong(),
                            );
                            ui.label("Bézier Ribbon Mesh + Spatial Grid Index");
                            ui.end_row();

                            ui.label(
                                RichText::new(lang.tr("Penyimpanan:", "Storage:")).strong(),
                            );
                            ui.label("Atomic JSON (%LOCALAPPDATA%\\RustNotes)");
                            ui.end_row();

                            ui.label(RichText::new("Target OS:").strong());
                            ui.label("Windows 10 / 11 64-bit (Native)");
                            ui.end_row();

                            ui.label(RichText::new(lang.tr("Lisensi:", "License:")).strong());
                            ui.label("MIT (Zero Web Runtime + PDFium BSD-3)");
                            ui.end_row();
                        });

                    ui.add_space(14.0);
                    ui.separator();
                    ui.add_space(8.0);
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button(lang.tr("Tutup", "Close")).clicked() {
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
    use eframe::egui::Color32;

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

    fn collect_shape_text_rects(shape: &egui::Shape, target: &str, out: &mut Vec<egui::Rect>) {
        match shape {
            egui::Shape::Text(text_shape) => {
                if text_shape.galley.text() == target {
                    out.push(text_shape.visual_bounding_rect());
                }
            }
            egui::Shape::Vec(children) => {
                for child in children {
                    collect_shape_text_rects(child, target, out);
                }
            }
            _ => {}
        }
    }

    fn find_rendered_text_rects(
        shapes: &[egui::epaint::ClippedShape],
        target: &str,
    ) -> Vec<egui::Rect> {
        let mut rects = Vec::new();
        for clipped in shapes {
            collect_shape_text_rects(&clipped.shape, target, &mut rects);
        }
        rects
    }

    fn click_point_in_view<F: FnMut(&egui::Context)>(
        ctx: &egui::Context,
        pos: egui::Pos2,
        mut render_fn: F,
    ) {
        let viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::new(1400.0, 900.0));
        let press_input = egui::RawInput {
            screen_rect: Some(viewport),
            events: vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            ..Default::default()
        };
        let _ = ctx.run(press_input, |ctx| render_fn(ctx));

        let release_input = egui::RawInput {
            screen_rect: Some(viewport),
            events: vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            ..Default::default()
        };
        let _ = ctx.run(release_input, |ctx| render_fn(ctx));
    }

    fn run_wide_frame<F: FnMut(&egui::Context)>(
        ctx: &egui::Context,
        mut render_fn: F,
    ) -> egui::FullOutput {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::Vec2::new(1400.0, 900.0),
            )),
            ..Default::default()
        };
        ctx.run(input, |ctx| render_fn(ctx))
    }

    #[test]
    fn test_about_modal_opens_renders_once_and_closes_from_library_view() {
        let mut app = RustNotesApp::default();
        let ctx = egui::Context::default();

        // 1. Awalnya About Modal tertutup di Library View dan belum dirender
        let out_closed = run_wide_frame(&ctx, |ctx| app.render_library_view(ctx));
        assert!(!app.show_about_modal);
        assert!(find_rendered_text_rects(&out_closed.shapes, "ℹ️ Tentang RustNotes").is_empty());
        assert!(find_rendered_text_rects(&out_closed.shapes, "Format Berkas:").is_empty());

        // 2. Klik tombol About pada Library View harus mengaktifkan show_about_modal
        let about_btn_rects = find_rendered_text_rects(&out_closed.shapes, "ℹ Tentang");
        assert_eq!(about_btn_rects.len(), 1);
        click_point_in_view(&ctx, about_btn_rects[0].center(), |ctx| {
            app.render_library_view(ctx)
        });
        assert!(app.show_about_modal);

        // 3. About Modal dirender tepat satu kali (tidak duplikat) saat aktif di Library View
        let out_open = run_wide_frame(&ctx, |ctx| app.render_library_view(ctx));
        assert_eq!(
            find_rendered_text_rects(&out_open.shapes, "ℹ️ Tentang RustNotes").len(),
            1
        );
        assert_eq!(
            find_rendered_text_rects(&out_open.shapes, "Format Berkas:").len(),
            1
        );

        // 4. Klik Tutup menutup modal dan menghentikan renderingnya
        let close_rects = find_rendered_text_rects(&out_open.shapes, "Tutup");
        assert_eq!(close_rects.len(), 1);
        click_point_in_view(&ctx, close_rects[0].center(), |ctx| {
            app.render_library_view(ctx)
        });
        assert!(!app.show_about_modal);

        let out_after_close = run_wide_frame(&ctx, |ctx| app.render_library_view(ctx));
        assert!(find_rendered_text_rects(&out_after_close.shapes, "Format Berkas:").is_empty());

        // 5. Buka kembali About Modal tetap bekerja tanpa duplikasi
        app.show_about_modal = true;
        let out_reopened = run_wide_frame(&ctx, |ctx| app.render_library_view(ctx));
        assert_eq!(
            find_rendered_text_rects(&out_reopened.shapes, "ℹ️ Tentang RustNotes").len(),
            1
        );
        assert_eq!(
            find_rendered_text_rects(&out_reopened.shapes, "Format Berkas:").len(),
            1
        );
    }

    #[test]
    fn test_library_view_shortcut_modal_stays_independent_from_about_modal() {
        let mut app = RustNotesApp::default();
        let ctx = egui::Context::default();

        let out_initial = run_wide_frame(&ctx, |ctx| app.render_library_view(ctx));
        let shortcut_btn_rects =
            find_rendered_text_rects(&out_initial.shapes, "⌨ Pintasan Keyboard");
        assert_eq!(shortcut_btn_rects.len(), 1);

        click_point_in_view(&ctx, shortcut_btn_rects[0].center(), |ctx| {
            app.render_library_view(ctx)
        });
        assert!(app.show_shortcut_modal);
        assert!(!app.show_about_modal);

        let out_shortcut = run_wide_frame(&ctx, |ctx| app.render_library_view(ctx));
        assert_eq!(
            find_rendered_text_rects(
                &out_shortcut.shapes,
                "⌨ Pengaturan & Panduan Pintasan Keyboard",
            )
            .len(),
            1
        );
        assert!(find_rendered_text_rects(&out_shortcut.shapes, "Format Berkas:").is_empty());

        // Membuka lalu menutup About Modal tidak boleh mengubah state Shortcut Modal
        app.show_about_modal = true;
        // Frame pertama digunakan egui untuk mengukur ukuran window anchor CENTER_CENTER; frame kedua merender penuh
        let _ = run_wide_frame(&ctx, |ctx| app.render_library_view(ctx));
        let out_both = run_wide_frame(&ctx, |ctx| app.render_library_view(ctx));
        assert_eq!(
            find_rendered_text_rects(&out_both.shapes, "Format Berkas:").len(),
            1
        );
        assert_eq!(
            find_rendered_text_rects(&out_both.shapes, "⌨ Pengaturan & Panduan Pintasan Keyboard",)
                .len(),
            1
        );

        let close_rects = find_rendered_text_rects(&out_both.shapes, "Tutup");
        assert_eq!(close_rects.len(), 1);
        click_point_in_view(&ctx, close_rects[0].center(), |ctx| {
            app.render_library_view(ctx)
        });
        assert!(!app.show_about_modal);
        assert!(app.show_shortcut_modal);
    }

    #[test]
    fn test_about_modal_renders_once_in_editor_view_without_regression() {
        let mut app = RustNotesApp::default();
        let nb = Notebook::new("Editor About NB", PageTemplate::Blank, Color32::WHITE);
        let nb_id = nb.id.clone();
        app.library.notebooks.push(nb);
        app.active_notebook_id = Some(nb_id);
        let ctx = egui::Context::default();

        let _ = run_wide_frame(&ctx, |ctx| app.render_editor_view(ctx));
        let out_closed = run_wide_frame(&ctx, |ctx| app.render_editor_view(ctx));
        assert!(find_rendered_text_rects(&out_closed.shapes, "Format Berkas:").is_empty());

        app.show_about_modal = true;
        let _ = run_wide_frame(&ctx, |ctx| app.render_editor_view(ctx));
        let out_open = run_wide_frame(&ctx, |ctx| app.render_editor_view(ctx));
        assert_eq!(
            find_rendered_text_rects(&out_open.shapes, "ℹ️ Tentang RustNotes").len(),
            1
        );
        assert_eq!(
            find_rendered_text_rects(&out_open.shapes, "Format Berkas:").len(),
            1
        );
    }
}

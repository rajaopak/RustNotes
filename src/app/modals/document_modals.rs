use super::super::state::RustNotesApp;
use crate::model::OutlineItem;
use eframe::egui::{self, Align, Color32, Layout, RichText};

impl RustNotesApp {
    pub(super) fn render_in_doc_search_modal(&mut self, ctx: &egui::Context, nb_idx: usize) {
        // Fase 5: In-Document Search (Ctrl+F)
        if self.show_in_doc_search {
            egui::Window::new("🔍 Cari di Dokumen")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::RIGHT_TOP, [-24.0, 96.0])
                .show(ctx, |ui| {
                    ui.set_width(280.0);
                    ui.horizontal(|ui| {
                        let query_resp = ui.text_edit_singleline(&mut self.doc_search_query);
                        if query_resp.lost_focus()
                            && ctx.input(|i| i.key_pressed(egui::Key::Escape))
                        {
                            self.show_in_doc_search = false;
                        }
                        if ui.button("✕").clicked() {
                            self.show_in_doc_search = false;
                        }
                    });

                    let q = self.doc_search_query.trim().to_lowercase();
                    if !q.is_empty() {
                        // Find all matches across all pages in active notebook (including Fase 8.6 OCR text)
                        let mut matches: Vec<(usize, String)> = Vec::new();
                        for (p_i, page) in self.library.notebooks[nb_idx].pages.iter().enumerate() {
                            if !page.ocr_text.is_empty()
                                && page.ocr_text.to_lowercase().contains(&q)
                            {
                                matches.push((p_i, format!("✍ OCR: {}", page.ocr_text)));
                            }
                            for obj in &page.objects {
                                match obj {
                                    crate::model::DrawObject::Text { text, .. }
                                    | crate::model::DrawObject::StickyNote { text, .. }
                                        if text.to_lowercase().contains(&q) =>
                                    {
                                        matches.push((p_i, text.clone()));
                                    }
                                    crate::model::DrawObject::Shape {
                                        label: Some(lbl), ..
                                    } if lbl.text.to_lowercase().contains(&q) => {
                                        matches.push((p_i, format!("📐 {}", lbl.text)));
                                    }
                                    _ => {}
                                }
                            }
                        }

                        if matches.is_empty() {
                            ui.label(
                                RichText::new("Tidak ditemukan kecocokan.")
                                    .size(11.0)
                                    .color(Color32::GRAY),
                            );
                        } else {
                            if self.doc_search_current_match >= matches.len() {
                                self.doc_search_current_match = 0;
                            }
                            let cur_match = self.doc_search_current_match;

                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(format!(
                                        "{}/{} hasil",
                                        cur_match + 1,
                                        matches.len()
                                    ))
                                    .size(11.0)
                                    .color(Color32::LIGHT_GRAY),
                                );
                                if ui.small_button("◀").clicked() {
                                    if cur_match == 0 {
                                        self.doc_search_current_match = matches.len() - 1;
                                    } else {
                                        self.doc_search_current_match -= 1;
                                    }
                                    let target_p = matches[self.doc_search_current_match].0;
                                    self.navigate_to_page(nb_idx, target_p);
                                }
                                if ui.small_button("▶").clicked() {
                                    self.doc_search_current_match = (cur_match + 1) % matches.len();
                                    let target_p = matches[self.doc_search_current_match].0;
                                    self.navigate_to_page(nb_idx, target_p);
                                }
                            });

                            let target_page = matches[cur_match].0;
                            let snippet = &matches[cur_match].1;
                            let snippet_preview: String = snippet.chars().take(28).collect();
                            ui.label(
                                RichText::new(format!(
                                    "Hal. {}: \"{}…\"",
                                    target_page + 1,
                                    snippet_preview
                                ))
                                .size(10.0)
                                .color(Color32::from_rgb(76, 147, 236)),
                            );
                        }
                    } else {
                        ui.label(
                            RichText::new("Ketik kata untuk mencari teks...")
                                .size(10.0)
                                .color(Color32::GRAY),
                        );
                    }
                });
        }
    }

    pub(super) fn render_add_outline_modal(
        &mut self,
        ctx: &egui::Context,
        nb_idx: usize,
        notebook_id: &str,
    ) {
        // Fase 5: Add Outline Item Modal Dialog
        if self.show_add_outline_modal {
            egui::Window::new("📑 Tambah ke Daftar Isi (Outline)")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.set_width(320.0);
                    ui.label(RichText::new("Tambahkan Bab / Judul pada Halaman Aktif").strong());
                    ui.label(
                        RichText::new(format!("Halaman saat ini: {}", self.current_page_index + 1))
                            .size(11.0)
                            .color(Color32::GRAY),
                    );
                    ui.add_space(8.0);
                    ui.label("Judul / Topik:");
                    ui.text_edit_singleline(&mut self.new_outline_title);
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.button("Simpan").clicked()
                                && !self.new_outline_title.trim().is_empty()
                            {
                                let item = OutlineItem {
                                    id: uuid::Uuid::new_v4().to_string(),
                                    title: self.new_outline_title.trim().to_string(),
                                    page_index: self.current_page_index,
                                };
                                self.library.notebooks[nb_idx].outline.push(item);
                                self.mark_dirty(notebook_id);
                                self.show_add_outline_modal = false;
                                self.new_outline_title.clear();
                            }
                            if ui.button("Batal").clicked() {
                                self.show_add_outline_modal = false;
                            }
                        });
                    });
                });
        }
    }

    pub(super) fn render_ocr_modal(
        &mut self,
        ctx: &egui::Context,
        nb_idx: usize,
        notebook_id: &str,
    ) {
        let lang = self.settings.language;
        // Fase 8.6: Handwriting OCR Inspector & Search Index Modal
        if self.show_ocr_modal {
            egui::Window::new(lang.tr(
                "🔤 Ekstraksi OCR Tulisan Tangan",
                "🔤 Handwriting OCR Extraction",
            ))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.set_width(380.0);
                ui.label(
                    RichText::new(lang.tr(
                        "Teks Tulisan Tangan Terindeks (Halaman Aktif)",
                        "Indexed Handwriting Text (Current Page)",
                    ))
                    .strong(),
                );
                ui.label(
                    RichText::new(lang.tr(
                        "Teks ini diekstrak secara otomatis dari goresan pena dan diindeks untuk pencarian global & Ctrl+F. Anda juga dapat menyuntingnya secara langsung.",
                        "This text is automatically extracted from pen strokes and indexed for global & Ctrl+F search. You can also edit it directly.",
                    ))
                    .size(11.0)
                    .color(Color32::GRAY),
                );
                ui.add_space(8.0);

                let cur_idx = self.current_page_index;
                let mut ocr_changed = false;
                {
                    let p = &mut self.library.notebooks[nb_idx].pages[cur_idx];
                    let resp = ui.add(
                        egui::TextEdit::multiline(&mut p.ocr_text)
                            .desired_rows(4)
                            .desired_width(f32::INFINITY)
                            .hint_text(lang.tr(
                                "Belum ada goresan tulisan tangan yang dikenali...",
                                "No handwriting strokes recognized yet...",
                            )),
                    );
                    if resp.changed() {
                        ocr_changed = true;
                    }
                }
                if ocr_changed {
                    self.mark_dirty(notebook_id);
                }

                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui
                        .button(lang.tr("🔄 Pindai Ulang Goresan", "🔄 Rescan Strokes"))
                        .clicked()
                    {
                        let p = &mut self.library.notebooks[nb_idx].pages[cur_idx];
                        p.ocr_text = crate::geometry::recognize_handwriting_from_strokes(&p.objects);
                        self.mark_dirty(notebook_id);
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button(lang.tr("Tutup", "Close")).clicked() {
                            self.show_ocr_modal = false;
                        }
                    });
                });
            });
        }
    }
}

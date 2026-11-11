use super::state::RustNotesApp;
use super::types::{AddPagePosition, ShortcutModalTab};
use crate::model::{
    CustomTemplate, OutlineItem, Page, PageOrientation, PageTemplate, PaperSizePreset,
    ShortcutAction,
};
use crate::storage::{
    app_storage_dir, save_asset_to_notebook, save_custom_template, save_settings_to_dir,
};
use eframe::egui::{
    self, Align, Button, Color32, Layout, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Vec2,
};

impl RustNotesApp {
    pub(crate) fn render_add_page_modal(&mut self, ctx: &egui::Context, nb_idx: usize) {
        if !self.show_add_page_modal {
            return;
        }
        let (cur_tmpl, cur_w, cur_h) = {
            if let Some(p) = self.library.notebooks[nb_idx]
                .pages
                .get(self.current_page_index)
            {
                (p.template, p.width, p.height)
            } else {
                (PageTemplate::Ruled, 595.0, 842.0)
            }
        };

        let mut close_modal = false;

        egui::Window::new("➕ Atur & Tambah Halaman Baru")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.set_width(450.0);
                ui.add_space(4.0);

                // Info Template & Format Saat Ini
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("📄 Halaman Aktif:").strong());
                        ui.label(format!(
                            "Hal {} dari {}",
                            self.current_page_index + 1,
                            self.library.notebooks[nb_idx].pages.len()
                        ));

                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui
                                .button("📋 Samakan Format Halaman Aktif")
                                .on_hover_text(
                                    "Salin template pola, ukuran kertas, orientasi, dan warna kertas dari halaman saat ini",
                                )
                                .clicked()
                            {
                                let current_page = self.library.notebooks[nb_idx]
                                    .pages
                                    .get(self.current_page_index);
                                if let Some(p) = current_page {
                                    self.add_page_template = p.template;
                                    self.add_page_paper_color = p.paper_color.to_color32();
                                    self.add_page_custom_width = p.width;
                                    self.add_page_custom_height = p.height;
                                    if let Some((preset, orient)) =
                                        PaperSizePreset::match_dimensions(p.width, p.height)
                                    {
                                        self.add_page_preset = preset;
                                        self.add_page_orientation = orient;
                                        self.add_page_use_custom_size = false;
                                    } else {
                                        self.add_page_use_custom_size = true;
                                        self.add_page_orientation = if p.width > p.height {
                                            PageOrientation::Landscape
                                        } else {
                                            PageOrientation::Portrait
                                        };
                                    }
                                }
                            }
                        });
                    });
                    ui.label(
                        RichText::new(format!(
                            "Template: {}  |  Ukuran: {:.0} × {:.0} pt",
                            cur_tmpl.name(),
                            cur_w,
                            cur_h
                        ))
                        .size(11.5)
                        .color(Color32::from_rgb(130, 195, 255)),
                    );
                });

                ui.add_space(6.0);
                ui.separator();
                ui.add_space(4.0);

                // Opsi Penempatan Halaman
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Posisi:").strong());
                    ui.radio_value(
                        &mut self.add_page_target_pos,
                        AddPagePosition::AfterCurrent,
                        "Setelah Hal Ini",
                    );
                    ui.radio_value(
                        &mut self.add_page_target_pos,
                        AddPagePosition::BeforeCurrent,
                        "Sebelum Hal Ini",
                    );
                    ui.radio_value(
                        &mut self.add_page_target_pos,
                        AddPagePosition::AtEnd,
                        "Di Akhir Dokumen",
                    );
                });

                ui.add_space(6.0);
                ui.separator();
                ui.add_space(4.0);

                // Opsi Jenis Halaman
                ui.label(RichText::new("Pilihan Halaman Baru:").strong());

                // 1. Template Standar & Kustom
                ui.group(|ui| {
                    ui.label(RichText::new("1. Template Pola Kertas:").strong());
                    ui.horizontal_wrapped(|ui| {
                        for tmpl in PageTemplate::ALL {
                            let is_sel = self.add_page_template == tmpl;
                            if ui
                                .selectable_label(is_sel, tmpl.name())
                                .on_hover_text(tmpl.description())
                                .clicked()
                            {
                                self.add_page_template = tmpl;
                            }
                        }
                    });

                    if !self.custom_templates.is_empty() {
                        ui.add_space(4.0);
                        ui.horizontal_wrapped(|ui| {
                            ui.label(
                                RichText::new("⭐ Kustom:")
                                    .size(11.5)
                                    .color(Color32::from_rgb(255, 215, 0)),
                            );
                            for ctmpl in &self.custom_templates {
                                let is_sel = self.add_page_template == ctmpl.template;
                                if ui
                                    .selectable_label(is_sel, format!("⭐ {}", ctmpl.name))
                                    .clicked()
                                {
                                    self.add_page_template = ctmpl.template;
                                    self.add_page_custom_width = ctmpl.width;
                                    self.add_page_custom_height = ctmpl.height;
                                    self.add_page_use_custom_size = true;
                                    self.add_page_paper_color = ctmpl.paper_color.to_color32();
                                }
                            }
                        });
                    }

                    ui.add_space(6.0);
                    ui.label(RichText::new("Ukuran & Proporsi Kertas:").strong());
                    ui.horizontal_wrapped(|ui| {
                        for preset in PaperSizePreset::ALL {
                            let is_sel =
                                !self.add_page_use_custom_size && self.add_page_preset == preset;
                            if ui
                                .selectable_label(is_sel, preset.short_name())
                                .on_hover_text(preset.name())
                                .clicked()
                            {
                                self.add_page_use_custom_size = false;
                                self.add_page_preset = preset;
                                let (pw, ph) =
                                    preset.dimensions(self.add_page_orientation);
                                self.add_page_custom_width = pw;
                                self.add_page_custom_height = ph;
                            }
                        }
                        if ui
                            .selectable_label(self.add_page_use_custom_size, "Custom")
                            .on_hover_text("Kustomisasi lebar dan tinggi secara bebas")
                            .clicked()
                        {
                            self.add_page_use_custom_size = true;
                        }
                    });

                    ui.horizontal(|ui| {
                        ui.label("Orientasi:");
                        if ui
                            .selectable_label(
                                !self.add_page_use_custom_size
                                    && self.add_page_orientation == PageOrientation::Portrait,
                                "Portrait",
                            )
                            .clicked()
                        {
                            self.add_page_use_custom_size = false;
                            self.add_page_orientation = PageOrientation::Portrait;
                            let (pw, ph) =
                                self.add_page_preset.dimensions(self.add_page_orientation);
                            self.add_page_custom_width = pw;
                            self.add_page_custom_height = ph;
                        }
                        if ui
                            .selectable_label(
                                !self.add_page_use_custom_size
                                    && self.add_page_orientation == PageOrientation::Landscape,
                                "Landscape",
                            )
                            .clicked()
                        {
                            self.add_page_use_custom_size = false;
                            self.add_page_orientation = PageOrientation::Landscape;
                            let (pw, ph) =
                                self.add_page_preset.dimensions(self.add_page_orientation);
                            self.add_page_custom_width = pw;
                            self.add_page_custom_height = ph;
                        }
                        if ui
                            .small_button("⇄ Swap W×H")
                            .on_hover_text("Tukar lebar dan tinggi kertas")
                            .clicked()
                        {
                            self.add_page_use_custom_size = true;
                            std::mem::swap(
                                &mut self.add_page_custom_width,
                                &mut self.add_page_custom_height,
                            );
                            self.add_page_orientation =
                                if self.add_page_custom_width > self.add_page_custom_height {
                                    PageOrientation::Landscape
                                } else {
                                    PageOrientation::Portrait
                                };
                        }
                    });

                    ui.horizontal(|ui| {
                        ui.label("W:");
                        if ui
                            .add(
                                egui::DragValue::new(&mut self.add_page_custom_width)
                                    .range(200.0..=4000.0)
                                    .speed(2.0)
                                    .suffix(" pt"),
                            )
                            .changed()
                        {
                            self.add_page_use_custom_size = true;
                        }
                        ui.label("× H:");
                        if ui
                            .add(
                                egui::DragValue::new(&mut self.add_page_custom_height)
                                    .range(200.0..=4000.0)
                                    .speed(2.0)
                                    .suffix(" pt"),
                            )
                            .changed()
                        {
                            self.add_page_use_custom_size = true;
                        }

                        let (calc_w, calc_h) = if self.add_page_use_custom_size {
                            (self.add_page_custom_width, self.add_page_custom_height)
                        } else {
                            self.add_page_preset.dimensions(self.add_page_orientation)
                        };
                        ui.label(
                            RichText::new(format!("({:.0} × {:.0} pt)", calc_w, calc_h))
                                .size(11.0)
                                .color(Color32::GRAY),
                        );
                    });

                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.label("Warna Kertas:");
                        let paper_colors = [
                            (Color32::from_rgb(255, 255, 255), "Putih Bersih"),
                            (Color32::from_rgb(255, 254, 250), "Cream Ivory"),
                            (Color32::from_rgb(255, 249, 219), "Pastel Yellow"),
                            (Color32::from_rgb(235, 251, 238), "Mint Green"),
                            (Color32::from_rgb(233, 236, 239), "Soft Gray"),
                            (Color32::from_rgb(26, 35, 50), "Dark Navy"),
                            (Color32::from_rgb(24, 24, 27), "Charcoal Black"),
                        ];
                        for (col, tip) in paper_colors {
                            let is_sel = self.add_page_paper_color == col;
                            let (rect, resp) =
                                ui.allocate_exact_size(Vec2::new(18.0, 18.0), Sense::click());
                            let resp = resp.on_hover_text(tip);
                            if resp.clicked() {
                                self.add_page_paper_color = col;
                            }
                            let border = if is_sel {
                                Stroke::new(2.0_f32, Color32::from_rgb(0, 114, 206))
                            } else {
                                Stroke::new(1.0_f32, Color32::from_rgb(100, 100, 100))
                            };
                            ui.painter().rect(rect, 3.0, col, border, StrokeKind::Outside);
                            if is_sel {
                                let check_col = if col.r() < 100 {
                                    Color32::WHITE
                                } else {
                                    Color32::BLACK
                                };
                                ui.painter().circle_filled(rect.center(), 2.5_f32, check_col);
                            }
                        }
                    });
                });

                ui.add_space(6.0);

                // 2. Impor PDF sebagai Halaman
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("2. Sisipkan Berkas PDF:").strong());
                        if ui.button("📥 Pilih PDF...").clicked()
                            && let Some(path) = rfd::FileDialog::new()
                                .add_filter("PDF Document", &["pdf"])
                                .pick_file()
                        {
                            self.import_pdf_pages_to_active(nb_idx, &path);
                            close_modal = true;
                        }
                    });
                });

                ui.add_space(6.0);

                // 3. Gunakan Gambar sebagai Halaman
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("3. Gunakan Gambar Latar:").strong());
                        if ui.button("🖼 Pilih Gambar...").clicked()
                            && let Some(path) = rfd::FileDialog::new()
                                .add_filter("Image", &["png", "jpg", "jpeg", "webp"])
                                .pick_file()
                            && let Ok(img_bytes) = std::fs::read(&path)
                        {
                            let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("png");
                            let storage_dir = app_storage_dir();
                            let nb_id = self.library.notebooks[nb_idx].id.clone();
                            if let Ok(asset_id) =
                                save_asset_to_notebook(&storage_dir, &nb_id, &img_bytes, ext)
                                && let Ok(dyn_img) = image::load_from_memory(&img_bytes)
                            {
                                let page_w = dyn_img.width() as f32;
                                let page_h = dyn_img.height() as f32;
                                let mut new_page =
                                    Page::with_size(PageTemplate::Blank, page_w, page_h);
                                new_page.background =
                                    Some(crate::model::PageBackground::Image { asset_id });

                                self.push_doc_undo(nb_idx);
                                let insert_pos = match self.add_page_target_pos {
                                    AddPagePosition::BeforeCurrent => self.current_page_index,
                                    AddPagePosition::AfterCurrent => self.current_page_index + 1,
                                    AddPagePosition::AtEnd => {
                                        self.library.notebooks[nb_idx].pages.len()
                                    }
                                };
                                let clamped_pos =
                                    insert_pos.min(self.library.notebooks[nb_idx].pages.len());
                                self.library.notebooks[nb_idx]
                                    .pages
                                    .insert(clamped_pos, new_page);
                                self.navigate_to_page(nb_idx, clamped_pos);
                                self.mark_dirty(&nb_id);
                                self.toast_message = Some(
                                    "✅ Halaman berlatar gambar berhasil ditambahkan.".to_string(),
                                );
                                close_modal = true;
                            }
                        }
                    });
                });

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(6.0);

                // Tombol Batal & Tambah
                ui.horizontal(|ui| {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let add_btn = Button::new(
                            RichText::new("➕ Tambah Halaman")
                                .strong()
                                .color(Color32::WHITE),
                        )
                        .fill(Color32::from_rgb(0, 114, 206));

                        if ui.add(add_btn).clicked() {
                            let (w, h) = if self.add_page_use_custom_size {
                                (
                                    self.add_page_custom_width.clamp(200.0, 4000.0),
                                    self.add_page_custom_height.clamp(200.0, 4000.0),
                                )
                            } else {
                                self.add_page_preset.dimensions(self.add_page_orientation)
                            };

                            self.push_doc_undo(nb_idx);
                            let target_idx = match self.add_page_target_pos {
                                AddPagePosition::BeforeCurrent => self.current_page_index,
                                AddPagePosition::AfterCurrent => self.current_page_index + 1,
                                AddPagePosition::AtEnd => {
                                    self.library.notebooks[nb_idx].pages.len()
                                }
                            };
                            let new_idx = self.library.notebooks[nb_idx].insert_page_with_color(
                                target_idx,
                                self.add_page_template,
                                w,
                                h,
                                self.add_page_paper_color,
                            );
                            self.navigate_to_page(nb_idx, new_idx);
                            let nb_id = self.library.notebooks[nb_idx].id.clone();
                            self.mark_dirty(&nb_id);

                            // Simpan ke memori template terakhir
                            self.last_created_template = self.add_page_template;
                            self.last_created_preset = self.add_page_preset;
                            self.last_created_orientation = self.add_page_orientation;
                            self.last_created_use_custom = self.add_page_use_custom_size;
                            self.last_created_width = w;
                            self.last_created_height = h;
                            self.last_created_paper_color = self.add_page_paper_color;

                            close_modal = true;
                        }

                        if ui.button("Batal").clicked() {
                            close_modal = true;
                        }
                    });
                });
            });

        if close_modal {
            self.show_add_page_modal = false;
        }
    }

    pub fn render_editor_modals(&mut self, ctx: &egui::Context, nb_idx: usize) {
        let notebook_id = self.library.notebooks[nb_idx].id.clone();
        let lang = self.settings.language;
        if self.show_page_resize_modal {
            let page_count = self.library.notebooks[nb_idx].pages.len();
            if self.resize_target_page_index >= page_count {
                self.resize_target_page_index = self.current_page_index.min(page_count - 1);
            }
            egui::Window::new("📐 Pengaturan Ukuran Kertas & Template Halaman")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.set_width(430.0);
                    ui.label(
                        RichText::new(
                            "Atur ukuran kertas spesifik (preset maupun angka bebas) serta template per halaman atau untuk seluruh dokumen.",
                        )
                        .size(11.5)
                        .color(Color32::GRAY),
                    );
                    ui.add_space(8.0);

                    // 1. Scope: Per-Page vs All Pages
                    ui.label(RichText::new("Cakupan Penerapan (Scope):").strong());
                    ui.horizontal(|ui| {
                        if ui
                            .selectable_label(
                                !self.resize_apply_all_pages,
                                format!(
                                    "📄 Halaman Spesifik (Hal. {} dari {})",
                                    self.resize_target_page_index + 1,
                                    page_count
                                ),
                            )
                            .clicked()
                        {
                            self.resize_apply_all_pages = false;
                        }
                        if ui
                            .selectable_label(
                                self.resize_apply_all_pages,
                                format!("📚 Semua Halaman ({page_count} Hal.)"),
                            )
                            .clicked()
                        {
                            self.resize_apply_all_pages = true;
                        }
                    });

                    // Jika mode per-halaman dan ada > 1 halaman, izinkan pilih halaman mana yang ingin diatur
                    if !self.resize_apply_all_pages && page_count > 1 {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("Pilih Halaman Target:")
                                    .size(11.5)
                                    .color(Color32::LIGHT_GRAY),
                            );
                            if self.resize_target_page_index > 0
                                && ui.small_button("◀").clicked()
                            {
                                let next_idx = self.resize_target_page_index - 1;
                                self.open_page_resize_modal(nb_idx, next_idx);
                            }
                            ui.label(
                                RichText::new(format!(
                                    "Halaman {}",
                                    self.resize_target_page_index + 1
                                ))
                                .strong(),
                            );
                            if self.resize_target_page_index + 1 < page_count
                                && ui.small_button("▶").clicked()
                            {
                                let next_idx = self.resize_target_page_index + 1;
                                self.open_page_resize_modal(nb_idx, next_idx);
                            }
                        });
                    }

                    ui.separator();

                    // 2. Preset & Custom Paper Size
                    ui.label(RichText::new("Pilih Preset atau Ukuran Spesifik:").strong());
                    ui.horizontal_wrapped(|ui| {
                        for preset in PaperSizePreset::ALL {
                            let is_sel = !self.resize_use_custom_size
                                && self.resize_target_preset == preset;
                            if ui.selectable_label(is_sel, preset.short_name()).on_hover_text(preset.name()).clicked() {
                                self.resize_use_custom_size = false;
                                self.resize_target_preset = preset;
                                let (pw, ph) =
                                    preset.dimensions(self.resize_target_orientation);
                                self.resize_custom_width = pw;
                                self.resize_custom_height = ph;
                            }
                        }
                        if ui
                            .selectable_label(self.resize_use_custom_size, "✏ Custom")
                            .on_hover_text("Atur lebar dan tinggi secara bebas")
                            .clicked()
                        {
                            self.resize_use_custom_size = true;
                        }
                    });

                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.label("Orientasi:");
                        if ui
                            .selectable_label(
                                !self.resize_use_custom_size
                                    && self.resize_target_orientation == PageOrientation::Portrait,
                                "Portrait",
                            )
                            .clicked()
                        {
                            self.resize_use_custom_size = false;
                            self.resize_target_orientation = PageOrientation::Portrait;
                            let (pw, ph) = self
                                .resize_target_preset
                                .dimensions(self.resize_target_orientation);
                            self.resize_custom_width = pw;
                            self.resize_custom_height = ph;
                        }
                        if ui
                            .selectable_label(
                                !self.resize_use_custom_size
                                    && self.resize_target_orientation == PageOrientation::Landscape,
                                "Landscape",
                            )
                            .clicked()
                        {
                            self.resize_use_custom_size = false;
                            self.resize_target_orientation = PageOrientation::Landscape;
                            let (pw, ph) = self
                                .resize_target_preset
                                .dimensions(self.resize_target_orientation);
                            self.resize_custom_width = pw;
                            self.resize_custom_height = ph;
                        }
                        if ui
                            .small_button("⇄ Tukar Lebar × Tinggi")
                            .clicked()
                        {
                            self.resize_use_custom_size = true;
                            std::mem::swap(
                                &mut self.resize_custom_width,
                                &mut self.resize_custom_height,
                            );
                        }
                    });

                    ui.add_space(6.0);
                    ui.label(RichText::new("Dimensi Kertas Spesifik (Width × Height):").strong());
                    ui.horizontal(|ui| {
                        ui.label("Lebar (Width):");
                        if ui
                            .add(
                                egui::DragValue::new(&mut self.resize_custom_width)
                                    .range(200.0..=4000.0)
                                    .speed(2.0)
                                    .suffix(" pt"),
                            )
                            .changed()
                        {
                            self.resize_use_custom_size = true;
                        }
                        ui.add_space(8.0);
                        ui.label("Tinggi (Height):");
                        if ui
                            .add(
                                egui::DragValue::new(&mut self.resize_custom_height)
                                    .range(200.0..=4000.0)
                                    .speed(2.0)
                                    .suffix(" pt"),
                            )
                            .changed()
                        {
                            self.resize_use_custom_size = true;
                        }
                    });

                    ui.separator();

                    // 3. Page Template Selector
                    ui.label(RichText::new("Template Pola Kertas:").strong());
                    ui.horizontal_wrapped(|ui| {
                        for tmpl in PageTemplate::ALL {
                            if ui
                                .selectable_label(self.resize_target_template == tmpl, tmpl.name())
                                .on_hover_text(tmpl.description())
                                .clicked()
                            {
                                self.resize_target_template = tmpl;
                            }
                        }
                    });

                    let (w, h) = if self.resize_use_custom_size {
                        (
                            self.resize_custom_width.clamp(200.0, 4000.0),
                            self.resize_custom_height.clamp(200.0, 4000.0),
                        )
                    } else {
                        self.resize_target_preset
                            .dimensions(self.resize_target_orientation)
                    };
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new(format!(
                            "Hasil Akhir: {:.0} × {:.0} pt ({:?})",
                            w, h, self.resize_target_template
                        ))
                        .size(12.0)
                        .color(Color32::from_rgb(130, 195, 255)),
                    );

                    ui.add_space(10.0);
                    ui.separator();
                    ui.add_space(6.0);

                    ui.horizontal(|ui| {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let apply_btn = Button::new(
                                RichText::new("Terapkan Perubahan")
                                    .strong()
                                    .color(Color32::WHITE),
                            )
                            .fill(Color32::from_rgb(0, 114, 206));
                            if ui.add(apply_btn).clicked() {
                                self.push_doc_undo(nb_idx);
                                if self.resize_apply_all_pages {
                                    let total = self.library.notebooks[nb_idx].pages.len();
                                    for idx in 0..total {
                                        self.library.notebooks[nb_idx].resize_page(idx, w, h);
                                        let p = &mut self.library.notebooks[nb_idx].pages[idx];
                                        p.template = self.resize_target_template;
                                        if matches!(
                                            p.background,
                                            Some(crate::model::PageBackground::Template(_))
                                        ) {
                                            p.background =
                                                Some(crate::model::PageBackground::Template(
                                                    self.resize_target_template,
                                                ));
                                        }
                                    }
                                } else {
                                    let target_idx = self.resize_target_page_index;
                                    self.library.notebooks[nb_idx].resize_page(target_idx, w, h);
                                    let p = &mut self.library.notebooks[nb_idx].pages[target_idx];
                                    p.template = self.resize_target_template;
                                    if matches!(
                                        p.background,
                                        Some(crate::model::PageBackground::Template(_))
                                    ) {
                                        p.background = Some(crate::model::PageBackground::Template(
                                            self.resize_target_template,
                                        ));
                                    }
                                }
                                self.mark_dirty(&notebook_id);
                                self.show_page_resize_modal = false;
                            }
                            if ui.button("Batal").clicked() {
                                self.show_page_resize_modal = false;
                            }
                        });
                    });
                });
        }

        // Add Page Modal
        self.render_add_page_modal(ctx, nb_idx);

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
                                self.mark_dirty(&notebook_id);
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

        // Fase 5: Save Page as Custom Template Modal Dialog
        if self.show_save_template_modal {
            egui::Window::new("💾 Simpan Halaman sebagai Template Kustom")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.set_width(340.0);
                    ui.label(RichText::new("Simpan Format & Latar Halaman Ini").strong());
                    ui.label(
                        RichText::new("Template yang disimpan dapat digunakan kembali saat membuat buku catatan baru.")
                            .size(11.0)
                            .color(Color32::GRAY),
                    );
                    ui.add_space(8.0);
                    ui.label("Nama Template:");
                    ui.text_edit_singleline(&mut self.new_template_name);
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.button("Simpan Template").clicked() && !self.new_template_name.trim().is_empty() {
                                let cur_p = &self.library.notebooks[nb_idx].pages[self.current_page_index];
                                let tmpl = CustomTemplate {
                                    id: uuid::Uuid::new_v4().to_string(),
                                    name: self.new_template_name.trim().to_string(),
                                    template: cur_p.template,
                                    background: cur_p.background.clone(),
                                    paper_color: cur_p.paper_color.clone(),
                                    width: cur_p.width,
                                    height: cur_p.height,
                                };
                                let storage_dir = app_storage_dir();
                                if let Ok(()) = save_custom_template(&storage_dir, &tmpl) {
                                    self.custom_templates.push(tmpl);
                                    self.toast_message = Some("✅ Template kustom berhasil disimpan!".to_string());
                                }
                                self.show_save_template_modal = false;
                                self.new_template_name.clear();
                            }
                            if ui.button("Batal").clicked() {
                                self.show_save_template_modal = false;
                            }
                        });
                    });
                });
        }

        self.render_shortcut_modal(ctx);

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
                    self.mark_dirty(&notebook_id);
                }

                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui
                        .button(lang.tr("🔄 Pindai Ulang Goresan", "🔄 Rescan Strokes"))
                        .clicked()
                    {
                        let p = &mut self.library.notebooks[nb_idx].pages[cur_idx];
                        p.ocr_text = crate::geometry::recognize_handwriting_from_strokes(&p.objects);
                        self.mark_dirty(&notebook_id);
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button(lang.tr("Tutup", "Close")).clicked() {
                            self.show_ocr_modal = false;
                        }
                    });
                });
            });
        }

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
                        self.canvas_state.pressure_sensitivity_enabled =
                            self.settings.pressure_sensitivity_enabled;
                        self.canvas_state.pressure_min = self.settings.pressure_min;
                        self.canvas_state.pressure_multiplier =
                            self.settings.pressure_multiplier;
                        self.canvas_state.highlighter_opacity =
                            self.settings.highlighter_opacity;
                        self.canvas_state.highlighter_pressure_enabled =
                            self.settings.highlighter_pressure_enabled;
                        self.canvas_state.mesh_cache.clear();
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
                            self.settings.pressure_sensitivity_enabled = true;
                            self.settings.pressure_min = 0.2;
                            self.settings.pressure_multiplier = 1.0;
                            self.canvas_state.pressure_sensitivity_enabled = true;
                            self.canvas_state.pressure_min = 0.2;
                            self.canvas_state.pressure_multiplier = 1.0;
                            let _ = save_settings_to_dir(&app_storage_dir(), &self.settings);
                        }
                    });
                }
            }
        });
    }
}

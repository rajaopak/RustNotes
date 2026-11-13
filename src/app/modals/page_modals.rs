use super::super::state::RustNotesApp;
use super::super::types::AddPagePosition;
use crate::model::{CustomTemplate, Page, PageOrientation, PageTemplate, PaperSizePreset};
use crate::storage::{app_storage_dir, save_asset_to_notebook, save_custom_template};
use eframe::egui::{
    self, Align, Button, Color32, Layout, RichText, Sense, Stroke, StrokeKind, Vec2,
};

impl RustNotesApp {
    pub(crate) fn render_add_page_modal(&mut self, ctx: &egui::Context, nb_idx: usize) {
        if !self.show_add_page_modal {
            return;
        }
        let Some(nb) = self.library.notebooks.get(nb_idx) else {
            self.show_add_page_modal = false;
            return;
        };
        let page_count = nb.pages.len();
        let current_page_display = if page_count == 0 {
            0
        } else {
            self.current_page_index.min(page_count - 1) + 1
        };
        let (cur_tmpl, cur_w, cur_h) = {
            if let Some(p) = nb.pages.get(self.current_page_index) {
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
                        ui.label(format!("Hal {current_page_display} dari {page_count}"));

                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui
                                .button("📋 Samakan Format Halaman Aktif")
                                .on_hover_text(
                                    "Salin template pola, ukuran kertas, orientasi, dan warna kertas dari halaman saat ini",
                                )
                                .clicked()
                            {
                                let current_page = self
                                    .library
                                    .notebooks
                                    .get(nb_idx)
                                    .and_then(|nb| nb.pages.get(self.current_page_index));
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
                            && let Some(nb_id) =
                                self.library.notebooks.get(nb_idx).map(|nb| nb.id.clone())
                            && let Some(path) = rfd::FileDialog::new()
                                .add_filter("Image", &["png", "jpg", "jpeg", "webp"])
                                .pick_file()
                            && let Ok(img_bytes) = std::fs::read(&path)
                        {
                            let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("png");
                            let storage_dir = app_storage_dir();
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

                                if nb_idx < self.library.notebooks.len() {
                                    self.push_doc_undo(nb_idx);
                                    if let Some(clamped_pos) =
                                        self.library.notebooks.get_mut(nb_idx).map(|nb| {
                                            let insert_pos = match self.add_page_target_pos {
                                                AddPagePosition::BeforeCurrent => {
                                                    self.current_page_index
                                                }
                                                AddPagePosition::AfterCurrent => {
                                                    self.current_page_index.saturating_add(1)
                                                }
                                                AddPagePosition::AtEnd => nb.pages.len(),
                                            };
                                            let clamped_pos = insert_pos.min(nb.pages.len());
                                            nb.pages.insert(clamped_pos, new_page);
                                            clamped_pos
                                        })
                                    {
                                        self.navigate_to_page(nb_idx, clamped_pos);
                                        self.mark_dirty(&nb_id);
                                        self.toast_message = Some(
                                            "✅ Halaman berlatar gambar berhasil ditambahkan."
                                                .to_string(),
                                        );
                                    }
                                }
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

                            if nb_idx < self.library.notebooks.len() {
                                self.push_doc_undo(nb_idx);
                                if let Some((new_idx, nb_id)) =
                                    self.library.notebooks.get_mut(nb_idx).map(|nb| {
                                        let target_idx = match self.add_page_target_pos {
                                            AddPagePosition::BeforeCurrent => {
                                                self.current_page_index
                                            }
                                            AddPagePosition::AfterCurrent => {
                                                self.current_page_index.saturating_add(1)
                                            }
                                            AddPagePosition::AtEnd => nb.pages.len(),
                                        };
                                        let new_idx = nb.insert_page_with_color(
                                            target_idx,
                                            self.add_page_template,
                                            w,
                                            h,
                                            self.add_page_paper_color,
                                        );
                                        (new_idx, nb.id.clone())
                                    })
                                {
                                    self.navigate_to_page(nb_idx, new_idx);
                                    self.mark_dirty(&nb_id);
                                }
                            }

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

    pub(super) fn render_page_resize_modal(
        &mut self,
        ctx: &egui::Context,
        nb_idx: usize,
        notebook_id: &str,
    ) {
        if !self.show_page_resize_modal {
            return;
        }

        let Some(page_count) = self.library.notebooks.get(nb_idx).map(|nb| nb.pages.len()) else {
            self.show_page_resize_modal = false;
            return;
        };
        if page_count == 0 {
            self.show_page_resize_modal = false;
            self.toast_message = Some("⚠️ Tidak ada halaman untuk diubah ukurannya.".to_string());
            return;
        }
        let last_page_idx = page_count.saturating_sub(1);
        if self.resize_target_page_index >= page_count {
            self.resize_target_page_index = self.current_page_index.min(last_page_idx);
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
                        if self.resize_target_page_index > 0 && ui.small_button("◀").clicked() {
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
                        if ui
                            .selectable_label(is_sel, preset.short_name())
                            .on_hover_text(preset.name())
                            .clicked()
                        {
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
                    if ui.small_button("⇄ Tukar Lebar × Tinggi").clicked() {
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
                            if nb_idx < self.library.notebooks.len() {
                                self.push_doc_undo(nb_idx);
                                if let Some(nb) = self.library.notebooks.get_mut(nb_idx) {
                                    if self.resize_apply_all_pages {
                                        for idx in 0..nb.pages.len() {
                                            apply_page_resize_and_template(
                                                nb,
                                                idx,
                                                w,
                                                h,
                                                self.resize_target_template,
                                            );
                                        }
                                    } else {
                                        apply_page_resize_and_template(
                                            nb,
                                            self.resize_target_page_index,
                                            w,
                                            h,
                                            self.resize_target_template,
                                        );
                                    }
                                    self.mark_dirty(notebook_id);
                                }
                            }
                            self.show_page_resize_modal = false;
                        }
                        if ui.button("Batal").clicked() {
                            self.show_page_resize_modal = false;
                        }
                    });
                });
            });
    }

    pub(super) fn render_save_template_modal(&mut self, ctx: &egui::Context, nb_idx: usize) {
        // Fase 5: Save Page as Custom Template Modal Dialog
        if !self.show_save_template_modal {
            return;
        }
        let Some(nb) = self.library.notebooks.get(nb_idx) else {
            self.show_save_template_modal = false;
            return;
        };
        if nb.pages.is_empty() {
            self.show_save_template_modal = false;
            self.toast_message =
                Some("⚠️ Tidak ada halaman aktif untuk disimpan sebagai template.".to_string());
            return;
        }
        egui::Window::new("💾 Simpan Halaman sebagai Template Kustom")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.set_width(340.0);
                ui.label(RichText::new("Simpan Format & Latar Halaman Ini").strong());
                ui.label(
                    RichText::new(
                        "Template yang disimpan dapat digunakan kembali saat membuat buku catatan baru.",
                    )
                    .size(11.0)
                    .color(Color32::GRAY),
                );
                ui.add_space(8.0);
                ui.label("Nama Template:");
                ui.text_edit_singleline(&mut self.new_template_name);
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button("Simpan Template").clicked()
                            && !self.new_template_name.trim().is_empty()
                        {
                            if let Some(cur_p) = self.library.notebooks.get(nb_idx).and_then(|nb| {
                                nb.pages
                                    .get(self.current_page_index)
                                    .or_else(|| nb.pages.last())
                            }) {
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
                                    self.toast_message =
                                        Some("✅ Template kustom berhasil disimpan!".to_string());
                                }
                            } else {
                                self.toast_message = Some(
                                    "⚠️ Halaman aktif tidak ditemukan untuk disimpan sebagai template."
                                        .to_string(),
                                );
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
}

pub(super) fn apply_page_resize_and_template(
    nb: &mut crate::model::Notebook,
    idx: usize,
    w: f32,
    h: f32,
    template: PageTemplate,
) {
    if !nb.resize_page(idx, w, h) {
        return;
    }
    if let Some(p) = nb.pages.get_mut(idx) {
        p.template = template;
        if matches!(
            p.background,
            Some(crate::model::PageBackground::Template(_))
        ) {
            p.background = Some(crate::model::PageBackground::Template(template));
        }
    }
}

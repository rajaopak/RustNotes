use super::cover::draw_goodnotes_cover;
use super::library_layout::folder_path_label;
use super::state::RustNotesApp;
use super::theme::UiPalette;
use super::types::COVER_PALETTE;
use crate::model::{Notebook, PageOrientation, PageTemplate, PaperSizePreset};
use eframe::egui::{
    self, Align, Button, Color32, Layout, RichText, ScrollArea, Sense, Stroke, StrokeKind, Vec2,
};

impl RustNotesApp {
    pub(crate) fn render_library_modals(&mut self, ctx: &egui::Context) {
        let palette = UiPalette::from_ctx(ctx, self.settings.theme_mode);
        self.render_delete_confirm_modals(ctx, &palette);
        self.render_new_notebook_modal(ctx, &palette);
        self.render_new_folder_modal(ctx, &palette);
        self.render_rename_notebook_modal(ctx, &palette);
        self.render_rename_folder_modal(ctx, &palette);
        self.render_move_notebooks_modal(ctx, &palette);
    }

    fn render_delete_confirm_modals(&mut self, ctx: &egui::Context, palette: &UiPalette) {
        let lang = self.settings.language;
        if let Some((del_id, del_title)) = self.confirm_delete_notebook.clone() {
            egui::Window::new(lang.tr("Hapus Permanen", "Delete Permanently"))
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.set_width(360.0);
                    ui.label(lang.tr(
                        &format!("Yakin ingin menghapus permanen '{del_title}'? Tindakan ini tidak dapat dibatalkan."),
                        &format!("Permanently delete '{del_title}'? This action cannot be undone."),
                    ));
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui
                                .add(
                                    Button::new(
                                        RichText::new(lang.tr("Hapus Permanen", "Delete Forever"))
                                            .color(Color32::WHITE),
                                    )
                                    .fill(palette.danger_soft_bg),
                                )
                                .clicked()
                            {
                                match self.permanently_delete_notebook(&del_id) {
                                    Ok(()) => {
                                        self.library_selected_ids.remove(&del_id);
                                        self.toast_message = Some(format!(
                                            "{} '{del_title}'.",
                                            lang.tr(
                                                "🗑 Buku catatan dihapus permanen:",
                                                "🗑 Notebook permanently deleted:"
                                            )
                                        ));
                                    }
                                    Err(e) => {
                                        self.toast_message = Some(format!("⚠ {e}"));
                                    }
                                }
                                self.confirm_delete_notebook = None;
                            }
                            if ui.button(lang.tr("Batal", "Cancel")).clicked() {
                                self.confirm_delete_notebook = None;
                            }
                        });
                    });
                });
        } else if !self.confirm_bulk_delete_ids.is_empty() {
            let count = self.confirm_bulk_delete_ids.len();
            egui::Window::new(lang.tr("Hapus Permanen Terpilih", "Delete Selected Permanently"))
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.set_width(360.0);
                    ui.label(match lang {
                        crate::model::AppLanguage::Indonesian => format!(
                            "Yakin ingin menghapus permanen {count} buku catatan terpilih? Tindakan ini tidak dapat dibatalkan."
                        ),
                        crate::model::AppLanguage::English => format!(
                            "Permanently delete {count} selected notebooks? This action cannot be undone."
                        ),
                    });
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui
                                .add(
                                    Button::new(
                                        RichText::new(lang.tr("Hapus Permanen", "Delete Forever"))
                                            .color(Color32::WHITE),
                                    )
                                    .fill(palette.danger_soft_bg),
                                )
                                .clicked()
                            {
                                let ids = std::mem::take(&mut self.confirm_bulk_delete_ids);
                                let mut deleted = 0usize;
                                for id in ids {
                                    if self.permanently_delete_notebook(&id).is_ok() {
                                        self.library_selected_ids.remove(&id);
                                        deleted += 1;
                                    }
                                }
                                if self.library_selected_ids.is_empty() {
                                    self.library_select_mode = false;
                                }
                                self.toast_message = Some(match lang {
                                    crate::model::AppLanguage::Indonesian => {
                                        format!("🗑 {deleted} buku catatan dihapus permanen.")
                                    }
                                    crate::model::AppLanguage::English => {
                                        format!("🗑 {deleted} notebooks permanently deleted.")
                                    }
                                });
                            }
                            if ui.button(lang.tr("Batal", "Cancel")).clicked() {
                                self.confirm_bulk_delete_ids.clear();
                            }
                        });
                    });
                });
        } else if self.confirm_empty_trash {
            egui::Window::new(lang.tr("Kosongkan Sampah", "Empty Trash"))
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.set_width(360.0);
                    ui.label(lang.tr(
                        "Yakin ingin menghapus permanen seluruh buku catatan di Sampah? Tindakan ini tidak dapat dibatalkan.",
                        "Permanently delete all notebooks in Trash? This action cannot be undone.",
                    ));
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui
                                .add(
                                    Button::new(
                                        RichText::new(lang.tr("Kosongkan", "Empty Trash"))
                                            .color(Color32::WHITE),
                                    )
                                    .fill(palette.danger_soft_bg),
                                )
                                .clicked()
                            {
                                match self.empty_trash() {
                                    Ok(n) => {
                                        self.library_selected_ids.clear();
                                        self.library_select_mode = false;
                                        self.toast_message = Some(match lang {
                                            crate::model::AppLanguage::Indonesian => {
                                                format!("🗑 {n} buku catatan dihapus permanen.")
                                            }
                                            crate::model::AppLanguage::English => {
                                                format!("🗑 {n} notebooks permanently deleted.")
                                            }
                                        });
                                    }
                                    Err(e) => {
                                        self.toast_message = Some(format!("⚠ {e}"));
                                    }
                                }
                                self.confirm_empty_trash = false;
                            }
                            if ui.button(lang.tr("Batal", "Cancel")).clicked() {
                                self.confirm_empty_trash = false;
                            }
                        });
                    });
                });
        }
    }

    fn render_new_notebook_modal(&mut self, ctx: &egui::Context, palette: &UiPalette) {
        if !self.show_new_notebook_modal {
            return;
        }
        let lang = self.settings.language;
        egui::Window::new(lang.tr("Buat Buku Catatan Baru", "Create Notebook"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.set_width(580.0);
                ui.add_space(6.0);

                ui.columns(2, |cols| {
                    // Left Column: Cover & Title & Paper Size
                    cols[0].vertical(|ui| {
                        ui.label(RichText::new(lang.tr("Judul", "Title")).strong());
                        ui.text_edit_singleline(&mut self.new_notebook_title);
                        ui.add_space(12.0);

                        ui.label(
                            RichText::new(lang.tr("Pratinjau Sampul", "Cover Preview")).strong(),
                        );
                        let (rect, _) =
                            ui.allocate_exact_size(Vec2::new(120.0, 155.0), Sense::hover());
                        let title_preview = if self.new_notebook_title.trim().is_empty() {
                            lang.tr("Buku Tanpa Judul", "Untitled Notebook")
                        } else {
                            self.new_notebook_title.trim()
                        };
                        draw_goodnotes_cover(
                            &ui.painter_at(rect),
                            rect,
                            title_preview,
                            self.new_notebook_cover,
                            false,
                        );

                        ui.add_space(10.0);
                        ui.label(RichText::new(lang.tr("Warna Sampul", "Cover Color")).strong());
                        ui.horizontal_wrapped(|ui| {
                            for c in COVER_PALETTE {
                                let (rect, resp) =
                                    ui.allocate_exact_size(Vec2::new(22.0, 22.0), Sense::click());
                                let stroke = if self.new_notebook_cover == c {
                                    Stroke::new(2.5_f32, palette.accent)
                                } else {
                                    Stroke::new(1.0_f32, palette.border_subtle)
                                };
                                ui.painter_at(rect)
                                    .rect(rect, 4.0, c, stroke, StrokeKind::Outside);
                                if resp.clicked() {
                                    self.new_notebook_cover = c;
                                }
                            }
                        });
                        ui.add_space(10.0);
                        ui.label(RichText::new(lang.tr("Ukuran Kertas", "Paper Size")).strong());
                        ui.horizontal_wrapped(|ui| {
                            for preset in PaperSizePreset::ALL {
                                let is_sel = !self.new_notebook_use_custom_size
                                    && self.new_notebook_preset == preset;
                                if ui
                                    .selectable_label(is_sel, preset.short_name())
                                    .on_hover_text(preset.name())
                                    .clicked()
                                {
                                    self.new_notebook_use_custom_size = false;
                                    self.new_notebook_preset = preset;
                                    let (w, h) = preset.dimensions(self.new_notebook_orientation);
                                    self.new_notebook_custom_width = w;
                                    self.new_notebook_custom_height = h;
                                }
                            }
                            if ui
                                .selectable_label(
                                    self.new_notebook_use_custom_size,
                                    lang.tr("Ukuran Kustom", "Custom Size"),
                                )
                                .on_hover_text(lang.tr(
                                    "Kustomisasi lebar dan tinggi secara bebas",
                                    "Customize page width and height freely",
                                ))
                                .clicked()
                            {
                                self.new_notebook_use_custom_size = true;
                            }
                        });
                        ui.horizontal(|ui| {
                            if ui
                                .selectable_label(
                                    !self.new_notebook_use_custom_size
                                        && self.new_notebook_orientation
                                            == PageOrientation::Portrait,
                                    "Portrait",
                                )
                                .clicked()
                            {
                                self.new_notebook_use_custom_size = false;
                                self.new_notebook_orientation = PageOrientation::Portrait;
                                let (w, h) = self
                                    .new_notebook_preset
                                    .dimensions(self.new_notebook_orientation);
                                self.new_notebook_custom_width = w;
                                self.new_notebook_custom_height = h;
                            }
                            if ui
                                .selectable_label(
                                    !self.new_notebook_use_custom_size
                                        && self.new_notebook_orientation
                                            == PageOrientation::Landscape,
                                    "Landscape",
                                )
                                .clicked()
                            {
                                self.new_notebook_use_custom_size = false;
                                self.new_notebook_orientation = PageOrientation::Landscape;
                                let (w, h) = self
                                    .new_notebook_preset
                                    .dimensions(self.new_notebook_orientation);
                                self.new_notebook_custom_width = w;
                                self.new_notebook_custom_height = h;
                            }
                            if ui
                                .small_button(lang.tr("⇄ Tukar W×H", "⇄ Swap W×H"))
                                .on_hover_text(lang.tr(
                                    "Tukar Lebar dan Tinggi kertas",
                                    "Swap page width and height",
                                ))
                                .clicked()
                            {
                                self.new_notebook_use_custom_size = true;
                                std::mem::swap(
                                    &mut self.new_notebook_custom_width,
                                    &mut self.new_notebook_custom_height,
                                );
                            }
                        });
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("W:").size(11.5).color(palette.text_secondary));
                            if ui
                                .add(
                                    egui::DragValue::new(&mut self.new_notebook_custom_width)
                                        .range(200.0..=4000.0)
                                        .speed(2.0)
                                        .suffix(" pt"),
                                )
                                .changed()
                            {
                                self.new_notebook_use_custom_size = true;
                            }
                            ui.label(
                                RichText::new("× H:")
                                    .size(11.5)
                                    .color(palette.text_secondary),
                            );
                            if ui
                                .add(
                                    egui::DragValue::new(&mut self.new_notebook_custom_height)
                                        .range(200.0..=4000.0)
                                        .speed(2.0)
                                        .suffix(" pt"),
                                )
                                .changed()
                            {
                                self.new_notebook_use_custom_size = true;
                            }
                        });
                        let (pw, ph) = if self.new_notebook_use_custom_size {
                            (
                                self.new_notebook_custom_width.clamp(200.0, 4000.0),
                                self.new_notebook_custom_height.clamp(200.0, 4000.0),
                            )
                        } else {
                            self.new_notebook_preset
                                .dimensions(self.new_notebook_orientation)
                        };
                        ui.label(
                            RichText::new(format!(
                                "{}: {:.0} × {:.0} pt",
                                lang.tr("Ukuran Kertas", "Page Size"),
                                pw,
                                ph
                            ))
                            .size(11.0)
                            .color(palette.text_muted),
                        );
                    });

                    // Right Column: Paper Templates & Colors
                    cols[1].vertical(|ui| {
                        ui.label(
                            RichText::new(lang.tr("Template Kertas", "Paper Template")).strong(),
                        );
                        ui.add_space(6.0);

                        ScrollArea::vertical().max_height(280.0).show(ui, |ui| {
                            for tmpl in PageTemplate::ALL {
                                let is_selected = self.new_notebook_template == tmpl;
                                let (rect, resp) =
                                    ui.allocate_exact_size(Vec2::new(260.0, 44.0), Sense::click());
                                let bg = if is_selected {
                                    palette.bg_card_selected
                                } else if resp.hovered() {
                                    palette.bg_card_hover
                                } else {
                                    palette.bg_card
                                };
                                let border_stroke = if is_selected {
                                    Stroke::new(1.5_f32, palette.border_accent)
                                } else {
                                    Stroke::new(1.0_f32, palette.border_subtle)
                                };
                                ui.painter().rect(
                                    rect,
                                    6.0,
                                    bg,
                                    border_stroke,
                                    StrokeKind::Outside,
                                );
                                ui.painter().text(
                                    rect.left_top() + Vec2::new(10.0, 6.0),
                                    egui::Align2::LEFT_TOP,
                                    tmpl.name_tr(lang),
                                    egui::FontId::proportional(13.5),
                                    if is_selected {
                                        palette.accent_text
                                    } else {
                                        palette.text_primary
                                    },
                                );
                                ui.painter().text(
                                    rect.left_top() + Vec2::new(10.0, 24.0),
                                    egui::Align2::LEFT_TOP,
                                    tmpl.description_tr(lang),
                                    egui::FontId::proportional(10.5),
                                    palette.text_muted,
                                );
                                if resp.clicked() {
                                    self.new_notebook_template = tmpl;
                                }
                                ui.add_space(4.0);
                            }

                            if !self.custom_templates.is_empty() {
                                ui.add_space(6.0);
                                ui.label(
                                    RichText::new(
                                        lang.tr("⭐ Template Kustom:", "⭐ Custom Templates:"),
                                    )
                                    .strong()
                                    .size(12.0)
                                    .color(palette.warning),
                                );
                                for ctmpl in &self.custom_templates {
                                    let is_cur = self.new_notebook_template == ctmpl.template;
                                    let (rect, resp) = ui.allocate_exact_size(
                                        Vec2::new(260.0, 36.0),
                                        Sense::click(),
                                    );
                                    let bg = if is_cur {
                                        palette.bg_card_selected
                                    } else if resp.hovered() {
                                        palette.bg_card_hover
                                    } else {
                                        palette.bg_card
                                    };
                                    ui.painter().rect(
                                        rect,
                                        6.0,
                                        bg,
                                        Stroke::new(1.0_f32, palette.border_subtle),
                                        StrokeKind::Outside,
                                    );
                                    ui.painter().text(
                                        rect.left_top() + Vec2::new(10.0, 10.0),
                                        egui::Align2::LEFT_TOP,
                                        format!("📄 {}", ctmpl.name),
                                        egui::FontId::proportional(12.0),
                                        if is_cur {
                                            palette.accent_text
                                        } else {
                                            palette.text_primary
                                        },
                                    );
                                    if resp.clicked() {
                                        self.new_notebook_template = ctmpl.template;
                                    }
                                    ui.add_space(4.0);
                                }
                            }
                        });

                        ui.add_space(8.0);
                        ui.label(RichText::new(lang.tr("Warna Kertas", "Paper Color")).strong());
                        let paper_colors = [
                            (
                                Color32::from_rgb(255, 254, 250),
                                lang.tr("Putih Hangat", "Warm White"),
                            ),
                            (
                                Color32::from_rgb(255, 255, 255),
                                lang.tr("Putih Murni", "Pure White"),
                            ),
                            (
                                Color32::from_rgb(245, 240, 225),
                                lang.tr("Kuning Gading / Vintage", "Ivory Cream"),
                            ),
                            (
                                Color32::from_rgb(235, 240, 245),
                                lang.tr("Biru Muda", "Soft Blue"),
                            ),
                            (
                                Color32::from_rgb(240, 245, 235),
                                lang.tr("Hijau Sage", "Sage Green"),
                            ),
                            (
                                Color32::from_rgb(45, 45, 48),
                                lang.tr("Abu-abu Gelap", "Dark Slate"),
                            ),
                            (
                                Color32::from_rgb(26, 26, 28),
                                lang.tr("Hitam Pekat", "Deep Charcoal"),
                            ),
                        ];
                        ui.horizontal_wrapped(|ui| {
                            for (col, tip) in paper_colors {
                                let is_sel = self.new_notebook_paper_color == col;
                                let (rect, resp) =
                                    ui.allocate_exact_size(Vec2::new(24.0, 24.0), Sense::click());
                                let stroke = if is_sel {
                                    Stroke::new(2.5_f32, palette.accent)
                                } else {
                                    Stroke::new(1.0_f32, palette.border_strong)
                                };
                                ui.painter_at(rect).rect(
                                    rect,
                                    4.0,
                                    col,
                                    stroke,
                                    StrokeKind::Outside,
                                );
                                if resp.on_hover_text(tip).clicked() {
                                    self.new_notebook_paper_color = col;
                                }
                            }
                        });
                    });
                });

                ui.add_space(16.0);
                ui.separator();
                ui.add_space(8.0);

                ui.horizontal(|ui| {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let create_btn = Button::new(
                            RichText::new(lang.tr("Buat Buku Catatan", "Create Notebook"))
                                .strong()
                                .color(Color32::WHITE),
                        )
                        .fill(palette.accent)
                        .corner_radius(16.0)
                        .min_size(Vec2::new(150.0, 34.0));
                        if ui.add(create_btn).clicked() {
                            let title = if self.new_notebook_title.trim().is_empty() {
                                lang.tr("Buku Tanpa Judul", "Untitled Notebook")
                            } else {
                                self.new_notebook_title.trim()
                            };
                            let (w, h) = if self.new_notebook_use_custom_size {
                                (
                                    self.new_notebook_custom_width.clamp(200.0, 4000.0),
                                    self.new_notebook_custom_height.clamp(200.0, 4000.0),
                                )
                            } else {
                                self.new_notebook_preset
                                    .dimensions(self.new_notebook_orientation)
                            };
                            let mut nb = Notebook::new_with_size(
                                title,
                                self.new_notebook_template,
                                self.new_notebook_cover,
                                w,
                                h,
                            );
                            if let Some(first_page) = nb.pages.first_mut() {
                                first_page.paper_color = self.new_notebook_paper_color.into();
                            }
                            self.last_created_template = self.new_notebook_template;
                            self.last_created_preset = self.new_notebook_preset;
                            self.last_created_orientation = self.new_notebook_orientation;
                            self.last_created_use_custom = self.new_notebook_use_custom_size;
                            self.last_created_width = w;
                            self.last_created_height = h;
                            self.last_created_paper_color = self.new_notebook_paper_color;

                            nb.folder_id = self.current_folder_id.clone();
                            let new_id = nb.id.clone();
                            self.library.notebooks.push(nb);
                            self.force_save_notebook(&new_id);
                            self.show_new_notebook_modal = false;

                            if !self.open_notebook_ids.contains(&new_id) {
                                self.open_notebook_ids.push(new_id.clone());
                            }
                            self.active_notebook_id = Some(new_id);
                            self.current_page_index = 0;
                        }

                        if ui.button(lang.tr("Batal", "Cancel")).clicked() {
                            self.show_new_notebook_modal = false;
                        }
                    });
                });
            });
    }

    fn render_new_folder_modal(&mut self, ctx: &egui::Context, palette: &UiPalette) {
        if !self.show_new_folder_modal {
            return;
        }
        let lang = self.settings.language;
        egui::Window::new(lang.tr("📁 Folder Baru", "📁 New Folder"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.set_width(320.0);
                ui.label(RichText::new(lang.tr("Nama Folder", "Folder Name")).strong());
                ui.text_edit_singleline(&mut self.new_folder_name);
                ui.add_space(8.0);
                ui.label(RichText::new(lang.tr("Warna Folder", "Folder Color")).strong());
                ui.horizontal_wrapped(|ui| {
                    for c in COVER_PALETTE {
                        let (rect, resp) =
                            ui.allocate_exact_size(Vec2::new(22.0, 22.0), Sense::click());
                        let stroke = if self.new_folder_color == c {
                            Stroke::new(2.5_f32, palette.accent)
                        } else {
                            Stroke::new(1.0_f32, palette.border_subtle)
                        };
                        ui.painter_at(rect)
                            .rect(rect, 4.0, c, stroke, StrokeKind::Outside);
                        if resp.clicked() {
                            self.new_folder_color = c;
                        }
                    }
                });
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .add(
                                Button::new(
                                    RichText::new(lang.tr("Buat", "Create"))
                                        .color(Color32::WHITE)
                                        .strong(),
                                )
                                .fill(palette.accent),
                            )
                            .clicked()
                        {
                            let name = if self.new_folder_name.trim().is_empty() {
                                lang.tr("Folder Baru", "New Folder").to_string()
                            } else {
                                self.new_folder_name.trim().to_string()
                            };
                            let mut f =
                                crate::model::Folder::new(&name, self.current_folder_id.clone());
                            f.color = self.new_folder_color.into();
                            self.library.folders.push(f);
                            self.persist_library();
                            self.show_new_folder_modal = false;
                            self.new_folder_name.clear();
                        }
                        if ui.button(lang.tr("Batal", "Cancel")).clicked() {
                            self.show_new_folder_modal = false;
                        }
                    });
                });
            });
    }

    fn render_rename_notebook_modal(&mut self, ctx: &egui::Context, palette: &UiPalette) {
        let Some(nb_id) = self.renaming_notebook_id.clone() else {
            return;
        };
        let lang = self.settings.language;
        egui::Window::new(lang.tr("✏ Ubah Nama & Sampul Buku", "✏ Rename & Edit Cover"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.set_width(340.0);
                ui.label(RichText::new(lang.tr("Judul Buku Catatan:", "Notebook Title:")).strong());
                ui.text_edit_singleline(&mut self.rename_notebook_title);
                ui.add_space(10.0);
                ui.label(RichText::new(lang.tr("Warna Sampul:", "Cover Color:")).strong());
                ui.horizontal_wrapped(|ui| {
                    for c in COVER_PALETTE {
                        let (rect, resp) =
                            ui.allocate_exact_size(Vec2::new(24.0, 24.0), Sense::click());
                        let stroke = if self.rename_notebook_cover == c {
                            Stroke::new(2.5_f32, palette.accent)
                        } else {
                            Stroke::new(1.0_f32, palette.border_subtle)
                        };
                        ui.painter_at(rect)
                            .rect(rect, 4.0, c, stroke, StrokeKind::Outside);
                        if resp.clicked() {
                            self.rename_notebook_cover = c;
                        }
                    }
                });
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .add(
                                Button::new(
                                    RichText::new(lang.tr("Simpan", "Save"))
                                        .color(Color32::WHITE)
                                        .strong(),
                                )
                                .fill(palette.accent),
                            )
                            .clicked()
                        {
                            let trimmed = self.rename_notebook_title.trim();
                            if !trimmed.is_empty()
                                && let Some(nb) =
                                    self.library.notebooks.iter_mut().find(|n| n.id == nb_id)
                            {
                                nb.title = trimmed.to_string();
                                nb.cover_color = self.rename_notebook_cover.into();
                                nb.updated_at = chrono::Utc::now().timestamp_millis();
                                self.mark_dirty(&nb_id);
                                self.sync_library_index();
                            }
                            self.renaming_notebook_id = None;
                        }
                        if ui.button(lang.tr("Batal", "Cancel")).clicked() {
                            self.renaming_notebook_id = None;
                        }
                    });
                });
            });
    }

    fn render_rename_folder_modal(&mut self, ctx: &egui::Context, palette: &UiPalette) {
        let Some(fid) = self.renaming_folder_id.clone() else {
            return;
        };
        let lang = self.settings.language;
        egui::Window::new(lang.tr("✏ Ubah Nama & Warna Folder", "✏ Rename & Color Folder"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.set_width(320.0);
                ui.label(RichText::new(lang.tr("Nama Folder:", "Folder Name:")).strong());
                ui.text_edit_singleline(&mut self.rename_folder_name);
                ui.add_space(8.0);
                ui.label(RichText::new(lang.tr("Warna Folder:", "Folder Color:")).strong());
                ui.horizontal_wrapped(|ui| {
                    for c in COVER_PALETTE {
                        let (rect, resp) =
                            ui.allocate_exact_size(Vec2::new(22.0, 22.0), Sense::click());
                        let stroke = if self.rename_folder_color == c {
                            Stroke::new(2.5_f32, palette.accent)
                        } else {
                            Stroke::new(1.0_f32, palette.border_subtle)
                        };
                        ui.painter_at(rect)
                            .rect(rect, 4.0, c, stroke, StrokeKind::Outside);
                        if resp.clicked() {
                            self.rename_folder_color = c;
                        }
                    }
                });
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .add(
                                Button::new(
                                    RichText::new(lang.tr("Simpan", "Save"))
                                        .color(Color32::WHITE)
                                        .strong(),
                                )
                                .fill(palette.accent),
                            )
                            .clicked()
                        {
                            let trimmed = self.rename_folder_name.trim();
                            if !trimmed.is_empty()
                                && let Some(folder) =
                                    self.library.folders.iter_mut().find(|f| f.id == fid)
                            {
                                folder.name = trimmed.to_string();
                                folder.color = self.rename_folder_color.into();
                                self.persist_library();
                            }
                            self.renaming_folder_id = None;
                        }
                        if ui.button(lang.tr("Batal", "Cancel")).clicked() {
                            self.renaming_folder_id = None;
                        }
                    });
                });
            });
    }

    fn render_move_notebooks_modal(&mut self, ctx: &egui::Context, palette: &UiPalette) {
        let mut target_ids: Vec<String> = Vec::new();
        if let Some(id) = self.moving_notebook_id.clone() {
            target_ids.push(id);
        } else if !self.bulk_moving_notebook_ids.is_empty() {
            target_ids = self.bulk_moving_notebook_ids.clone();
        }
        if target_ids.is_empty() {
            return;
        }

        let lang = self.settings.language;
        let root_label = lang.tr("Semua Dokumen (Root)", "All Documents (Root)");
        let title = if target_ids.len() > 1 {
            match lang {
                crate::model::AppLanguage::Indonesian => {
                    format!("📁 Pindahkan {} Buku ke Folder", target_ids.len())
                }
                crate::model::AppLanguage::English => {
                    format!("📁 Move {} Notebooks to Folder", target_ids.len())
                }
            }
        } else {
            lang.tr("📁 Pindahkan Buku ke Folder", "📁 Move Notebook to Folder")
                .to_string()
        };

        egui::Window::new(title)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.set_width(340.0);
                ui.label(
                    RichText::new(lang.tr("Pilih folder tujuan:", "Select destination folder:"))
                        .strong(),
                );
                ui.add_space(8.0);
                let mut picked_dest: Option<Option<String>> = None;
                ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                    if ui.button(format!("📁 {root_label}")).clicked() {
                        picked_dest = Some(None);
                    }
                    if !self.library.folders.is_empty() {
                        ui.separator();
                    }
                    let folders = self.library.folders.clone();
                    for folder in &folders {
                        let path_text = folder_path_label(
                            &folders,
                            Some(&folder.id),
                            lang.tr("Dokumen", "Documents"),
                        );
                        if ui.button(format!("📁 {path_text}")).clicked() {
                            picked_dest = Some(Some(folder.id.clone()));
                        }
                    }
                });

                if let Some(dest) = picked_dest {
                    let moved = self.move_notebooks_to_folder(&target_ids, dest.as_deref());
                    let dest_name = match dest.as_deref() {
                        None => lang.tr("Semua Dokumen", "All Documents").to_string(),
                        Some(fid) => self
                            .library
                            .folders
                            .iter()
                            .find(|f| f.id == fid)
                            .map(|f| f.name.clone())
                            .unwrap_or_default(),
                    };
                    self.toast_message = Some(match lang {
                        crate::model::AppLanguage::Indonesian => {
                            format!("📁 {moved} buku dipindahkan ke '{dest_name}'.")
                        }
                        crate::model::AppLanguage::English => {
                            format!("📁 Moved {moved} notebook(s) to '{dest_name}'.")
                        }
                    });
                    self.moving_notebook_id = None;
                    self.bulk_moving_notebook_ids.clear();
                }

                ui.add_space(8.0);
                ui.label(
                    RichText::new(lang.tr(
                        "Tips: Anda juga dapat langsung Drag & Drop buku ke kartu folder atau jalur direktori.",
                        "Tip: You can also Drag & Drop notebooks directly onto folders or breadcrumb paths.",
                    ))
                    .size(10.5)
                    .color(palette.text_muted),
                );
                ui.add_space(8.0);
                if ui.button(lang.tr("Batal", "Cancel")).clicked() {
                    self.moving_notebook_id = None;
                    self.bulk_moving_notebook_ids.clear();
                }
            });
    }
}

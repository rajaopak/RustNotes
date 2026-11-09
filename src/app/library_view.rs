use super::cover::draw_goodnotes_cover;
use super::state::RustNotesApp;
use super::types::{COVER_PALETTE, NavView};
use crate::model::{Library, Notebook, PageOrientation, PageTemplate, PaperSizePreset, ThemeMode};
use crate::storage::{app_storage_dir, save_notebook_to_dir, save_settings_to_dir};
use eframe::egui::{
    self, Align, Button, Color32, CornerRadius, FontId, Layout, Rect, RichText, ScrollArea, Sense,
    Stroke, StrokeKind, Ui, Vec2,
};

impl RustNotesApp {
    pub(crate) fn render_library_view(&mut self, ctx: &egui::Context) {
        // Document Tabs Bar if any documents are opened
        if !self.open_notebook_ids.is_empty() {
            egui::TopBottomPanel::top("library_tabs")
                .exact_height(38.0)
                .show(ctx, |ui| {
                    ui.painter().rect_filled(
                        ui.max_rect(),
                        0.0,
                        Color32::from_rgb(40, 72, 114), // #284872
                    );
                    ui.horizontal_centered(|ui| {
                        ui.add_space(8.0);
                        let mut to_close = None;
                        for id in &self.open_notebook_ids {
                            if let Some(note) = self.library.notebooks.iter().find(|n| &n.id == id)
                            {
                                let is_active = self.active_notebook_id.as_ref() == Some(id);
                                let tab_color = if is_active {
                                    Color32::from_rgb(54, 95, 145) // #365f91
                                } else {
                                    Color32::TRANSPARENT
                                };
                                let (rect, resp) =
                                    ui.allocate_exact_size(Vec2::new(180.0, 32.0), Sense::click());
                                ui.painter().rect_filled(
                                    rect,
                                    CornerRadius {
                                        nw: 6,
                                        ne: 6,
                                        se: 0,
                                        sw: 0,
                                    },
                                    tab_color,
                                );
                                ui.painter().text(
                                    rect.left_center() + Vec2::new(12.0, 0.0),
                                    egui::Align2::LEFT_CENTER,
                                    format!("📖  {}", note.title),
                                    FontId::proportional(13.0),
                                    Color32::WHITE,
                                );
                                if resp.clicked() {
                                    self.active_notebook_id = Some(id.clone());
                                }
                                // Tab close X button
                                let close_rect = Rect::from_center_size(
                                    rect.right_center() - Vec2::new(16.0, 0.0),
                                    Vec2::new(18.0, 18.0),
                                );
                                if ui.put(close_rect, Button::new("✕").frame(false)).clicked() {
                                    to_close = Some(id.clone());
                                }
                            }
                        }
                        if let Some(id) = to_close {
                            self.open_notebook_ids.retain(|i| i != &id);
                            if self.active_notebook_id.as_ref() == Some(&id) {
                                self.active_notebook_id = self.open_notebook_ids.last().cloned();
                            }
                        }
                    });
                });
        }

        // Left Sidebar
        egui::SidePanel::left("library_sidebar")
            .resizable(false)
            .exact_width(240.0)
            .show(ctx, |ui| {
                ui.add_space(20.0);
                // Brand Header
                ui.horizontal(|ui| {
                    ui.add_space(10.0);
                    ui.label(RichText::new("📖").size(24.0));
                    ui.heading(
                        RichText::new("RustNotes")
                            .size(24.0)
                            .strong()
                            .color(Color32::WHITE),
                    );
                });
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.add_space(42.0);
                    ui.label(
                        RichText::new("Pure Native Windows Notebook")
                            .size(11.0)
                            .color(Color32::from_rgb(160, 156, 155)),
                    );
                });

                ui.add_space(32.0);
                ui.horizontal(|ui| {
                    ui.add_space(12.0);
                    ui.label(
                        RichText::new("MY LIBRARY")
                            .size(11.0)
                            .strong()
                            .color(Color32::from_rgb(140, 136, 135)),
                    );
                });
                ui.add_space(8.0);

                let nav_btn = |ui: &mut Ui,
                               selected: bool,
                               icon: &str,
                               label: &str,
                               count: Option<usize>|
                 -> bool {
                    let (rect, resp) =
                        ui.allocate_exact_size(Vec2::new(216.0, 42.0), Sense::click());
                    let bg = if selected {
                        Color32::from_rgb(39, 54, 91) // #27365b
                    } else if resp.hovered() {
                        Color32::from_rgb(50, 48, 48)
                    } else {
                        Color32::TRANSPARENT
                    };
                    let text_color = if selected {
                        Color32::from_rgb(76, 147, 236) // #4c93ec
                    } else {
                        Color32::from_rgb(240, 240, 240)
                    };
                    ui.painter().rect_filled(rect, 8.0, bg);
                    ui.painter().text(
                        rect.left_center() + Vec2::new(14.0, 0.0),
                        egui::Align2::LEFT_CENTER,
                        format!("{icon}  {label}"),
                        FontId::proportional(14.0),
                        text_color,
                    );
                    if let Some(c) = count
                        && c > 0
                    {
                        ui.painter().text(
                            rect.right_center() - Vec2::new(14.0, 0.0),
                            egui::Align2::RIGHT_CENTER,
                            format!("{c}"),
                            FontId::proportional(12.0),
                            Color32::GRAY,
                        );
                    }
                    resp.clicked()
                };

                let lang = self.settings.language;
                let docs_count = self.library.notebooks.iter().filter(|n| !n.trashed).count();
                if nav_btn(
                    ui,
                    self.current_nav_view == NavView::Documents,
                    "📁",
                    lang.tr("Dokumen", "Documents"),
                    Some(docs_count),
                ) {
                    self.current_nav_view = NavView::Documents;
                }
                let fav_count = self
                    .library
                    .notebooks
                    .iter()
                    .filter(|n| !n.trashed && n.favorite)
                    .count();
                if nav_btn(
                    ui,
                    self.current_nav_view == NavView::Favorites,
                    "⭐",
                    lang.tr("Favorit", "Favorites"),
                    Some(fav_count),
                ) {
                    self.current_nav_view = NavView::Favorites;
                }
                let trash_count = self.library.notebooks.iter().filter(|n| n.trashed).count();
                if nav_btn(
                    ui,
                    self.current_nav_view == NavView::Trash,
                    "🗑",
                    lang.tr("Sampah", "Trash"),
                    Some(trash_count),
                ) {
                    self.current_nav_view = NavView::Trash;
                }

                // Bottom Sidebar status & export/import
                ui.with_layout(Layout::bottom_up(Align::LEFT), |ui| {
                    ui.add_space(16.0);
                    ui.horizontal(|ui| {
                        ui.add_space(12.0);
                        ui.label(
                            RichText::new("⚡ 100% Rust & GPU Accelerated")
                                .size(11.0)
                                .color(Color32::from_rgb(120, 116, 115)),
                        );
                    });
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.add_space(12.0);
                        ui.label(
                            RichText::new("💾 Saved locally on this device")
                                .size(11.0)
                                .color(Color32::from_rgb(120, 116, 115)),
                        );
                    });
                    ui.add_space(12.0);

                    // Backup buttons
                    if ui.button("📥 Export Library Backup (.json)").clicked()
                        && let Some(path) = rfd::FileDialog::new()
                            .set_file_name("RustNotes-backup.json")
                            .save_file()
                        && let Ok(data) = serde_json::to_string_pretty(&self.library)
                    {
                        let _ = std::fs::write(path, data);
                    }
                    if ui.button("📤 Import Library Backup (.json)").clicked()
                        && let Some(path) = rfd::FileDialog::new()
                            .add_filter("JSON", &["json"])
                            .pick_file()
                        && let Ok(content) = std::fs::read_to_string(path)
                        && let Ok(lib) = serde_json::from_str::<Library>(&content)
                    {
                        self.library = lib;
                        self.persist_library();
                    }
                });
            });

        // Main Library Content
        egui::CentralPanel::default().show(ctx, |ui| {
            let lang = self.settings.language;
            ui.add_space(16.0);
            // Baris 1 Header: Judul di kiri, Pencarian & Pengaturan Cepat di kanan (tanpa tabrakan)
            ui.horizontal_wrapped(|ui| {
                ui.add_space(20.0);
                let title = match self.current_nav_view {
                    NavView::Documents => lang.tr("Dokumen", "Documents"),
                    NavView::Favorites => lang.tr("Favorit", "Favorites"),
                    NavView::Trash => lang.tr("Sampah", "Trash"),
                };
                ui.heading(
                    RichText::new(title)
                        .size(32.0)
                        .strong()
                        .color(Color32::WHITE),
                );

                ui.add_space(16.0);
                // Search bar
                ui.add(
                    egui::TextEdit::singleline(&mut self.search_query)
                        .hint_text(lang.tr("🔍 Cari catatan / OCR...", "🔍 Search notes / OCR..."))
                        .desired_width(200.0),
                );

                // Fase 8.7: Theme Mode toggle button (Dark / Light / System OS Windows)
                let theme_label = match self.settings.theme_mode {
                    ThemeMode::Dark => "🌙 Dark",
                    ThemeMode::Light => "☀ Light",
                    ThemeMode::System => "💻 System",
                };
                let theme_btn =
                    Button::new(RichText::new(theme_label).size(12.5).color(Color32::WHITE))
                        .fill(Color32::from_rgb(65, 65, 78))
                        .corner_radius(16.0)
                        .min_size(Vec2::new(76.0, 32.0));
                if ui
                    .add(theme_btn)
                    .on_hover_text(lang.tr(
                        "Siklus Tema: Gelap -> Terang -> Ikuti Sistem Windows",
                        "Cycle Theme: Dark -> Light -> Follow Windows System",
                    ))
                    .clicked()
                {
                    self.settings.theme_mode = self.settings.theme_mode.next();
                    let _ = save_settings_to_dir(&app_storage_dir(), &self.settings);
                }

                // Fase 8.8: Language Switcher (Bahasa Indonesia / Bahasa Inggris)
                let lang_btn =
                    Button::new(RichText::new(lang.badge()).size(12.5).color(Color32::WHITE))
                        .fill(Color32::from_rgb(58, 72, 92))
                        .corner_radius(16.0)
                        .min_size(Vec2::new(60.0, 32.0));
                if ui
                    .add(lang_btn)
                    .on_hover_text(lang.tr(
                        "Ganti Bahasa (Indonesia / Inggris)",
                        "Switch Language (Indonesian / English)",
                    ))
                    .clicked()
                {
                    self.settings.language = self.settings.language.toggle();
                    let _ = save_settings_to_dir(&app_storage_dir(), &self.settings);
                }

                // Fase 7: "About" button
                let about_btn = Button::new(
                    RichText::new(lang.tr("ℹ Tentang", "ℹ About"))
                        .size(12.5)
                        .color(Color32::WHITE),
                )
                .fill(Color32::from_rgb(40, 50, 65))
                .corner_radius(16.0)
                .min_size(Vec2::new(74.0, 32.0));
                if ui.add(about_btn).clicked() {
                    self.show_about_modal = true;
                }
            });

            // Baris 2 Header: Bilah Aksi Responsif (Wrap otomatis jika jendela kecil, tidak menabrak judul)
            if self.current_nav_view != NavView::Trash {
                ui.add_space(10.0);
                ui.horizontal_wrapped(|ui| {
                    ui.add_space(20.0);
                    let btn = Button::new(
                        RichText::new(lang.tr("+  Buku Baru", "+  New Notebook"))
                            .size(13.5)
                            .strong()
                            .color(Color32::WHITE),
                    )
                    .fill(Color32::from_rgb(0, 114, 206)) // Goodnotes Blue
                    .corner_radius(18.0)
                    .min_size(Vec2::new(124.0, 34.0));
                    if ui.add(btn).clicked() {
                        self.show_new_notebook_modal = true;
                        self.new_notebook_title = String::new();
                    }

                    // Fase 5: "New Folder" button
                    let folder_btn = Button::new(
                        RichText::new(lang.tr("📁+ Folder Baru", "📁+ New Folder"))
                            .size(13.0)
                            .color(Color32::WHITE),
                    )
                    .fill(Color32::from_rgb(55, 75, 100))
                    .corner_radius(18.0)
                    .min_size(Vec2::new(110.0, 34.0));
                    if ui.add(folder_btn).clicked() {
                        self.show_new_folder_modal = true;
                        self.new_folder_name.clear();
                    }

                    // Fase 3: "Import PDF" button
                    let import_btn = Button::new(
                        RichText::new(lang.tr("📥 Impor PDF", "📥 Import PDF"))
                            .size(13.0)
                            .color(Color32::WHITE),
                    )
                    .fill(Color32::from_rgb(45, 60, 85))
                    .corner_radius(18.0)
                    .min_size(Vec2::new(105.0, 34.0));
                    if ui.add(import_btn).clicked()
                        && let Some(path) = rfd::FileDialog::new()
                            .add_filter("PDF Document", &["pdf"])
                            .pick_file()
                    {
                        self.import_pdf_as_notebook(&path);
                    }

                    // Fase 7: "Import .rnote" button
                    let rnote_btn = Button::new(
                        RichText::new(lang.tr("📦 Impor .rnote", "📦 Import .rnote"))
                            .size(13.0)
                            .color(Color32::WHITE),
                    )
                    .fill(Color32::from_rgb(50, 70, 95))
                    .corner_radius(18.0)
                    .min_size(Vec2::new(110.0, 34.0));
                    if ui.add(rnote_btn).clicked()
                        && let Some(path) = rfd::FileDialog::new()
                            .add_filter("RustNotes Document (*.rnote)", &["rnote", "zip"])
                            .pick_file()
                    {
                        self.open_external_file(&path);
                    }
                });
            }

            ui.add_space(14.0);
            ui.separator();
            ui.add_space(14.0);

            // Fase 5: Breadcrumb navigation for Documents view
            if self.current_nav_view == NavView::Documents {
                ui.horizontal(|ui| {
                    ui.add_space(20.0);
                    let is_root = self.current_folder_id.is_none();
                    if ui.selectable_label(is_root, "📁 All Documents").clicked() {
                        self.current_folder_id = None;
                    }
                    if let Some(fid) = &self.current_folder_id {
                        let mut chain = Vec::new();
                        let mut curr = Some(fid.clone());
                        while let Some(c_id) = curr {
                            if let Some(f) = self.library.folders.iter().find(|f| f.id == c_id) {
                                chain.push((f.id.clone(), f.name.clone()));
                                curr = f.parent_id.clone();
                            } else {
                                break;
                            }
                        }
                        chain.reverse();
                        for (id, name) in chain {
                            ui.label(" / ");
                            let is_active = self.current_folder_id.as_ref() == Some(&id);
                            if ui
                                .selectable_label(is_active, format!("📁 {name}"))
                                .clicked()
                            {
                                self.current_folder_id = Some(id);
                            }
                        }
                    }
                });
                ui.add_space(8.0);
            }

            // Fase 5: Subfolders list in current folder
            let mut action_open_folder = None;
            let mut action_delete_folder = None;
            let query = self.search_query.trim().to_lowercase();

            if self.current_nav_view == NavView::Documents && query.is_empty() {
                let current_subfolders: Vec<crate::model::Folder> = self
                    .library
                    .folders
                    .iter()
                    .filter(|f| f.parent_id == self.current_folder_id)
                    .cloned()
                    .collect();

                if !current_subfolders.is_empty() {
                    ui.horizontal_wrapped(|ui| {
                        ui.add_space(20.0);
                        for folder in &current_subfolders {
                            let count = self
                                .library
                                .notebooks
                                .iter()
                                .filter(|n| {
                                    !n.trashed && n.folder_id.as_deref() == Some(&folder.id)
                                })
                                .count();
                            let (rect, resp) =
                                ui.allocate_exact_size(Vec2::new(130.0, 70.0), Sense::click());
                            let painter = ui.painter_at(rect);

                            let bg_col = if resp.hovered() {
                                Color32::from_rgb(55, 52, 53)
                            } else {
                                Color32::from_rgb(42, 40, 41)
                            };
                            painter.rect_filled(rect, 8.0, bg_col);
                            painter.rect_stroke(
                                rect,
                                8.0,
                                Stroke::new(1.0_f32, Color32::from_rgb(70, 67, 67)),
                                StrokeKind::Outside,
                            );

                            // Top color bar
                            painter.rect_filled(
                                Rect::from_min_size(rect.min, Vec2::new(rect.width(), 4.0)),
                                CornerRadius {
                                    nw: 8,
                                    ne: 8,
                                    se: 0,
                                    sw: 0,
                                },
                                folder.color.to_color32(),
                            );

                            painter.text(
                                rect.left_top() + Vec2::new(10.0, 14.0),
                                egui::Align2::LEFT_TOP,
                                "📁",
                                FontId::proportional(22.0),
                                Color32::WHITE,
                            );
                            painter.text(
                                rect.left_top() + Vec2::new(42.0, 14.0),
                                egui::Align2::LEFT_TOP,
                                &folder.name,
                                FontId::proportional(14.0),
                                Color32::WHITE,
                            );
                            painter.text(
                                rect.left_top() + Vec2::new(42.0, 38.0),
                                egui::Align2::LEFT_TOP,
                                format!("{count} items"),
                                FontId::proportional(11.0),
                                Color32::GRAY,
                            );

                            if resp.clicked() {
                                action_open_folder = Some(folder.id.clone());
                            }
                            resp.context_menu(|ui| {
                                if ui.button("🗑 Hapus Folder").clicked() {
                                    action_delete_folder = Some(folder.id.clone());
                                    ui.close_menu();
                                }
                            });
                        }
                    });
                    ui.add_space(14.0);
                }
            }

            if let Some(fid) = action_open_folder {
                self.current_folder_id = Some(fid);
            }
            if let Some(del_id) = action_delete_folder {
                let parent = self
                    .library
                    .folders
                    .iter()
                    .find(|f| f.id == del_id)
                    .and_then(|f| f.parent_id.clone());
                let mut moved_nb_ids = Vec::new();
                for nb in &mut self.library.notebooks {
                    if nb.folder_id.as_deref() == Some(&del_id) {
                        nb.folder_id = parent.clone();
                        moved_nb_ids.push(nb.id.clone());
                    }
                }
                for id in moved_nb_ids {
                    self.mark_dirty(&id);
                }
                for f in &mut self.library.folders {
                    if f.parent_id.as_deref() == Some(&del_id) {
                        f.parent_id = parent.clone();
                    }
                }
                self.library.folders.retain(|f| f.id != del_id);
                if self.current_folder_id.as_deref() == Some(&del_id) {
                    self.current_folder_id = parent;
                }
                self.persist_library();
            }

            // Fase 5: Smart In-Memory Search across titles, text objects, and sticky notes
            let mut notebooks_to_show: Vec<(usize, Notebook, Option<String>)> = Vec::new();
            for (i, n) in self.library.notebooks.iter().enumerate() {
                let matches_view = match self.current_nav_view {
                    NavView::Documents => {
                        !n.trashed
                            && (query.is_empty() && n.folder_id == self.current_folder_id
                                || !query.is_empty())
                    }
                    NavView::Favorites => !n.trashed && n.favorite,
                    NavView::Trash => n.trashed,
                };
                if !matches_view {
                    continue;
                }
                if query.is_empty() || n.title.to_lowercase().contains(&query) {
                    notebooks_to_show.push((i, n.clone(), None));
                } else {
                    let mut found_snippet = None;
                    for (p_idx, page) in n.pages.iter().enumerate() {
                        // Fase 8.6: Cek kecocokan pada indeks OCR tulisan tangan halaman
                        if !page.ocr_text.is_empty()
                            && page.ocr_text.to_lowercase().contains(&query)
                        {
                            let snip: String = page.ocr_text.chars().take(28).collect();
                            found_snippet = Some(format!("✍ OCR Hal. {}: \"{snip}\"", p_idx + 1));
                            break;
                        }
                        for obj in &page.objects {
                            match obj {
                                crate::model::DrawObject::Text { text, .. }
                                | crate::model::DrawObject::StickyNote { text, .. }
                                    if text.to_lowercase().contains(&query) =>
                                {
                                    let snip: String = text.chars().take(28).collect();
                                    found_snippet = Some(format!("Hal. {}: \"{snip}\"", p_idx + 1));
                                    break;
                                }
                                crate::model::DrawObject::Shape {
                                    label: Some(lbl), ..
                                } if lbl.text.to_lowercase().contains(&query) => {
                                    let snip: String = lbl.text.chars().take(28).collect();
                                    found_snippet =
                                        Some(format!("📐 Bentuk Hal. {}: \"{snip}\"", p_idx + 1));
                                    break;
                                }
                                _ => {}
                            }
                        }
                        if found_snippet.is_some() {
                            break;
                        }
                    }
                    if let Some(snippet) = found_snippet {
                        notebooks_to_show.push((i, n.clone(), Some(snippet)));
                    }
                }
            }

            if notebooks_to_show.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space(90.0);
                    ui.label(RichText::new("📖").size(56.0));
                    ui.add_space(10.0);
                    ui.heading(
                        RichText::new(lang.tr("Belum ada buku catatan", "No notebooks here yet"))
                            .size(22.0)
                            .color(Color32::WHITE),
                    );
                    ui.label(
                        RichText::new(lang.tr(
                            "Buat buku catatan baru untuk mulai menulis dan menggambar.",
                            "Create a notebook to begin sketching and writing.",
                        ))
                        .size(14.0)
                        .color(Color32::GRAY),
                    );
                    if self.current_nav_view != NavView::Trash {
                        ui.add_space(16.0);
                        let btn = Button::new(
                            RichText::new(lang.tr("+  Buat Buku Catatan", "+  Create Notebook"))
                                .size(14.0)
                                .strong()
                                .color(Color32::WHITE),
                        )
                        .fill(Color32::from_rgb(0, 114, 206))
                        .corner_radius(18.0)
                        .min_size(Vec2::new(150.0, 38.0));
                        if ui.add(btn).clicked() {
                            self.show_new_notebook_modal = true;
                            self.new_notebook_title = String::new();
                        }
                    }
                });
            } else {
                ScrollArea::vertical().show(ui, |ui| {
                    ui.add_space(8.0);
                    ui.horizontal_wrapped(|ui| {
                        ui.add_space(20.0);
                        let mut to_open = None;
                        let mut to_toggle_fav = None;
                        let mut to_trash = None;
                        let mut to_restore = None;
                        let mut to_duplicate = None;
                        let mut action_move_nb = None;
                        // Fase 6: Export ZIP & Import ZIP
                        let mut to_export_zip: Option<(String, String)> = None; // (nb_id, nb_title)

                        for (orig_idx, note, snippet) in &notebooks_to_show {
                            ui.vertical(|ui| {
                                ui.set_width(136.0);

                                // Realistic Goodnotes Cover Card
                                let (rect, resp) =
                                    ui.allocate_exact_size(Vec2::new(130.0, 168.0), Sense::click());
                                let painter = ui.painter_at(rect);

                                // Draw Book Cover with realistic spine, label badge, watermark lines and star!
                                draw_goodnotes_cover(
                                    &painter,
                                    rect,
                                    &note.title,
                                    note.cover_color.to_color32(),
                                    note.favorite,
                                );

                                if resp.hovered() {
                                    painter.rect_stroke(
                                        rect,
                                        CornerRadius {
                                            nw: 0,
                                            ne: 12,
                                            se: 12,
                                            sw: 0,
                                        },
                                        Stroke::new(2.0_f32, Color32::from_rgb(76, 147, 236)),
                                        StrokeKind::Outside,
                                    );
                                }

                                if resp.clicked() && !note.trashed {
                                    to_open = Some(note.id.clone());
                                }

                                resp.context_menu(|ui| {
                                    if note.trashed {
                                        if ui.button("↩ Restore Notebook").clicked() {
                                            to_restore = Some(*orig_idx);
                                            ui.close_menu();
                                        }
                                    } else {
                                        if ui.button("📖 Open").clicked() {
                                            to_open = Some(note.id.clone());
                                            ui.close_menu();
                                        }
                                        let fav_text = if note.favorite {
                                            "★ Remove Favorite"
                                        } else {
                                            "☆ Add to Favorites"
                                        };
                                        if ui.button(fav_text).clicked() {
                                            to_toggle_fav = Some(*orig_idx);
                                            ui.close_menu();
                                        }
                                        if ui.button("📄 Duplicate").clicked() {
                                            to_duplicate = Some(*orig_idx);
                                            ui.close_menu();
                                        }
                                        ui.separator();
                                        // Fase 5: Move to Folder
                                        if ui.button("📁 Move to Folder...").clicked() {
                                            action_move_nb = Some(note.id.clone());
                                            ui.close_menu();
                                        }
                                        ui.separator();
                                        // Fase 6 & 7: Export as .rnote / ZIP
                                        if ui.button("📦 Export as .rnote...").clicked() {
                                            to_export_zip =
                                                Some((note.id.clone(), note.title.clone()));
                                            ui.close_menu();
                                        }
                                        ui.separator();
                                        if ui.button("🗑 Move to Trash").clicked() {
                                            to_trash = Some(*orig_idx);
                                            ui.close_menu();
                                        }
                                    }
                                });

                                ui.add_space(8.0);
                                ui.label(
                                    RichText::new(&note.title)
                                        .strong()
                                        .size(14.0)
                                        .color(Color32::WHITE),
                                );
                                let page_str = if note.pages.len() == 1 {
                                    lang.tr("halaman", "page")
                                } else {
                                    lang.tr("halaman", "pages")
                                };
                                ui.label(
                                    RichText::new(format!("{} {}", note.pages.len(), page_str))
                                        .size(11.0)
                                        .color(Color32::from_rgb(140, 136, 135)),
                                );
                                if let Some(snip) = snippet {
                                    ui.label(
                                        RichText::new(format!("🔍 {snip}"))
                                            .size(10.0)
                                            .color(Color32::from_rgb(76, 147, 236)),
                                    );
                                }
                                ui.add_space(20.0);
                            });
                            ui.add_space(14.0);
                        }

                        if let Some(id) = action_move_nb {
                            self.moving_notebook_id = Some(id);
                        }

                        if let Some(id) = to_open {
                            if !self.open_notebook_ids.contains(&id) {
                                self.open_notebook_ids.push(id.clone());
                            }
                            self.active_notebook_id = Some(id);
                            self.current_page_index = 0;
                        }
                        if let Some(idx) = to_toggle_fav {
                            self.library.notebooks[idx].favorite =
                                !self.library.notebooks[idx].favorite;
                            self.persist_library();
                        }
                        if let Some(idx) = to_duplicate {
                            let mut clone = self.library.notebooks[idx].clone();
                            clone.id = uuid::Uuid::new_v4().to_string();
                            clone.title = format!("{} copy", clone.title);
                            self.library.notebooks.push(clone);
                            self.persist_library();
                        }
                        if let Some(idx) = to_trash {
                            self.library.notebooks[idx].trashed = true;
                            self.persist_library();
                        }
                        if let Some(idx) = to_restore {
                            self.library.notebooks[idx].trashed = false;
                            self.persist_library();
                        }
                        // Fase 6: Handle Export ZIP
                        if let Some((nb_id, nb_title)) = to_export_zip {
                            let storage_dir = app_storage_dir();
                            // Force-save dulu kalau ada perubahan belum tersimpan
                            if self.dirty_notebooks.contains(&nb_id)
                                && let Some(nb) =
                                    self.library.notebooks.iter().find(|n| n.id == nb_id)
                            {
                                let _ = save_notebook_to_dir(&storage_dir, nb);
                                self.dirty_notebooks.remove(&nb_id);
                            }
                            let safe_title = nb_title
                                .replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");
                            let default_name = format!("{safe_title}.rnote");
                            if let Some(path) = rfd::FileDialog::new()
                                .set_file_name(&default_name)
                                .add_filter("RustNotes Document (*.rnote)", &["rnote"])
                                .add_filter("ZIP Archive (*.zip)", &["zip"])
                                .save_file()
                            {
                                match crate::storage::export_notebook_to_rnote(
                                    &storage_dir,
                                    &nb_id,
                                    &path,
                                ) {
                                    Ok(()) => {
                                        self.toast_message = Some(format!(
                                            "✅ Notebook '{nb_title}' berhasil diekspor ke .rnote."
                                        ));
                                    }
                                    Err(e) => {
                                        self.toast_message =
                                            Some(format!("⚠ Gagal ekspor .rnote: {e}"));
                                    }
                                }
                            }
                        }
                    });
                });
            }
        });

        // Goodnotes 2-Column New Notebook Modal Dialog
        if self.show_new_notebook_modal {
            egui::Window::new("Create Notebook")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.set_width(580.0);
                    ui.set_height(420.0);
                    ui.add_space(8.0);

                    ui.columns(2, |cols| {
                        // Left Column: Cover & Title
                        cols[0].vertical(|ui| {
                            ui.label(RichText::new("Title").strong());
                            ui.text_edit_singleline(&mut self.new_notebook_title);
                            ui.add_space(14.0);

                            ui.label(RichText::new("Cover Preview").strong());
                            let (rect, _) =
                                ui.allocate_exact_size(Vec2::new(120.0, 155.0), Sense::hover());
                            let title_preview = if self.new_notebook_title.trim().is_empty() {
                                "Untitled Notebook"
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

                            ui.add_space(12.0);
                            ui.label(RichText::new("Cover Color").strong());
                            ui.horizontal_wrapped(|ui| {
                                for c in COVER_PALETTE {
                                    let (rect, resp) = ui
                                        .allocate_exact_size(Vec2::new(22.0, 22.0), Sense::click());
                                    let stroke = if self.new_notebook_cover == c {
                                        Stroke::new(2.5_f32, Color32::WHITE)
                                    } else {
                                        Stroke::NONE
                                    };
                                    ui.painter_at(rect).rect(
                                        rect,
                                        4.0,
                                        c,
                                        stroke,
                                        StrokeKind::Outside,
                                    );
                                    if resp.clicked() {
                                        self.new_notebook_cover = c;
                                    }
                                }
                            });
                            ui.add_space(10.0);
                            ui.label(RichText::new("Paper Size").strong());
                            ui.horizontal_wrapped(|ui| {
                                for preset in PaperSizePreset::ALL {
                                    let is_sel = !self.new_notebook_use_custom_size
                                        && self.new_notebook_preset == preset;
                                    let name = match preset {
                                        PaperSizePreset::A4 => "A4",
                                        PaperSizePreset::Letter => "Letter",
                                        PaperSizePreset::A5 => "A5",
                                        PaperSizePreset::Legacy => "Legacy",
                                    };
                                    if ui.selectable_label(is_sel, name).clicked() {
                                        self.new_notebook_use_custom_size = false;
                                        self.new_notebook_preset = preset;
                                        let (w, h) =
                                            preset.dimensions(self.new_notebook_orientation);
                                        self.new_notebook_custom_width = w;
                                        self.new_notebook_custom_height = h;
                                    }
                                }
                                let is_slide = self.new_notebook_use_custom_size
                                    && (self.new_notebook_custom_width - 1280.0).abs() < 1.0
                                    && (self.new_notebook_custom_height - 720.0).abs() < 1.0;
                                if ui.selectable_label(is_slide, "16:9 Slide").clicked() {
                                    self.new_notebook_use_custom_size = true;
                                    self.new_notebook_custom_width = 1280.0;
                                    self.new_notebook_custom_height = 720.0;
                                }
                                if ui
                                    .selectable_label(
                                        self.new_notebook_use_custom_size && !is_slide,
                                        "Custom Size",
                                    )
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
                                    .small_button("⇄ Swap W×H")
                                    .on_hover_text("Tukar Lebar dan Tinggi kertas")
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
                                ui.label(RichText::new("W:").size(11.5).color(Color32::LIGHT_GRAY));
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
                                    RichText::new("× H:").size(11.5).color(Color32::LIGHT_GRAY),
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
                                RichText::new(format!("Ukuran Kertas: {:.0} × {:.0} pt", pw, ph))
                                    .size(11.0)
                                    .color(Color32::GRAY),
                            );
                        });

                        // Right Column: Paper Templates
                        cols[1].vertical(|ui| {
                            ui.label(RichText::new("Paper Template").strong());
                            ui.add_space(6.0);

                            let templates = [
                                (
                                    PageTemplate::Ruled,
                                    "Ruled Narrow",
                                    "Horizontal writing lines",
                                ),
                                (
                                    PageTemplate::Grid,
                                    "Squared (Grid)",
                                    "Math & structure grid",
                                ),
                                (
                                    PageTemplate::Dotted,
                                    "Dotted Paper",
                                    "Bullet journaling dots",
                                ),
                                (
                                    PageTemplate::Blank,
                                    "Blank Paper",
                                    "Plain unlined sketch paper",
                                ),
                                (
                                    PageTemplate::Cornell,
                                    "Cornell Notes",
                                    "Summary & cue column template",
                                ),
                            ];

                            for (tmpl, label, desc) in templates {
                                let is_selected = self.new_notebook_template == tmpl;
                                let (rect, resp) =
                                    ui.allocate_exact_size(Vec2::new(270.0, 48.0), Sense::click());
                                let bg = if is_selected {
                                    Color32::from_rgb(39, 54, 91)
                                } else if resp.hovered() {
                                    Color32::from_rgb(52, 50, 51)
                                } else {
                                    Color32::from_rgb(38, 36, 37)
                                };
                                let border_stroke = if is_selected {
                                    Stroke::new(1.5_f32, Color32::from_rgb(76, 147, 236))
                                } else {
                                    Stroke::new(1.0_f32, Color32::from_rgb(60, 57, 57))
                                };
                                ui.painter().rect(
                                    rect,
                                    6.0,
                                    bg,
                                    border_stroke,
                                    StrokeKind::Outside,
                                );
                                ui.painter().text(
                                    rect.left_top() + Vec2::new(12.0, 10.0),
                                    egui::Align2::LEFT_TOP,
                                    label,
                                    FontId::proportional(14.0),
                                    if is_selected {
                                        Color32::from_rgb(76, 147, 236)
                                    } else {
                                        Color32::WHITE
                                    },
                                );
                                ui.painter().text(
                                    rect.left_top() + Vec2::new(12.0, 28.0),
                                    egui::Align2::LEFT_TOP,
                                    desc,
                                    FontId::proportional(11.0),
                                    Color32::GRAY,
                                );
                                if resp.clicked() {
                                    self.new_notebook_template = tmpl;
                                }
                            }

                            // Fase 5: Custom Templates section if any exist
                            if !self.custom_templates.is_empty() {
                                ui.add_space(8.0);
                                ui.label(
                                    RichText::new("⭐ Template Kustom:")
                                        .strong()
                                        .size(12.0)
                                        .color(Color32::from_rgb(255, 204, 0)),
                                );
                                for ctmpl in &self.custom_templates {
                                    let is_cur = self.new_notebook_template == ctmpl.template;
                                    let (rect, resp) = ui.allocate_exact_size(
                                        Vec2::new(270.0, 36.0),
                                        Sense::click(),
                                    );
                                    let bg = if is_cur {
                                        Color32::from_rgb(39, 54, 91)
                                    } else if resp.hovered() {
                                        Color32::from_rgb(52, 50, 51)
                                    } else {
                                        Color32::from_rgb(38, 36, 37)
                                    };
                                    ui.painter().rect(
                                        rect,
                                        6.0,
                                        bg,
                                        Stroke::new(1.0_f32, Color32::from_rgb(60, 57, 57)),
                                        StrokeKind::Outside,
                                    );
                                    ui.painter().text(
                                        rect.left_top() + Vec2::new(10.0, 10.0),
                                        egui::Align2::LEFT_TOP,
                                        format!("📄 {}", ctmpl.name),
                                        FontId::proportional(12.0),
                                        if is_cur {
                                            Color32::from_rgb(76, 147, 236)
                                        } else {
                                            Color32::WHITE
                                        },
                                    );
                                    if resp.clicked() {
                                        self.new_notebook_template = ctmpl.template;
                                    }
                                }
                            }
                        });
                    });

                    ui.add_space(20.0);
                    ui.separator();
                    ui.add_space(10.0);

                    // Modal Footer Buttons
                    ui.horizontal(|ui| {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let create_btn = Button::new(
                                RichText::new("Create Notebook")
                                    .strong()
                                    .color(Color32::WHITE),
                            )
                            .fill(Color32::from_rgb(0, 114, 206))
                            .corner_radius(16.0)
                            .min_size(Vec2::new(140.0, 34.0));
                            if ui.add(create_btn).clicked() {
                                let title = if self.new_notebook_title.trim().is_empty() {
                                    "Untitled Notebook"
                                } else {
                                    self.new_notebook_title.trim()
                                };
                                let mut nb = if self.new_notebook_use_custom_size {
                                    Notebook::new_with_size(
                                        title,
                                        self.new_notebook_template,
                                        self.new_notebook_cover,
                                        self.new_notebook_custom_width.clamp(200.0, 4000.0),
                                        self.new_notebook_custom_height.clamp(200.0, 4000.0),
                                    )
                                } else {
                                    Notebook::new_with_preset(
                                        title,
                                        self.new_notebook_template,
                                        self.new_notebook_cover,
                                        self.new_notebook_preset,
                                        self.new_notebook_orientation,
                                    )
                                };
                                nb.folder_id = self.current_folder_id.clone();
                                let new_id = nb.id.clone();
                                self.library.notebooks.push(nb);
                                // Fase 6: Simpan notebook baru langsung (force-save)
                                self.force_save_notebook(&new_id);
                                self.show_new_notebook_modal = false;

                                if !self.open_notebook_ids.contains(&new_id) {
                                    self.open_notebook_ids.push(new_id.clone());
                                }
                                self.active_notebook_id = Some(new_id);
                                self.current_page_index = 0;
                            }

                            if ui.button("Cancel").clicked() {
                                self.show_new_notebook_modal = false;
                            }
                        });
                    });
                });
        }

        // Fase 5: Create New Folder Modal Dialog
        if self.show_new_folder_modal {
            egui::Window::new("📁 New Folder")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.set_width(320.0);
                    ui.label(RichText::new("Folder Name").strong());
                    ui.text_edit_singleline(&mut self.new_folder_name);
                    ui.add_space(8.0);
                    ui.label(RichText::new("Folder Color").strong());
                    ui.horizontal_wrapped(|ui| {
                        for c in COVER_PALETTE {
                            let (rect, resp) =
                                ui.allocate_exact_size(Vec2::new(22.0, 22.0), Sense::click());
                            let stroke = if self.new_folder_color == c {
                                Stroke::new(2.5_f32, Color32::WHITE)
                            } else {
                                Stroke::NONE
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
                            if ui.button("Create").clicked() {
                                let name = if self.new_folder_name.trim().is_empty() {
                                    "New Folder".to_string()
                                } else {
                                    self.new_folder_name.trim().to_string()
                                };
                                let mut f = crate::model::Folder::new(
                                    &name,
                                    self.current_folder_id.clone(),
                                );
                                f.color = self.new_folder_color.into();
                                self.library.folders.push(f);
                                self.persist_library();
                                self.show_new_folder_modal = false;
                                self.new_folder_name.clear();
                            }
                            if ui.button("Cancel").clicked() {
                                self.show_new_folder_modal = false;
                            }
                        });
                    });
                });
        }

        // Fase 5: Move Notebook to Folder Modal Dialog
        if let Some(nb_id) = self.moving_notebook_id.clone() {
            egui::Window::new("📁 Move Notebook to Folder")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.set_width(320.0);
                    ui.label(RichText::new("Select destination folder:").strong());
                    ui.add_space(8.0);
                    ScrollArea::vertical().max_height(240.0).show(ui, |ui| {
                        if ui.button("📁 Root (All Documents)").clicked() {
                            if let Some(nb) =
                                self.library.notebooks.iter_mut().find(|n| n.id == nb_id)
                            {
                                nb.folder_id = None;
                                self.mark_dirty(&nb_id);
                                self.sync_library_index();
                            }
                            self.moving_notebook_id = None;
                        }
                        ui.separator();
                        let folders = self.library.folders.clone();
                        for folder in folders {
                            if ui.button(format!("📁 {}", folder.name)).clicked() {
                                if let Some(nb) =
                                    self.library.notebooks.iter_mut().find(|n| n.id == nb_id)
                                {
                                    nb.folder_id = Some(folder.id);
                                    self.mark_dirty(&nb_id);
                                    self.sync_library_index();
                                }
                                self.moving_notebook_id = None;
                            }
                        }
                    });
                    ui.add_space(8.0);
                    if ui.button("Cancel").clicked() {
                        self.moving_notebook_id = None;
                    }
                });
        }
    }
}

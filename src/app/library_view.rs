use super::cover::{draw_goodnotes_cover, fit_title_to_width};
use super::library_layout::{
    can_move_folder_into, compute_folder_grid, compute_responsive_grid, folder_breadcrumb_chain,
    folder_path_label, format_relative_time, notebook_has_pdf_pages, sort_notebook_items,
};
use super::state::RustNotesApp;
use super::theme::UiPalette;
use super::types::{LibraryDragPayload, LibraryFilter, LibrarySortBy, LibraryViewStyle, NavView};
use crate::model::Notebook;
use crate::storage::{app_storage_dir, export_notebook_to_rnote};
use eframe::egui::{
    self, Align, Button, Color32, CornerRadius, FontId, Layout, Order, Pos2, Rect, RichText,
    ScrollArea, Sense, Stroke, StrokeKind, Vec2,
};

impl RustNotesApp {
    /// Memindahkan daftar buku ke folder target (atau `None` untuk root).
    pub(crate) fn move_notebooks_to_folder(
        &mut self,
        nb_ids: &[String],
        dest_folder: Option<&str>,
    ) -> usize {
        let mut moved_ids = Vec::new();
        for nb in &mut self.library.notebooks {
            if nb_ids.contains(&nb.id) && nb.folder_id.as_deref() != dest_folder {
                nb.folder_id = dest_folder.map(str::to_string);
                moved_ids.push(nb.id.clone());
            }
        }
        let count = moved_ids.len();
        for id in moved_ids {
            self.mark_dirty(&id);
        }
        if count > 0 {
            self.sync_library_index();
        }
        count
    }

    /// Memindahkan folder ke parent folder target dengan proteksi anti-siklus.
    pub(crate) fn move_folder_to_parent(
        &mut self,
        folder_id: &str,
        dest_parent: Option<&str>,
    ) -> bool {
        if !can_move_folder_into(&self.library.folders, folder_id, dest_parent) {
            return false;
        }
        let mut changed = false;
        if let Some(f) = self.library.folders.iter_mut().find(|f| f.id == folder_id)
            && f.parent_id.as_deref() != dest_parent
        {
            f.parent_id = dest_parent.map(str::to_string);
            changed = true;
        }
        if changed {
            self.persist_library();
        }
        changed
    }

    /// Membuat buku catatan baru dalam 1 klik memakai konfigurasi template/ukuran kertas terakhir.
    fn create_quick_notebook(&mut self) {
        let lang = self.settings.language;
        let base_title = lang.tr("Catatan Cepat", "Quick Note");
        let existing_quick = self
            .library
            .notebooks
            .iter()
            .filter(|n| !n.trashed && n.title.starts_with(base_title))
            .count();
        let title = if existing_quick == 0 {
            base_title.to_string()
        } else {
            format!("{base_title} {}", existing_quick + 1)
        };
        let (w, h) = if self.last_created_use_custom {
            (
                self.last_created_width.clamp(200.0, 4000.0),
                self.last_created_height.clamp(200.0, 4000.0),
            )
        } else {
            self.last_created_preset
                .dimensions(self.last_created_orientation)
        };
        let mut nb = Notebook::new_with_size(
            &title,
            self.last_created_template,
            self.new_notebook_cover,
            w,
            h,
        );
        if let Some(p) = nb.pages.first_mut() {
            p.paper_color = self.last_created_paper_color.into();
        }
        if self.current_nav_view == NavView::Documents {
            nb.folder_id = self.current_folder_id.clone();
        }
        let new_id = nb.id.clone();
        self.library.notebooks.push(nb);
        self.force_save_notebook(&new_id);
        if !self.open_notebook_ids.contains(&new_id) {
            self.open_notebook_ids.push(new_id.clone());
        }
        self.active_notebook_id = Some(new_id);
        self.current_page_index = 0;
    }

    pub(crate) fn render_library_view(&mut self, ctx: &egui::Context) {
        let palette = UiPalette::from_ctx(ctx, self.settings.theme_mode);
        let lang = self.settings.language;
        let pointer_released = ctx.input(|i| i.pointer.any_released());

        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.library_drag_payload = None;
            if self.library_select_mode || !self.library_selected_ids.is_empty() {
                self.library_select_mode = false;
                self.library_selected_ids.clear();
            }
        }

        // Bersihkan ID terpilih bila buku sudah tidak ada di library
        self.library_selected_ids
            .retain(|id| self.library.notebooks.iter().any(|n| &n.id == id));

        let viewport_w = ctx.screen_rect().width();
        let compact_sidebar = self.library_sidebar_collapsed || viewport_w < 720.0;
        let sidebar_w = if compact_sidebar { 62.0 } else { 232.0 };

        let mut drop_to_root = false;
        let mut drop_to_favorites = false;
        let mut drop_to_trash = false;

        // Left Sidebar (MY LIBRARY + Recent + Open Now + Utilities)
        egui::SidePanel::left("library_sidebar")
            .resizable(false)
            .exact_width(sidebar_w)
            .frame(
                egui::Frame::NONE
                    .fill(palette.bg_sidebar)
                    .stroke(Stroke::new(1.0_f32, palette.border_subtle))
                    .inner_margin(egui::Margin::symmetric(
                        if compact_sidebar { 6 } else { 12 },
                        14,
                    )),
            )
            .show(ctx, |ui| {
                // Header Brand & Collapse Toggle
                ui.horizontal(|ui| {
                    if !compact_sidebar {
                        ui.label(RichText::new("📖").size(22.0));
                        ui.heading(
                            RichText::new("RustNotes")
                                .size(21.0)
                                .strong()
                                .color(palette.text_primary),
                        );
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let toggle_icon = if compact_sidebar { "»" } else { "«" };
                        let toggle_tip = if compact_sidebar {
                            lang.tr("Perluas Sidebar", "Expand Sidebar")
                        } else {
                            lang.tr("Perkecil Sidebar", "Collapse Sidebar")
                        };
                        if ui
                            .add(
                                Button::new(
                                    RichText::new(toggle_icon)
                                        .size(13.0)
                                        .strong()
                                        .color(palette.text_secondary),
                                )
                                .fill(palette.btn_neutral_bg)
                                .corner_radius(6.0)
                                .min_size(Vec2::new(26.0, 26.0)),
                            )
                            .on_hover_text(toggle_tip)
                            .clicked()
                        {
                            self.library_sidebar_collapsed = !self.library_sidebar_collapsed;
                        }
                    });
                });

                if !compact_sidebar {
                    ui.label(
                        RichText::new(lang.tr(
                            "Buku Catatan Native Windows",
                            "Pure Native Windows Notebook",
                        ))
                        .size(11.0)
                        .color(palette.text_muted),
                    );
                    ui.add_space(20.0);
                    ui.label(
                        RichText::new(lang.tr("PUSTAKA SAYA", "MY LIBRARY"))
                            .size(10.5)
                            .strong()
                            .color(palette.text_muted),
                    );
                    ui.add_space(6.0);
                } else {
                    ui.add_space(16.0);
                }

                let btn_w = ui.available_width();
                let dragging_nb = matches!(
                    self.library_drag_payload,
                    Some(LibraryDragPayload::Notebooks(_))
                );
                let dragging_any = self.library_drag_payload.is_some();

                let nav_btn = |ui: &mut egui::Ui,
                               selected: bool,
                               icon: &str,
                               label: &str,
                               count: Option<usize>,
                               accept_drop: bool,
                               drop_hint: Option<&str>|
                 -> (bool, bool) {
                    let (rect, resp) =
                        ui.allocate_exact_size(Vec2::new(btn_w, 38.0), Sense::click());
                    let is_drop_hover = accept_drop && resp.hovered();
                    let bg = if is_drop_hover || selected {
                        palette.accent_soft_bg
                    } else if resp.hovered() {
                        palette.bg_card_hover
                    } else {
                        Color32::TRANSPARENT
                    };
                    let text_color = if selected || is_drop_hover {
                        palette.accent_text
                    } else {
                        palette.text_primary
                    };
                    ui.painter().rect_filled(rect, 8.0, bg);
                    if is_drop_hover {
                        ui.painter().rect_stroke(
                            rect,
                            8.0,
                            Stroke::new(1.5_f32, palette.accent),
                            StrokeKind::Inside,
                        );
                    }
                    if compact_sidebar {
                        ui.painter().text(
                            rect.center(),
                            egui::Align2::CENTER_CENTER,
                            icon,
                            FontId::proportional(16.0),
                            text_color,
                        );
                    } else {
                        ui.painter().text(
                            rect.left_center() + Vec2::new(12.0, 0.0),
                            egui::Align2::LEFT_CENTER,
                            format!("{icon}  {label}"),
                            FontId::proportional(13.5),
                            text_color,
                        );
                        if is_drop_hover && let Some(hint) = drop_hint {
                            ui.painter().text(
                                rect.right_center() - Vec2::new(10.0, 0.0),
                                egui::Align2::RIGHT_CENTER,
                                hint,
                                FontId::proportional(10.5),
                                palette.accent_text,
                            );
                        } else if let Some(c) = count
                            && c > 0
                        {
                            ui.painter().text(
                                rect.right_center() - Vec2::new(12.0, 0.0),
                                egui::Align2::RIGHT_CENTER,
                                format!("{c}"),
                                FontId::proportional(11.5),
                                palette.text_muted,
                            );
                        }
                    }
                    (resp.clicked(), is_drop_hover && pointer_released)
                };

                let docs_count = self.library.notebooks.iter().filter(|n| !n.trashed).count();
                let fav_count = self
                    .library
                    .notebooks
                    .iter()
                    .filter(|n| !n.trashed && n.favorite)
                    .count();
                let trash_count = self.library.notebooks.iter().filter(|n| n.trashed).count();

                // 🕒 Recent (Persisten antar-sesi berdasarkan updated_at)
                let (recent_click, _) = nav_btn(
                    ui,
                    self.current_nav_view == NavView::Recent,
                    "🕒",
                    lang.tr("Terbaru", "Recent"),
                    Some(docs_count),
                    false,
                    None,
                );
                if recent_click {
                    self.current_nav_view = NavView::Recent;
                    self.library_selected_ids.clear();
                    self.library_select_mode = false;
                }

                // 📁 Documents (Drop target -> move to Root folder)
                let (docs_click, docs_drop) = nav_btn(
                    ui,
                    self.current_nav_view == NavView::Documents,
                    "📁",
                    lang.tr("Dokumen", "Documents"),
                    Some(docs_count),
                    dragging_any,
                    Some(lang.tr("→ Root", "→ Root")),
                );
                if docs_click {
                    self.current_nav_view = NavView::Documents;
                    self.library_selected_ids.clear();
                    self.library_select_mode = false;
                }
                if docs_drop {
                    drop_to_root = true;
                }

                // ⭐ Favorites (Drop target -> mark favorite)
                let (fav_click, fav_drop) = nav_btn(
                    ui,
                    self.current_nav_view == NavView::Favorites,
                    "⭐",
                    lang.tr("Favorit", "Favorites"),
                    Some(fav_count),
                    dragging_nb,
                    Some(lang.tr("+ Favorit", "+ Fav")),
                );
                if fav_click {
                    self.current_nav_view = NavView::Favorites;
                    self.library_selected_ids.clear();
                    self.library_select_mode = false;
                }
                if fav_drop {
                    drop_to_favorites = true;
                }

                // 🗑 Trash (Drop target -> move to Trash)
                let (trash_click, trash_drop) = nav_btn(
                    ui,
                    self.current_nav_view == NavView::Trash,
                    "🗑",
                    lang.tr("Sampah", "Trash"),
                    Some(trash_count),
                    dragging_nb,
                    Some(lang.tr("→ Sampah", "→ Trash")),
                );
                if trash_click {
                    self.current_nav_view = NavView::Trash;
                    self.library_selected_ids.clear();
                    self.library_select_mode = false;
                }
                if trash_drop {
                    drop_to_trash = true;
                }

                // Pengganti top tab bar: daftar dokumen yang sedang terbuka pada Sidebar (`OPEN NOW`)
                if !compact_sidebar && !self.open_notebook_ids.is_empty() {
                    ui.add_space(16.0);
                    ui.label(
                        RichText::new(lang.tr("SEDANG DIBUKA", "OPEN NOW"))
                            .size(10.5)
                            .strong()
                            .color(palette.text_muted),
                    );
                    ui.add_space(4.0);
                    let mut reopen_id = None;
                    let mut close_tab_id = None;
                    let open_ids = self.open_notebook_ids.clone();
                    for id in open_ids.into_iter().take(6) {
                        if let Some(nb) = self
                            .library
                            .notebooks
                            .iter()
                            .find(|n| n.id == id && !n.trashed)
                        {
                            let row_w = ui.available_width();
                            let (row_rect, row_resp) =
                                ui.allocate_exact_size(Vec2::new(row_w, 28.0), Sense::click());
                            let close_rect = Rect::from_center_size(
                                row_rect.right_center() - Vec2::new(14.0, 0.0),
                                Vec2::new(18.0, 18.0),
                            );
                            if row_resp.hovered() {
                                ui.painter()
                                    .rect_filled(row_rect, 6.0, palette.bg_card_hover);
                            }
                            let font = FontId::proportional(12.5);
                            let fitted = fit_title_to_width(
                                ui.painter(),
                                &nb.title,
                                &font,
                                palette.text_primary,
                                (row_w - 48.0).max(40.0),
                            );
                            ui.painter().text(
                                row_rect.left_center() + Vec2::new(8.0, 0.0),
                                egui::Align2::LEFT_CENTER,
                                format!("📖 {fitted}"),
                                font,
                                palette.text_primary,
                            );
                            let close_hovered = ctx
                                .input(|i| i.pointer.hover_pos())
                                .is_some_and(|p| close_rect.contains(p));
                            if close_hovered {
                                ui.painter().rect_filled(
                                    close_rect,
                                    4.0,
                                    palette.btn_neutral_hover,
                                );
                            }
                            ui.painter().text(
                                close_rect.center(),
                                egui::Align2::CENTER_CENTER,
                                "✕",
                                FontId::proportional(10.5),
                                if close_hovered {
                                    palette.danger
                                } else {
                                    palette.text_muted
                                },
                            );
                            if row_resp.clicked() {
                                if close_hovered {
                                    close_tab_id = Some(id.clone());
                                } else {
                                    reopen_id = Some(id.clone());
                                }
                            }
                        }
                    }
                    if let Some(id) = close_tab_id {
                        self.open_notebook_ids.retain(|i| i != &id);
                    } else if let Some(id) = reopen_id {
                        self.active_notebook_id = Some(id);
                        self.current_page_index = 0;
                    }
                }

                // Bottom Sidebar: Keyboard Shortcuts + Backup Import/Export + Storage Info
                ui.with_layout(Layout::bottom_up(Align::LEFT), |ui| {
                    if !compact_sidebar {
                        ui.label(
                            RichText::new("⚡ 100% Rust & GPU Accelerated")
                                .size(10.5)
                                .color(palette.text_muted),
                        );
                        ui.add_space(2.0);
                        ui.label(
                            RichText::new(lang.tr(
                                "💾 Tersimpan lokal (%LOCALAPPDATA%)",
                                "💾 Saved locally (%LOCALAPPDATA%)",
                            ))
                            .size(10.5)
                            .color(palette.text_muted),
                        );
                        ui.add_space(10.0);
                    }

                    let sidebar_util_btn = |ui: &mut egui::Ui, label: &str, hover: &str| -> bool {
                        ui.add(
                            Button::new(
                                RichText::new(label)
                                    .size(12.0)
                                    .color(palette.btn_neutral_text),
                            )
                            .fill(palette.btn_neutral_bg)
                            .corner_radius(7.0)
                            .min_size(Vec2::new(btn_w, 30.0)),
                        )
                        .on_hover_text(hover)
                        .clicked()
                    };

                    if sidebar_util_btn(
                        ui,
                        if compact_sidebar {
                            "⌨"
                        } else {
                            lang.tr("⌨ Pintasan Keyboard", "⌨ Keyboard Shortcuts")
                        },
                        lang.tr(
                            "Buka pengaturan pintasan keyboard & stylus (F1)",
                            "Open keyboard shortcut & stylus settings (F1)",
                        ),
                    ) {
                        self.show_shortcut_modal = true;
                    }
                    ui.add_space(4.0);

                    if sidebar_util_btn(
                        ui,
                        if compact_sidebar {
                            "📥"
                        } else {
                            lang.tr("📥 Ekspor Backup (.json)", "📥 Export Backup (.json)")
                        },
                        lang.tr(
                            "Ekspor seluruh library ke berkas cadangan JSON",
                            "Export entire library to a JSON backup file",
                        ),
                    ) && let Some(path) = rfd::FileDialog::new()
                        .set_file_name("RustNotes-backup.json")
                        .save_file()
                    {
                        match serde_json::to_string_pretty(&self.library) {
                            Ok(data) => {
                                if let Err(e) =
                                    crate::storage::atomic_write_file(&path, &data, None)
                                {
                                    self.toast_message = Some(format!(
                                        "{}: {e}",
                                        lang.tr(
                                            "⚠ Gagal mengekspor cadangan",
                                            "⚠ Failed to export backup"
                                        )
                                    ));
                                } else {
                                    self.toast_message = Some(
                                        lang.tr(
                                            "✅ Cadangan library berhasil diekspor.",
                                            "✅ Library backup exported successfully.",
                                        )
                                        .to_string(),
                                    );
                                }
                            }
                            Err(e) => {
                                self.toast_message = Some(format!(
                                    "{}: {e}",
                                    lang.tr(
                                        "⚠ Gagal memproses data cadangan",
                                        "⚠ Failed to serialize backup data"
                                    )
                                ));
                            }
                        }
                    }
                    ui.add_space(4.0);

                    if sidebar_util_btn(
                        ui,
                        if compact_sidebar {
                            "📤"
                        } else {
                            lang.tr("📤 Impor Backup (.json)", "📤 Import Backup (.json)")
                        },
                        lang.tr(
                            "Impor cadangan library dari berkas JSON",
                            "Import library backup from a JSON file",
                        ),
                    ) && let Some(path) = rfd::FileDialog::new()
                        .add_filter("JSON", &["json"])
                        .pick_file()
                        && let Err(e) = self.import_library_backup_from_path(&path)
                    {
                        self.toast_message = Some(format!(
                            "{}: {e}",
                            lang.tr("⚠ Gagal mengimpor cadangan", "⚠ Failed to import backup")
                        ));
                    }
                });
            });

        // Apply sidebar drag drops immediately before rendering content
        if let Some(payload) = self.library_drag_payload.clone()
            && pointer_released
            && (drop_to_root || drop_to_favorites || drop_to_trash)
        {
            match payload {
                LibraryDragPayload::Notebooks(ids) => {
                    if drop_to_root {
                        let moved = self.move_notebooks_to_folder(&ids, None);
                        if moved > 0 {
                            self.toast_message = Some(match lang {
                                crate::model::AppLanguage::Indonesian => {
                                    format!("📁 {moved} buku dipindahkan ke Semua Dokumen.")
                                }
                                crate::model::AppLanguage::English => {
                                    format!("📁 Moved {moved} notebook(s) to All Documents.")
                                }
                            });
                        }
                    } else if drop_to_favorites {
                        let mut changed = 0usize;
                        for nb in &mut self.library.notebooks {
                            if ids.contains(&nb.id) && !nb.favorite {
                                nb.favorite = true;
                                changed += 1;
                            }
                        }
                        if changed > 0 {
                            self.persist_library();
                            self.toast_message = Some(match lang {
                                crate::model::AppLanguage::Indonesian => {
                                    format!("⭐ {changed} buku ditambahkan ke Favorit.")
                                }
                                crate::model::AppLanguage::English => {
                                    format!("⭐ Added {changed} notebook(s) to Favorites.")
                                }
                            });
                        }
                    } else if drop_to_trash {
                        let mut changed = 0usize;
                        for nb in &mut self.library.notebooks {
                            if ids.contains(&nb.id) && !nb.trashed {
                                nb.trashed = true;
                                changed += 1;
                            }
                        }
                        if changed > 0 {
                            self.library_selected_ids.clear();
                            self.library_select_mode = false;
                            self.persist_library();
                            self.toast_message = Some(match lang {
                                crate::model::AppLanguage::Indonesian => {
                                    format!("🗑 {changed} buku dipindahkan ke Sampah.")
                                }
                                crate::model::AppLanguage::English => {
                                    format!("🗑 Moved {changed} notebook(s) to Trash.")
                                }
                            });
                        }
                    }
                }
                LibraryDragPayload::Folder(fid) => {
                    if drop_to_root && self.move_folder_to_parent(&fid, None) {
                        self.toast_message = Some(
                            lang.tr(
                                "📁 Folder dipindahkan ke Semua Dokumen.",
                                "📁 Folder moved to All Documents.",
                            )
                            .to_string(),
                        );
                    }
                }
            }
            self.library_drag_payload = None;
        }

        // Main Library Central Panel
        egui::CentralPanel::default()
            .frame(
                egui::Frame::NONE
                    .fill(palette.bg_panel)
                    .inner_margin(egui::Margin::symmetric(20, 16)),
            )
            .show(ctx, |ui| {
                // --- HEADER BARIS 1: Judul Halaman & Kontrol Global (Search, Theme, Lang, About) ---
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(8.0, 8.0);
                    let title = match self.current_nav_view {
                        NavView::Recent => lang.tr("Terbaru", "Recent"),
                        NavView::Documents => lang.tr("Dokumen", "Documents"),
                        NavView::Favorites => lang.tr("Favorit", "Favorites"),
                        NavView::Trash => lang.tr("Sampah", "Trash"),
                    };
                    ui.heading(
                        RichText::new(title)
                            .size(28.0)
                            .strong()
                            .color(palette.text_primary),
                    );

                    ui.add_space(6.0);
                    let search_w = if ui.available_width() > 640.0 {
                        230.0
                    } else {
                        160.0
                    };
                    egui::Frame::NONE
                        .fill(palette.bg_input)
                        .stroke(Stroke::new(1.0_f32, palette.border_subtle))
                        .corner_radius(16.0)
                        .inner_margin(egui::Margin::symmetric(10, 4))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.search_query)
                                        .hint_text(lang.tr(
                                            "🔍 Cari catatan / OCR...",
                                            "🔍 Search notes / OCR...",
                                        ))
                                        .frame(false)
                                        .desired_width(search_w),
                                );
                                if !self.search_query.is_empty()
                                    && ui
                                        .small_button("✕")
                                        .on_hover_text(lang.tr("Hapus pencarian", "Clear search"))
                                        .clicked()
                                {
                                    self.search_query.clear();
                                }
                            });
                        });

                    let theme_btn = Button::new(
                        RichText::new(self.settings.theme_mode.label_tr(lang))
                            .size(12.0)
                            .color(palette.btn_neutral_text),
                    )
                    .fill(palette.btn_neutral_bg)
                    .corner_radius(16.0)
                    .min_size(Vec2::new(82.0, 30.0));
                    if ui
                        .add(theme_btn)
                        .on_hover_text(lang.tr(
                            "Siklus Tema: Gelap -> Terang -> Ikuti Sistem Windows",
                            "Cycle Theme: Dark -> Light -> Follow Windows System",
                        ))
                        .clicked()
                    {
                        self.settings.theme_mode = self.settings.theme_mode.next();
                        self.persist_settings();
                    }

                    let lang_btn = Button::new(
                        RichText::new(lang.badge())
                            .size(12.0)
                            .color(palette.btn_neutral_text),
                    )
                    .fill(palette.btn_neutral_bg)
                    .corner_radius(16.0)
                    .min_size(Vec2::new(60.0, 30.0));
                    if ui
                        .add(lang_btn)
                        .on_hover_text(lang.tr(
                            "Ganti Bahasa (Indonesia / Inggris)",
                            "Switch Language (Indonesian / English)",
                        ))
                        .clicked()
                    {
                        self.settings.language = self.settings.language.toggle();
                        self.persist_settings();
                    }

                    let about_btn = Button::new(
                        RichText::new(lang.tr("ℹ Tentang", "ℹ About"))
                            .size(12.0)
                            .color(palette.btn_neutral_text),
                    )
                    .fill(palette.btn_neutral_bg)
                    .corner_radius(16.0)
                    .min_size(Vec2::new(76.0, 30.0));
                    if ui.add(about_btn).clicked() {
                        self.show_about_modal = true;
                    }
                });

                ui.add_space(10.0);

                // --- HEADER BARIS 2: Tombol Aksi + Filter, Sort, Grid/List, Bulk Select ---
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);

                    if self.current_nav_view == NavView::Trash {
                        let has_trash = self.library.notebooks.iter().any(|n| n.trashed);
                        if has_trash {
                            let empty_btn = Button::new(
                                RichText::new(lang.tr("🗑 Kosongkan Sampah", "🗑 Empty Trash"))
                                    .size(12.5)
                                    .strong()
                                    .color(Color32::WHITE),
                            )
                            .fill(palette.danger_soft_bg)
                            .corner_radius(16.0)
                            .min_size(Vec2::new(138.0, 32.0));
                            if ui.add(empty_btn).clicked() {
                                self.confirm_empty_trash = true;
                            }
                        }
                    } else {
                        let new_btn = Button::new(
                            RichText::new(lang.tr("+  Buku Baru", "+  New Notebook"))
                                .size(13.0)
                                .strong()
                                .color(Color32::WHITE),
                        )
                        .fill(palette.accent)
                        .corner_radius(16.0)
                        .min_size(Vec2::new(124.0, 32.0));
                        if ui.add(new_btn).clicked() {
                            self.show_new_notebook_modal = true;
                            self.new_notebook_title.clear();
                        }

                        let quick_btn = Button::new(
                            RichText::new(lang.tr("⚡ Catatan Cepat", "⚡ Quick Note"))
                                .size(12.5)
                                .color(palette.accent_text),
                        )
                        .fill(palette.accent_soft_bg)
                        .stroke(Stroke::new(1.0_f32, palette.border_accent))
                        .corner_radius(16.0)
                        .min_size(Vec2::new(112.0, 32.0));
                        if ui
                            .add(quick_btn)
                            .on_hover_text(lang.tr(
                                "Buat buku catatan baru seketika memakai template & ukuran terakhir",
                                "Instantly create a new notebook using your last template & size",
                            ))
                            .clicked()
                        {
                            self.create_quick_notebook();
                        }

                        if self.current_nav_view == NavView::Documents {
                            let folder_btn = Button::new(
                                RichText::new(lang.tr("📁+ Folder Baru", "📁+ New Folder"))
                                    .size(12.5)
                                    .color(palette.btn_neutral_text),
                            )
                            .fill(palette.btn_neutral_bg)
                            .corner_radius(16.0)
                            .min_size(Vec2::new(110.0, 32.0));
                            if ui.add(folder_btn).clicked() {
                                self.show_new_folder_modal = true;
                                self.new_folder_name.clear();
                            }
                        }

                        let import_pdf_btn = Button::new(
                            RichText::new(lang.tr("📥 Impor PDF", "📥 Import PDF"))
                                .size(12.5)
                                .color(palette.btn_neutral_text),
                        )
                        .fill(palette.btn_neutral_bg)
                        .corner_radius(16.0)
                        .min_size(Vec2::new(102.0, 32.0));
                        if ui.add(import_pdf_btn).clicked()
                            && let Some(path) = rfd::FileDialog::new()
                                .add_filter("PDF Document", &["pdf"])
                                .pick_file()
                        {
                            self.import_pdf_as_notebook(&path);
                        }

                        let rnote_btn = Button::new(
                            RichText::new(lang.tr("📦 Impor .rnote", "📦 Import .rnote"))
                                .size(12.5)
                                .color(palette.btn_neutral_text),
                        )
                        .fill(palette.btn_neutral_bg)
                        .corner_radius(16.0)
                        .min_size(Vec2::new(106.0, 32.0));
                        if ui.add(rnote_btn).clicked()
                            && let Some(path) = rfd::FileDialog::new()
                                .add_filter("RustNotes Document (*.rnote)", &["rnote", "zip"])
                                .pick_file()
                        {
                            self.open_external_file(&path);
                        }
                    }

                    ui.separator();

                    // Filter Cepat (All / PDF / Favorites)
                    for f in LibraryFilter::ALL {
                        if self.current_nav_view == NavView::Favorites && f == LibraryFilter::FavoritesOnly {
                            continue;
                        }
                        let is_sel = self.library_filter == f;
                        if ui
                            .add(
                                Button::new(
                                    RichText::new(f.label(lang))
                                        .size(11.5)
                                        .color(if is_sel {
                                            palette.accent_text
                                        } else {
                                            palette.text_secondary
                                        }),
                                )
                                .fill(if is_sel {
                                    palette.accent_soft_bg
                                } else {
                                    palette.bg_card
                                })
                                .stroke(Stroke::new(
                                    1.0_f32,
                                    if is_sel {
                                        palette.border_accent
                                    } else {
                                        palette.border_subtle
                                    },
                                ))
                                .corner_radius(14.0)
                                .min_size(Vec2::new(48.0, 28.0)),
                            )
                            .clicked()
                        {
                            self.library_filter = f;
                        }
                    }

                    // Sort Menu
                    ui.menu_button(
                        RichText::new(format!("↕ {}", self.library_sort_by.label(lang)))
                            .size(11.5)
                            .color(palette.btn_neutral_text),
                        |ui| {
                            for s in LibrarySortBy::ALL {
                                if ui
                                    .selectable_label(self.library_sort_by == s, s.label(lang))
                                    .clicked()
                                {
                                    self.library_sort_by = s;
                                    ui.close_menu();
                                }
                            }
                        },
                    );

                    // Grid / List View Switcher
                    let grid_active = self.library_view_style == LibraryViewStyle::Grid;
                    if ui
                        .add(
                            Button::new(
                                RichText::new("▦")
                                    .size(13.0)
                                    .color(if grid_active {
                                        palette.accent_text
                                    } else {
                                        palette.text_secondary
                                    }),
                            )
                            .fill(if grid_active {
                                palette.accent_soft_bg
                            } else {
                                palette.btn_neutral_bg
                            })
                            .corner_radius(6.0)
                            .min_size(Vec2::new(28.0, 28.0)),
                        )
                        .on_hover_text(lang.tr("Tampilan Grid (Sampul)", "Grid Cover View"))
                        .clicked()
                    {
                        self.library_view_style = LibraryViewStyle::Grid;
                    }
                    if ui
                        .add(
                            Button::new(
                                RichText::new("☰")
                                    .size(13.0)
                                    .color(if !grid_active {
                                        palette.accent_text
                                    } else {
                                        palette.text_secondary
                                    }),
                            )
                            .fill(if !grid_active {
                                palette.accent_soft_bg
                            } else {
                                palette.btn_neutral_bg
                            })
                            .corner_radius(6.0)
                            .min_size(Vec2::new(28.0, 28.0)),
                        )
                        .on_hover_text(lang.tr("Tampilan Daftar (List)", "List View"))
                        .clicked()
                    {
                        self.library_view_style = LibraryViewStyle::List;
                    }

                    // Bulk Select Toggle
                    let sel_btn = Button::new(
                        RichText::new(if self.library_select_mode {
                            lang.tr("☑ Selesai Pilih", "☑ Done Selecting")
                        } else {
                            lang.tr("☑ Pilih", "☑ Select")
                        })
                        .size(12.0)
                        .color(if self.library_select_mode {
                            Color32::WHITE
                        } else {
                            palette.btn_neutral_text
                        }),
                    )
                    .fill(if self.library_select_mode {
                        palette.accent
                    } else {
                        palette.btn_neutral_bg
                    })
                    .corner_radius(14.0)
                    .min_size(Vec2::new(72.0, 28.0));
                    if ui
                        .add(sel_btn)
                        .on_hover_text(lang.tr(
                            "Mode seleksi massal (tips: Ctrl+Klik kartu untuk langsung memilih)",
                            "Bulk selection mode (tip: Ctrl+Click cards to select quickly)",
                        ))
                        .clicked()
                    {
                        self.library_select_mode = !self.library_select_mode;
                        if !self.library_select_mode {
                            self.library_selected_ids.clear();
                        }
                    }
                });

                ui.add_space(10.0);
                ui.separator();
                ui.add_space(8.0);

                // --- DIRECTORY PATH & INTERACTIVE DROP-TARGET BREADCRUMB BAR ---
                let mut nav_target_folder: Option<Option<String>> = None;
                let mut drop_to_folder: Option<Option<String>> = None;
                let dragging_any = self.library_drag_payload.is_some();

                if self.current_nav_view == NavView::Documents {
                    let chain = folder_breadcrumb_chain(
                        &self.library.folders,
                        self.current_folder_id.as_deref(),
                    );
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(6.0, 4.0);

                        // Tombol naik 1 tingkat folder (`⬆`)
                        if self.current_folder_id.is_some() {
                            let parent_of_cur = self
                                .current_folder_id
                                .as_deref()
                                .and_then(|cid| {
                                    self.library.folders.iter().find(|f| f.id == cid)
                                })
                                .and_then(|f| f.parent_id.clone());
                            if ui
                                .add(
                                    Button::new(
                                        RichText::new("⬆")
                                            .size(12.0)
                                            .color(palette.btn_neutral_text),
                                    )
                                    .fill(palette.btn_neutral_bg)
                                    .corner_radius(6.0)
                                    .min_size(Vec2::new(26.0, 24.0)),
                                )
                                .on_hover_text(lang.tr("Naik ke folder induk", "Up to parent folder"))
                                .clicked()
                            {
                                nav_target_folder = Some(parent_of_cur);
                            }
                        }

                        // Segmen Root (All Documents) — juga sebagai Drop Target
                        let is_root = self.current_folder_id.is_none();
                        let root_resp = ui.selectable_label(
                            is_root,
                            RichText::new(lang.tr("📁 Semua Dokumen", "📁 All Documents"))
                                .size(12.5)
                                .color(if is_root {
                                    palette.accent_text
                                } else {
                                    palette.text_primary
                                }),
                        );
                        if dragging_any && root_resp.hovered() {
                            ui.painter().rect_stroke(
                                root_resp.rect.expand(2.0),
                                4.0,
                                Stroke::new(1.5_f32, palette.accent),
                                StrokeKind::Outside,
                            );
                            if pointer_released {
                                drop_to_folder = Some(None);
                            }
                        }
                        if root_resp.clicked() {
                            nav_target_folder = Some(None);
                        }

                        for (fid, fname) in &chain {
                            ui.label(RichText::new("/").color(palette.text_muted));
                            let is_active = self.current_folder_id.as_ref() == Some(fid);
                            let seg_resp = ui.selectable_label(
                                is_active,
                                RichText::new(format!("📁 {fname}"))
                                    .size(12.5)
                                    .color(if is_active {
                                        palette.accent_text
                                    } else {
                                        palette.text_primary
                                    }),
                            );
                            let valid_folder_drop = match &self.library_drag_payload {
                                Some(LibraryDragPayload::Notebooks(_)) => true,
                                Some(LibraryDragPayload::Folder(src_f)) => {
                                    src_f != fid
                                        && can_move_folder_into(
                                            &self.library.folders,
                                            src_f,
                                            Some(fid.as_str()),
                                        )
                                }
                                None => false,
                            };
                            if valid_folder_drop && seg_resp.hovered() {
                                ui.painter().rect_stroke(
                                    seg_resp.rect.expand(2.0),
                                    4.0,
                                    Stroke::new(1.5_f32, palette.accent),
                                    StrokeKind::Outside,
                                );
                                if pointer_released {
                                    drop_to_folder = Some(Some(fid.clone()));
                                }
                            }
                            if seg_resp.clicked() {
                                nav_target_folder = Some(Some(fid.clone()));
                            }
                        }

                        if dragging_any {
                            ui.label(
                                RichText::new(lang.tr(
                                    "  (Lepas pada folder atau jalur di atas untuk memindahkan)",
                                    "  (Drop onto any folder or path segment to move)",
                                ))
                                .size(11.0)
                                .color(palette.accent_text),
                            );
                        }
                    });
                    ui.add_space(6.0);
                }

                if let Some(target) = nav_target_folder {
                    self.current_folder_id = target;
                    self.library_selected_ids.clear();
                }

                // Filter Subfolder untuk directory aktif
                let query = self.search_query.trim().to_lowercase();
                let current_subfolders: Vec<crate::model::Folder> =
                    if self.current_nav_view == NavView::Documents && query.is_empty() {
                        let mut subs: Vec<_> = self
                            .library
                            .folders
                            .iter()
                            .filter(|f| f.parent_id == self.current_folder_id)
                            .cloned()
                            .collect();
                        subs.sort_by_key(|a| a.name.to_lowercase());
                        subs
                    } else {
                        Vec::new()
                    };

                // Kumpulkan & urutkan daftar buku catatan sesuai nav view, filter, dan pencarian
                let mut notebooks_to_show: Vec<(usize, Option<String>)> = Vec::new();
                for (i, n) in self.library.notebooks.iter().enumerate() {
                    let matches_view = match self.current_nav_view {
                        NavView::Recent => !n.trashed,
                        NavView::Documents => {
                            !n.trashed
                                && ((query.is_empty() && n.folder_id == self.current_folder_id)
                                    || !query.is_empty())
                        }
                        NavView::Favorites => !n.trashed && n.favorite,
                        NavView::Trash => n.trashed,
                    };
                    if !matches_view {
                        continue;
                    }
                    let matches_filter = match self.library_filter {
                        LibraryFilter::All => true,
                        LibraryFilter::PdfOnly => notebook_has_pdf_pages(n),
                        LibraryFilter::FavoritesOnly => n.favorite,
                    };
                    if !matches_filter {
                        continue;
                    }

                    if query.is_empty() || n.title.to_lowercase().contains(&query) {
                        notebooks_to_show.push((i, None));
                    } else {
                        let mut found_snippet = None;
                        for (p_idx, page) in n.pages.iter().enumerate() {
                            if !page.ocr_text.is_empty()
                                && page.ocr_text.to_lowercase().contains(&query)
                            {
                                let snip: String = page.ocr_text.chars().take(28).collect();
                                found_snippet = Some(format!(
                                    "✍ OCR {} {}: \"{snip}\"",
                                    lang.tr("Hal.", "p."),
                                    p_idx + 1
                                ));
                                break;
                            }
                            for obj in &page.objects {
                                match obj {
                                    crate::model::DrawObject::Text { text, .. }
                                    | crate::model::DrawObject::StickyNote { text, .. }
                                        if text.to_lowercase().contains(&query) =>
                                    {
                                        let snip: String = text.chars().take(28).collect();
                                        found_snippet = Some(format!(
                                            "{} {}: \"{snip}\"",
                                            lang.tr("Hal.", "p."),
                                            p_idx + 1
                                        ));
                                        break;
                                    }
                                    crate::model::DrawObject::Shape {
                                        label: Some(lbl), ..
                                    } if lbl.text.to_lowercase().contains(&query) => {
                                        let snip: String = lbl.text.chars().take(28).collect();
                                        found_snippet = Some(format!(
                                            "📐 {} {}: \"{snip}\"",
                                            lang.tr("Hal.", "p."),
                                            p_idx + 1
                                        ));
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
                            notebooks_to_show.push((i, Some(snippet)));
                        }
                    }
                }

                let effective_sort = if self.current_nav_view == NavView::Recent
                    && self.library_sort_by == LibrarySortBy::LastModified
                {
                    LibrarySortBy::LastModified
                } else {
                    self.library_sort_by
                };
                sort_notebook_items(&self.library.notebooks, &mut notebooks_to_show, effective_sort);

                // --- BULK ACTION TOOLBAR ---
                if self.library_select_mode || !self.library_selected_ids.is_empty() {
                    let sel_count = self.library_selected_ids.len();
                    let visible_ids: Vec<String> = notebooks_to_show
                        .iter()
                        .map(|(idx, _)| self.library.notebooks[*idx].id.clone())
                        .collect();

                    egui::Frame::NONE
                        .fill(palette.accent_soft_bg)
                        .stroke(Stroke::new(1.0_f32, palette.border_accent))
                        .corner_radius(10.0)
                        .inner_margin(egui::Margin::symmetric(12, 8))
                        .show(ui, |ui| {
                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing = Vec2::new(8.0, 4.0);
                                ui.label(
                                    RichText::new(match lang {
                                        crate::model::AppLanguage::Indonesian => {
                                            format!("☑ {sel_count} buku dipilih")
                                        }
                                        crate::model::AppLanguage::English => {
                                            format!("☑ {sel_count} selected")
                                        }
                                    })
                                    .strong()
                                    .color(palette.accent_text),
                                );

                                if ui
                                    .button(lang.tr("Pilih Semua", "Select All"))
                                    .clicked()
                                {
                                    for id in &visible_ids {
                                        self.library_selected_ids.insert(id.clone());
                                    }
                                    self.library_select_mode = true;
                                }

                                if ui
                                    .button(lang.tr("Bersihkan", "Clear"))
                                    .clicked()
                                {
                                    self.library_selected_ids.clear();
                                }

                                if sel_count > 0 {
                                    let selected_vec: Vec<String> =
                                        self.library_selected_ids.iter().cloned().collect();
                                    if self.current_nav_view == NavView::Trash {
                                        if ui
                                            .button(lang.tr("↩ Pulihkan", "↩ Restore"))
                                            .clicked()
                                        {
                                            let mut restored = 0usize;
                                            for nb in &mut self.library.notebooks {
                                                if selected_vec.contains(&nb.id) && nb.trashed {
                                                    nb.trashed = false;
                                                    restored += 1;
                                                }
                                            }
                                            if restored > 0 {
                                                self.persist_library();
                                                self.library_selected_ids.clear();
                                                self.library_select_mode = false;
                                                self.toast_message = Some(match lang {
                                                    crate::model::AppLanguage::Indonesian => {
                                                        format!("↩ {restored} buku dipulihkan.")
                                                    }
                                                    crate::model::AppLanguage::English => {
                                                        format!("↩ {restored} notebook(s) restored.")
                                                    }
                                                });
                                            }
                                        }
                                        if ui
                                            .add(
                                                Button::new(
                                                    RichText::new(lang.tr(
                                                        "❌ Hapus Permanen",
                                                        "❌ Delete Forever",
                                                    ))
                                                    .color(Color32::WHITE),
                                                )
                                                .fill(palette.danger_soft_bg),
                                            )
                                            .clicked()
                                        {
                                            self.confirm_bulk_delete_ids = selected_vec;
                                        }
                                    } else {
                                        if ui
                                            .button(lang.tr("📁 Pindah...", "📁 Move..."))
                                            .clicked()
                                        {
                                            self.bulk_moving_notebook_ids = selected_vec.clone();
                                        }
                                        if ui
                                            .button(lang.tr("★ Favorit", "★ Favorite"))
                                            .clicked()
                                        {
                                            let all_fav = self
                                                .library
                                                .notebooks
                                                .iter()
                                                .filter(|n| selected_vec.contains(&n.id))
                                                .all(|n| n.favorite);
                                            for nb in &mut self.library.notebooks {
                                                if selected_vec.contains(&nb.id) {
                                                    nb.favorite = !all_fav;
                                                }
                                            }
                                            self.persist_library();
                                        }
                                        if ui
                                            .button(lang.tr("📄 Duplikasi", "📄 Duplicate"))
                                            .clicked()
                                        {
                                            let storage_dir = app_storage_dir();
                                            let mut clones = Vec::new();
                                            for orig_id in &selected_vec {
                                                if self.dirty_notebooks.contains(orig_id) {
                                                    self.force_save_notebook(orig_id);
                                                }
                                                if let Some(orig) = self
                                                    .library
                                                    .notebooks
                                                    .iter()
                                                    .find(|n| &n.id == orig_id)
                                                {
                                                    let mut clone = orig.clone();
                                                    clone.id = uuid::Uuid::new_v4().to_string();
                                                    clone.title = format!(
                                                        "{} {}",
                                                        clone.title,
                                                        lang.tr("salinan", "copy")
                                                    );
                                                    if crate::storage::duplicate_notebook_storage(
                                                        &storage_dir,
                                                        orig_id,
                                                        &clone,
                                                    )
                                                    .is_ok()
                                                    {
                                                        clones.push(clone);
                                                    }
                                                }
                                            }
                                            let dup_cnt = clones.len();
                                            self.library.notebooks.extend(clones);
                                            if dup_cnt > 0 {
                                                self.sync_library_index();
                                                self.toast_message = Some(match lang {
                                                    crate::model::AppLanguage::Indonesian => {
                                                        format!("📄 {dup_cnt} buku diduplikasi.")
                                                    }
                                                    crate::model::AppLanguage::English => {
                                                        format!("📄 {dup_cnt} notebook(s) duplicated.")
                                                    }
                                                });
                                            }
                                        }
                                        if ui
                                            .button(lang.tr("🗑 Buang", "🗑 Trash"))
                                            .clicked()
                                        {
                                            let mut moved = 0usize;
                                            for nb in &mut self.library.notebooks {
                                                if selected_vec.contains(&nb.id) && !nb.trashed {
                                                    nb.trashed = true;
                                                    moved += 1;
                                                }
                                            }
                                            if moved > 0 {
                                                self.library_selected_ids.clear();
                                                self.library_select_mode = false;
                                                self.persist_library();
                                                self.toast_message = Some(match lang {
                                                    crate::model::AppLanguage::Indonesian => {
                                                        format!("🗑 {moved} buku dipindahkan ke Sampah.")
                                                    }
                                                    crate::model::AppLanguage::English => {
                                                        format!("🗑 Moved {moved} notebook(s) to Trash.")
                                                    }
                                                });
                                            }
                                        }
                                    }
                                }

                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if ui.small_button(lang.tr("✕ Tutup", "✕ Close")).clicked() {
                                        self.library_select_mode = false;
                                        self.library_selected_ids.clear();
                                    }
                                });
                            });
                        });
                    ui.add_space(10.0);
                }

                // --- AREA KONTEN TERPADU DALAM SATU VERTICAL SCROLL AREA (ANTI-OVERLAP) ---
                let mut action_open_folder: Option<String> = None;
                let mut action_rename_folder: Option<(String, String, Color32)> = None;
                let mut action_delete_folder: Option<String> = None;

                let mut to_open: Option<String> = None;
                let mut to_toggle_select: Option<String> = None;
                let mut to_toggle_fav: Option<usize> = None;
                let mut to_rename_nb: Option<(String, String, Color32)> = None;
                let mut to_trash: Option<usize> = None;
                let mut to_restore: Option<usize> = None;
                let mut to_delete_perm: Option<(String, String)> = None;
                let mut to_duplicate: Option<usize> = None;
                let mut action_move_nb: Option<String> = None;
                let mut to_export_zip: Option<(String, String)> = None;
                let mut start_drag: Option<LibraryDragPayload> = None;

                let now_ms = chrono::Utc::now().timestamp_millis();
                let show_folder_badge = self.current_nav_view == NavView::Recent
                    || self.current_nav_view == NavView::Favorites
                    || !query.is_empty();

                ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let avail_w = ui.available_width().max(120.0);

                        // 1. Subfolders Grid (Responsif per baris, juga Drop Target untuk Drag & Drop!)
                        if !current_subfolders.is_empty() {
                            ui.label(
                                RichText::new(lang.tr("Folder", "Folders"))
                                    .size(12.0)
                                    .strong()
                                    .color(palette.text_muted),
                            );
                            ui.add_space(6.0);

                            let folder_gap = 12.0;
                            let (f_cols, f_card_w) = compute_folder_grid(avail_w, 0.0, folder_gap);
                            let f_card_h = 68.0;

                            for row in current_subfolders.chunks(f_cols) {
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = folder_gap;
                                    for folder in row {
                                        let item_cnt = self
                                            .library
                                            .notebooks
                                            .iter()
                                            .filter(|n| {
                                                !n.trashed
                                                    && n.folder_id.as_deref() == Some(&folder.id)
                                            })
                                            .count();
                                        let sub_cnt = self
                                            .library
                                            .folders
                                            .iter()
                                            .filter(|f| f.parent_id.as_deref() == Some(&folder.id))
                                            .count();

                                        let (rect, resp) = ui.allocate_exact_size(
                                            Vec2::new(f_card_w, f_card_h),
                                            Sense::click_and_drag(),
                                        );
                                        let can_accept_drop = match &self.library_drag_payload {
                                            Some(LibraryDragPayload::Notebooks(_)) => true,
                                            Some(LibraryDragPayload::Folder(src_fid)) => {
                                                src_fid != &folder.id
                                                    && can_move_folder_into(
                                                        &self.library.folders,
                                                        src_fid,
                                                        Some(&folder.id),
                                                    )
                                            }
                                            None => false,
                                        };
                                        let is_drop_target = can_accept_drop && resp.hovered();

                                        if resp.drag_started() {
                                            start_drag =
                                                Some(LibraryDragPayload::Folder(folder.id.clone()));
                                        }
                                        if is_drop_target && pointer_released {
                                            drop_to_folder = Some(Some(folder.id.clone()));
                                        }

                                        let painter = ui.painter_at(rect);
                                        let bg_col = if is_drop_target {
                                            palette.bg_card_selected
                                        } else if resp.hovered() {
                                            palette.bg_card_hover
                                        } else {
                                            palette.bg_card
                                        };
                                        let border_col = if is_drop_target {
                                            palette.accent
                                        } else if resp.hovered() {
                                            palette.border_strong
                                        } else {
                                            palette.border_subtle
                                        };
                                        painter.rect_filled(rect, 8.0, bg_col);
                                        painter.rect_stroke(
                                            rect,
                                            8.0,
                                            Stroke::new(
                                                if is_drop_target { 2.0 } else { 1.0_f32 },
                                                border_col,
                                            ),
                                            StrokeKind::Inside,
                                        );
                                        painter.rect_filled(
                                            Rect::from_min_size(
                                                rect.min,
                                                Vec2::new(rect.width(), 4.0),
                                            ),
                                            CornerRadius {
                                                nw: 8,
                                                ne: 8,
                                                se: 0,
                                                sw: 0,
                                            },
                                            folder.color.to_color32(),
                                        );

                                        painter.text(
                                            rect.left_top() + Vec2::new(10.0, 15.0),
                                            egui::Align2::LEFT_TOP,
                                            "📁",
                                            FontId::proportional(20.0),
                                            palette.text_primary,
                                        );

                                        let name_font = FontId::proportional(13.5);
                                        let fitted_name = fit_title_to_width(
                                            &painter,
                                            &folder.name,
                                            &name_font,
                                            palette.text_primary,
                                            (f_card_w - 52.0).max(40.0),
                                        );
                                        painter.text(
                                            rect.left_top() + Vec2::new(38.0, 14.0),
                                            egui::Align2::LEFT_TOP,
                                            fitted_name,
                                            name_font,
                                            palette.text_primary,
                                        );

                                        let sub_label = if is_drop_target {
                                            lang.tr("Lepas di sini", "Drop here").to_string()
                                        } else if sub_cnt > 0 {
                                            match lang {
                                                crate::model::AppLanguage::Indonesian => {
                                                    format!("{item_cnt} buku · {sub_cnt} folder")
                                                }
                                                crate::model::AppLanguage::English => {
                                                    format!("{item_cnt} notes · {sub_cnt} folders")
                                                }
                                            }
                                        } else {
                                            match lang {
                                                crate::model::AppLanguage::Indonesian => {
                                                    format!("{item_cnt} buku")
                                                }
                                                crate::model::AppLanguage::English => {
                                                    format!("{item_cnt} items")
                                                }
                                            }
                                        };
                                        painter.text(
                                            rect.left_top() + Vec2::new(38.0, 38.0),
                                            egui::Align2::LEFT_TOP,
                                            sub_label,
                                            FontId::proportional(11.0),
                                            if is_drop_target {
                                                palette.accent_text
                                            } else {
                                                palette.text_muted
                                            },
                                        );

                                        if resp.clicked() && !dragging_any {
                                            action_open_folder = Some(folder.id.clone());
                                        }
                                        resp.context_menu(|ui| {
                                            if ui
                                                .button(lang.tr("📂 Buka Folder", "📂 Open Folder"))
                                                .clicked()
                                            {
                                                action_open_folder = Some(folder.id.clone());
                                                ui.close_menu();
                                            }
                                            if ui
                                                .button(lang.tr(
                                                    "✏ Ubah Nama & Warna...",
                                                    "✏ Rename & Color...",
                                                ))
                                                .clicked()
                                            {
                                                action_rename_folder = Some((
                                                    folder.id.clone(),
                                                    folder.name.clone(),
                                                    folder.color.to_color32(),
                                                ));
                                                ui.close_menu();
                                            }
                                            ui.separator();
                                            if ui
                                                .button(
                                                    RichText::new(lang.tr(
                                                        "🗑 Hapus Folder",
                                                        "🗑 Delete Folder",
                                                    ))
                                                    .color(palette.danger),
                                                )
                                                .clicked()
                                            {
                                                action_delete_folder = Some(folder.id.clone());
                                                ui.close_menu();
                                            }
                                        });
                                    }
                                });
                                ui.add_space(10.0);
                            }
                            ui.add_space(8.0);
                        }

                        // 2. Empty state when no notebooks and (if Documents) no subfolders or search is empty
                        if notebooks_to_show.is_empty() {
                            if current_subfolders.is_empty() {
                                ui.vertical_centered(|ui| {
                                    ui.add_space(64.0);
                                    ui.label(RichText::new("📖").size(52.0));
                                    ui.add_space(10.0);
                                    ui.heading(
                                        RichText::new(if !query.is_empty() {
                                            lang.tr(
                                                "Tidak ditemukan hasil cocok",
                                                "No matching notes found",
                                            )
                                        } else if self.current_nav_view == NavView::Trash {
                                            lang.tr("Sampah kosong", "Trash is empty")
                                        } else if self.current_nav_view == NavView::Favorites {
                                            lang.tr(
                                                "Belum ada buku favorit",
                                                "No favorite notebooks yet",
                                            )
                                        } else {
                                            lang.tr(
                                                "Belum ada buku catatan",
                                                "No notebooks here yet",
                                            )
                                        })
                                        .size(20.0)
                                        .color(palette.text_primary),
                                    );
                                    ui.label(
                                        RichText::new(lang.tr(
                                            "Buat buku catatan baru atau seret file PDF ke jendela ini.",
                                            "Create a new notebook or drop a PDF file into this window.",
                                        ))
                                        .size(13.0)
                                        .color(palette.text_muted),
                                    );
                                    if self.current_nav_view != NavView::Trash && query.is_empty() {
                                        ui.add_space(14.0);
                                        let btn = Button::new(
                                            RichText::new(lang.tr(
                                                "+  Buat Buku Catatan",
                                                "+  Create Notebook",
                                            ))
                                            .size(13.5)
                                            .strong()
                                            .color(Color32::WHITE),
                                        )
                                        .fill(palette.accent)
                                        .corner_radius(18.0)
                                        .min_size(Vec2::new(158.0, 36.0));
                                        if ui.add(btn).clicked() {
                                            self.show_new_notebook_modal = true;
                                            self.new_notebook_title.clear();
                                        }
                                    }
                                });
                            }
                            return;
                        }

                        if !current_subfolders.is_empty() {
                            ui.label(
                                RichText::new(lang.tr("Buku Catatan", "Notebooks"))
                                    .size(12.0)
                                    .strong()
                                    .color(palette.text_muted),
                            );
                            ui.add_space(6.0);
                        }

                        let ctrl_held =
                            ctx.input(|i| i.modifiers.command || i.modifiers.ctrl);

                        match self.library_view_style {
                            LibraryViewStyle::Grid => {
                                let gap_x = 16.0;
                                let metrics = compute_responsive_grid(
                                    avail_w, 0.0, gap_x, 118.0, 134.0, 162.0, 1.29,
                                );

                                for row in notebooks_to_show.chunks(metrics.cols) {
                                    ui.horizontal_top(|ui| {
                                        ui.spacing_mut().item_spacing.x = gap_x;
                                        for (orig_idx, snippet) in row {
                                            let note = &self.library.notebooks[*orig_idx];
                                            let nb_id = note.id.clone();
                                            let is_selected =
                                                self.library_selected_ids.contains(&nb_id);
                                            let is_being_dragged =
                                                match &self.library_drag_payload {
                                                    Some(LibraryDragPayload::Notebooks(ids)) => {
                                                        ids.contains(&nb_id)
                                                    }
                                                    _ => false,
                                                };

                                            ui.vertical(|ui| {
                                                ui.set_width(metrics.card_width);

                                                let (rect, resp) = ui.allocate_exact_size(
                                                    Vec2::new(
                                                        metrics.card_width,
                                                        metrics.card_height,
                                                    ),
                                                    Sense::click_and_drag(),
                                                );

                                                // Drag initiation
                                                if resp.drag_started() && !note.trashed {
                                                    let ids = if is_selected
                                                        && !self.library_selected_ids.is_empty()
                                                    {
                                                        self.library_selected_ids
                                                            .iter()
                                                            .cloned()
                                                            .collect()
                                                    } else {
                                                        vec![nb_id.clone()]
                                                    };
                                                    start_drag =
                                                        Some(LibraryDragPayload::Notebooks(ids));
                                                }

                                                let painter = ui.painter_at(rect);
                                                draw_goodnotes_cover(
                                                    &painter,
                                                    rect,
                                                    &note.title,
                                                    note.cover_color.to_color32(),
                                                    note.favorite,
                                                );

                                                if is_being_dragged {
                                                    painter.rect_filled(
                                                        rect,
                                                        0.0,
                                                        Color32::from_rgba_unmultiplied(
                                                            0, 0, 0, 90,
                                                        ),
                                                    );
                                                }

                                                // Outline border for selection / hover
                                                if is_selected {
                                                    painter.rect_stroke(
                                                        rect,
                                                        CornerRadius {
                                                            nw: 0,
                                                            ne: 12,
                                                            se: 12,
                                                            sw: 0,
                                                        },
                                                        Stroke::new(2.5_f32, palette.accent),
                                                        StrokeKind::Inside,
                                                    );
                                                } else if resp.hovered() {
                                                    painter.rect_stroke(
                                                        rect,
                                                        CornerRadius {
                                                            nw: 0,
                                                            ne: 12,
                                                            se: 12,
                                                            sw: 0,
                                                        },
                                                        Stroke::new(2.0_f32, palette.border_accent),
                                                        StrokeKind::Inside,
                                                    );
                                                }

                                                // Selection Checkbox Badge (top-left on select mode or selected)
                                                if self.library_select_mode || is_selected {
                                                    let cb_rect = Rect::from_min_size(
                                                        rect.left_top() + Vec2::new(14.0, 8.0),
                                                        Vec2::new(20.0, 20.0),
                                                    );
                                                    painter.rect_filled(
                                                        cb_rect,
                                                        5.0,
                                                        if is_selected {
                                                            palette.accent
                                                        } else {
                                                            Color32::from_rgba_unmultiplied(
                                                                20, 24, 32, 180,
                                                            )
                                                        },
                                                    );
                                                    painter.rect_stroke(
                                                        cb_rect,
                                                        5.0,
                                                        Stroke::new(1.2_f32, Color32::WHITE),
                                                        StrokeKind::Outside,
                                                    );
                                                    if is_selected {
                                                        painter.text(
                                                            cb_rect.center(),
                                                            egui::Align2::CENTER_CENTER,
                                                            "✓",
                                                            FontId::proportional(13.0),
                                                            Color32::WHITE,
                                                        );
                                                    }
                                                }

                                                // Hover quick star button (bottom-left of cover, away from spine)
                                                let star_rect = Rect::from_min_size(
                                                    rect.left_bottom() + Vec2::new(14.0, -28.0),
                                                    Vec2::new(22.0, 22.0),
                                                );
                                                let ptr_pos = ctx.input(|i| i.pointer.hover_pos());
                                                let card_hovered = resp.hovered()
                                                    || ptr_pos.is_some_and(|p| rect.contains(p));
                                                let star_hovered =
                                                    ptr_pos.is_some_and(|p| star_rect.contains(p));
                                                let mut handled_overlay_click = false;

                                                if card_hovered && !note.trashed && !self.library_select_mode {
                                                    painter.rect_filled(
                                                        star_rect,
                                                        6.0,
                                                        Color32::from_rgba_unmultiplied(
                                                            20, 24, 30, 175,
                                                        ),
                                                    );
                                                    painter.text(
                                                        star_rect.center(),
                                                        egui::Align2::CENTER_CENTER,
                                                        if note.favorite { "★" } else { "☆" },
                                                        FontId::proportional(13.0),
                                                        if note.favorite {
                                                            Color32::from_rgb(255, 204, 0)
                                                        } else {
                                                            Color32::WHITE
                                                        },
                                                    );
                                                    if resp.clicked() && star_hovered {
                                                        to_toggle_fav = Some(*orig_idx);
                                                        handled_overlay_click = true;
                                                    }
                                                }

                                                if !handled_overlay_click && resp.clicked() {
                                                    if self.library_select_mode || ctrl_held {
                                                        to_toggle_select = Some(nb_id.clone());
                                                    } else if !note.trashed {
                                                        to_open = Some(nb_id.clone());
                                                    }
                                                }

                                                resp.context_menu(|ui| {
                                                    if note.trashed {
                                                        if ui
                                                            .button(lang.tr(
                                                                "↩ Pulihkan Buku",
                                                                "↩ Restore Notebook",
                                                            ))
                                                            .clicked()
                                                        {
                                                            to_restore = Some(*orig_idx);
                                                            ui.close_menu();
                                                        }
                                                        if ui
                                                            .button(
                                                                RichText::new(lang.tr(
                                                                    "❌ Hapus Permanen",
                                                                    "❌ Delete Permanently",
                                                                ))
                                                                .color(palette.danger),
                                                            )
                                                            .clicked()
                                                        {
                                                            to_delete_perm = Some((
                                                                note.id.clone(),
                                                                note.title.clone(),
                                                            ));
                                                            ui.close_menu();
                                                        }
                                                    } else {
                                                        if ui
                                                            .button(lang.tr("📖 Buka", "📖 Open"))
                                                            .clicked()
                                                        {
                                                            to_open = Some(note.id.clone());
                                                            ui.close_menu();
                                                        }
                                                        if ui
                                                            .button(lang.tr(
                                                                "✏ Ubah Nama & Sampul...",
                                                                "✏ Rename & Cover...",
                                                            ))
                                                            .clicked()
                                                        {
                                                            to_rename_nb = Some((
                                                                note.id.clone(),
                                                                note.title.clone(),
                                                                note.cover_color.to_color32(),
                                                            ));
                                                            ui.close_menu();
                                                        }
                                                        let fav_text = if note.favorite {
                                                            lang.tr(
                                                                "★ Hapus dari Favorit",
                                                                "★ Remove Favorite",
                                                            )
                                                        } else {
                                                            lang.tr(
                                                                "☆ Tambah ke Favorit",
                                                                "☆ Add to Favorites",
                                                            )
                                                        };
                                                        if ui.button(fav_text).clicked() {
                                                            to_toggle_fav = Some(*orig_idx);
                                                            ui.close_menu();
                                                        }
                                                        if ui
                                                            .button(lang.tr(
                                                                "📄 Duplikasi",
                                                                "📄 Duplicate",
                                                            ))
                                                            .clicked()
                                                        {
                                                            to_duplicate = Some(*orig_idx);
                                                            ui.close_menu();
                                                        }
                                                        ui.separator();
                                                        if ui
                                                            .button(lang.tr(
                                                                "📁 Pindah ke Folder...",
                                                                "📁 Move to Folder...",
                                                            ))
                                                            .clicked()
                                                        {
                                                            action_move_nb = Some(note.id.clone());
                                                            ui.close_menu();
                                                        }
                                                        ui.separator();
                                                        if ui
                                                            .button(lang.tr(
                                                                "📦 Ekspor sebagai .rnote...",
                                                                "📦 Export as .rnote...",
                                                            ))
                                                            .clicked()
                                                        {
                                                            to_export_zip = Some((
                                                                note.id.clone(),
                                                                note.title.clone(),
                                                            ));
                                                            ui.close_menu();
                                                        }
                                                        ui.separator();
                                                        if ui
                                                            .button(
                                                                RichText::new(lang.tr(
                                                                    "🗑 Pindah ke Sampah",
                                                                    "🗑 Move to Trash",
                                                                ))
                                                                .color(palette.danger),
                                                            )
                                                            .clicked()
                                                        {
                                                            to_trash = Some(*orig_idx);
                                                            ui.close_menu();
                                                        }
                                                    }
                                                });

                                                ui.add_space(6.0);
                                                let title_font = FontId::proportional(13.5);
                                                let card_title = fit_title_to_width(
                                                    ui.painter(),
                                                    &note.title,
                                                    &title_font,
                                                    palette.text_primary,
                                                    metrics.card_width,
                                                );
                                                ui.label(
                                                    RichText::new(card_title)
                                                        .strong()
                                                        .size(13.5)
                                                        .color(palette.text_primary),
                                                );

                                                let page_str = if note.pages.len() == 1 {
                                                    lang.tr("hal.", "page")
                                                } else {
                                                    lang.tr("hal.", "pages")
                                                };
                                                let rel_time = format_relative_time(
                                                    note.updated_at,
                                                    now_ms,
                                                    lang,
                                                );
                                                ui.label(
                                                    RichText::new(format!(
                                                        "{} {} · {}",
                                                        note.pages.len(),
                                                        page_str,
                                                        rel_time
                                                    ))
                                                    .size(11.0)
                                                    .color(palette.text_muted),
                                                );
                                                if show_folder_badge && note.folder_id.is_some() {
                                                    let path_txt = folder_path_label(
                                                        &self.library.folders,
                                                        note.folder_id.as_deref(),
                                                        lang.tr("Dokumen", "Docs"),
                                                    );
                                                    let path_font = FontId::proportional(10.0);
                                                    let fitted_path = fit_title_to_width(
                                                        ui.painter(),
                                                        &path_txt,
                                                        &path_font,
                                                        palette.text_secondary,
                                                        metrics.card_width - 14.0,
                                                    );
                                                    ui.label(
                                                        RichText::new(format!("📁 {fitted_path}"))
                                                            .size(10.0)
                                                            .color(palette.text_secondary),
                                                    );
                                                }
                                                if let Some(snip) = snippet {
                                                    ui.label(
                                                        RichText::new(format!("🔍 {snip}"))
                                                            .size(10.0)
                                                            .color(palette.accent_text),
                                                    );
                                                }
                                                ui.add_space(10.0);
                                            });
                                        }
                                    });
                                    ui.add_space(10.0);
                                }
                            }
                            LibraryViewStyle::List => {
                                let row_w = avail_w;
                                let show_meta_cols = row_w >= 540.0;
                                for (orig_idx, snippet) in &notebooks_to_show {
                                    let note = &self.library.notebooks[*orig_idx];
                                    let nb_id = note.id.clone();
                                    let is_selected = self.library_selected_ids.contains(&nb_id);
                                    let row_h = if snippet.is_some() { 58.0 } else { 48.0 };

                                    let (rect, resp) = ui.allocate_exact_size(
                                        Vec2::new(row_w, row_h),
                                        Sense::click_and_drag(),
                                    );
                                    if resp.drag_started() && !note.trashed {
                                        let ids = if is_selected
                                            && !self.library_selected_ids.is_empty()
                                        {
                                            self.library_selected_ids.iter().cloned().collect()
                                        } else {
                                            vec![nb_id.clone()]
                                        };
                                        start_drag = Some(LibraryDragPayload::Notebooks(ids));
                                    }

                                    let painter = ui.painter_at(rect);
                                    let row_bg = if is_selected {
                                        palette.bg_card_selected
                                    } else if resp.hovered() {
                                        palette.bg_card_hover
                                    } else {
                                        palette.bg_card
                                    };
                                    painter.rect_filled(rect, 8.0, row_bg);
                                    painter.rect_stroke(
                                        rect,
                                        8.0,
                                        Stroke::new(
                                            if is_selected { 1.5 } else { 1.0_f32 },
                                            if is_selected {
                                                palette.border_accent
                                            } else {
                                                palette.border_subtle
                                            },
                                        ),
                                        StrokeKind::Inside,
                                    );
                                    // Cover color strip at left edge
                                    painter.rect_filled(
                                        Rect::from_min_size(rect.min, Vec2::new(6.0, row_h)),
                                        CornerRadius {
                                            nw: 8,
                                            ne: 0,
                                            se: 0,
                                            sw: 8,
                                        },
                                        note.cover_color.to_color32(),
                                    );

                                    let mut left_x = rect.left() + 16.0;
                                    if self.library_select_mode || is_selected {
                                        let cb = Rect::from_center_size(
                                            Pos2::new(left_x + 8.0, rect.center().y),
                                            Vec2::new(16.0, 16.0),
                                        );
                                        painter.rect_filled(
                                            cb,
                                            4.0,
                                            if is_selected {
                                                palette.accent
                                            } else {
                                                palette.btn_neutral_bg
                                            },
                                        );
                                        painter.rect_stroke(
                                            cb,
                                            4.0,
                                            Stroke::new(1.0_f32, palette.border_strong),
                                            StrokeKind::Outside,
                                        );
                                        if is_selected {
                                            painter.text(
                                                cb.center(),
                                                egui::Align2::CENTER_CENTER,
                                                "✓",
                                                FontId::proportional(11.0),
                                                Color32::WHITE,
                                            );
                                        }
                                        left_x += 26.0;
                                    }

                                    let max_title_w = if show_meta_cols {
                                        (row_w - 280.0).max(120.0)
                                    } else {
                                        (row_w - 90.0).max(80.0)
                                    };
                                    let title_font = FontId::proportional(14.0);
                                    let fitted_title = fit_title_to_width(
                                        &painter,
                                        &note.title,
                                        &title_font,
                                        palette.text_primary,
                                        max_title_w,
                                    );
                                    let title_y = if snippet.is_some() {
                                        rect.top() + 10.0
                                    } else {
                                        rect.center().y - 7.0
                                    };
                                    painter.text(
                                        Pos2::new(left_x, title_y),
                                        egui::Align2::LEFT_TOP,
                                        fitted_title,
                                        title_font,
                                        palette.text_primary,
                                    );
                                    if let Some(snip) = snippet {
                                        painter.text(
                                            Pos2::new(left_x, rect.top() + 31.0),
                                            egui::Align2::LEFT_TOP,
                                            format!("🔍 {snip}"),
                                            FontId::proportional(11.0),
                                            palette.accent_text,
                                        );
                                    }

                                    // Star button + metadata on right
                                    let star_rect = Rect::from_center_size(
                                        rect.right_center() - Vec2::new(22.0, 0.0),
                                        Vec2::new(24.0, 24.0),
                                    );
                                    let ptr_pos = ctx.input(|i| i.pointer.hover_pos());
                                    let star_hovered =
                                        ptr_pos.is_some_and(|p| star_rect.contains(p));
                                    painter.text(
                                        star_rect.center(),
                                        egui::Align2::CENTER_CENTER,
                                        if note.favorite { "★" } else { "☆" },
                                        FontId::proportional(15.0),
                                        if note.favorite {
                                            Color32::from_rgb(255, 204, 0)
                                        } else {
                                            palette.text_muted
                                        },
                                    );

                                    if show_meta_cols {
                                        let page_str = if note.pages.len() == 1 {
                                            lang.tr("hal.", "page")
                                        } else {
                                            lang.tr("hal.", "pages")
                                        };
                                        let rel =
                                            format_relative_time(note.updated_at, now_ms, lang);
                                        let folder_txt = if note.folder_id.is_some() {
                                            format!(
                                                "📁 {} · ",
                                                folder_path_label(
                                                    &self.library.folders,
                                                    note.folder_id.as_deref(),
                                                    lang.tr("Dokumen", "Docs"),
                                                )
                                            )
                                        } else {
                                            String::new()
                                        };
                                        painter.text(
                                            rect.right_center() - Vec2::new(48.0, 0.0),
                                            egui::Align2::RIGHT_CENTER,
                                            format!(
                                                "{folder_txt}{} {page_str} · {rel}",
                                                note.pages.len()
                                            ),
                                            FontId::proportional(11.5),
                                            palette.text_muted,
                                        );
                                    }

                                    if resp.clicked() {
                                        if star_hovered && !note.trashed {
                                            to_toggle_fav = Some(*orig_idx);
                                        } else if self.library_select_mode || ctrl_held {
                                            to_toggle_select = Some(nb_id.clone());
                                        } else if !note.trashed {
                                            to_open = Some(nb_id.clone());
                                        }
                                    }

                                    resp.context_menu(|ui| {
                                        if note.trashed {
                                            if ui
                                                .button(lang.tr(
                                                    "↩ Pulihkan Buku",
                                                    "↩ Restore Notebook",
                                                ))
                                                .clicked()
                                            {
                                                to_restore = Some(*orig_idx);
                                                ui.close_menu();
                                            }
                                            if ui
                                                .button(
                                                    RichText::new(lang.tr(
                                                        "❌ Hapus Permanen",
                                                        "❌ Delete Permanently",
                                                    ))
                                                    .color(palette.danger),
                                                )
                                                .clicked()
                                            {
                                                to_delete_perm =
                                                    Some((note.id.clone(), note.title.clone()));
                                                ui.close_menu();
                                            }
                                        } else {
                                            if ui
                                                .button(lang.tr("📖 Buka", "📖 Open"))
                                                .clicked()
                                            {
                                                to_open = Some(note.id.clone());
                                                ui.close_menu();
                                            }
                                            if ui
                                                .button(lang.tr(
                                                    "✏ Ubah Nama & Sampul...",
                                                    "✏ Rename & Cover...",
                                                ))
                                                .clicked()
                                            {
                                                to_rename_nb = Some((
                                                    note.id.clone(),
                                                    note.title.clone(),
                                                    note.cover_color.to_color32(),
                                                ));
                                                ui.close_menu();
                                            }
                                            if ui
                                                .button(lang.tr(
                                                    "📁 Pindah ke Folder...",
                                                    "📁 Move to Folder...",
                                                ))
                                                .clicked()
                                            {
                                                action_move_nb = Some(note.id.clone());
                                                ui.close_menu();
                                            }
                                            if ui
                                                .button(lang.tr(
                                                    "📄 Duplikasi",
                                                    "📄 Duplicate",
                                                ))
                                                .clicked()
                                            {
                                                to_duplicate = Some(*orig_idx);
                                                ui.close_menu();
                                            }
                                            if ui
                                                .button(lang.tr(
                                                    "📦 Ekspor sebagai .rnote...",
                                                    "📦 Export as .rnote...",
                                                ))
                                                .clicked()
                                            {
                                                to_export_zip =
                                                    Some((note.id.clone(), note.title.clone()));
                                                ui.close_menu();
                                            }
                                            ui.separator();
                                            if ui
                                                .button(
                                                    RichText::new(lang.tr(
                                                        "🗑 Pindah ke Sampah",
                                                        "🗑 Move to Trash",
                                                    ))
                                                    .color(palette.danger),
                                                )
                                                .clicked()
                                            {
                                                to_trash = Some(*orig_idx);
                                                ui.close_menu();
                                            }
                                        }
                                    });
                                    ui.add_space(6.0);
                                }
                            }
                        }
                    });

                // --- PROSES MUTASI TERTUNDA (DEFERRED ACTIONS & DRAG DROP) ---
                if let Some(drag) = start_drag {
                    self.library_drag_payload = Some(drag);
                }

                if let Some(target_folder_opt) = drop_to_folder
                    && let Some(payload) = self.library_drag_payload.take()
                {
                    match payload {
                        LibraryDragPayload::Notebooks(ids) => {
                            let moved =
                                self.move_notebooks_to_folder(&ids, target_folder_opt.as_deref());
                            if moved > 0 {
                                self.toast_message = Some(match lang {
                                    crate::model::AppLanguage::Indonesian => {
                                        format!("📁 {moved} buku catatan dipindahkan.")
                                    }
                                    crate::model::AppLanguage::English => {
                                        format!("📁 Moved {moved} notebook(s).")
                                    }
                                });
                            }
                        }
                        LibraryDragPayload::Folder(fid) => {
                            if self.move_folder_to_parent(&fid, target_folder_opt.as_deref()) {
                                self.toast_message = Some(
                                    lang.tr(
                                        "📁 Folder berhasil dipindahkan.",
                                        "📁 Folder moved.",
                                    )
                                    .to_string(),
                                );
                            }
                        }
                    }
                }

                if let Some(fid) = action_open_folder {
                    self.current_folder_id = Some(fid);
                    self.library_selected_ids.clear();
                }
                if let Some((fid, name, col)) = action_rename_folder {
                    self.renaming_folder_id = Some(fid);
                    self.rename_folder_name = name;
                    self.rename_folder_color = col;
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

                if let Some(id) = to_toggle_select {
                    self.library_select_mode = true;
                    if self.library_selected_ids.contains(&id) {
                        self.library_selected_ids.remove(&id);
                    } else {
                        self.library_selected_ids.insert(id);
                    }
                }
                if let Some((id, title, col)) = to_rename_nb {
                    self.renaming_notebook_id = Some(id);
                    self.rename_notebook_title = title;
                    self.rename_notebook_cover = col;
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
                    self.library.notebooks[idx].favorite = !self.library.notebooks[idx].favorite;
                    self.persist_library();
                }
                if let Some(idx) = to_duplicate {
                    let orig_id = self.library.notebooks[idx].id.clone();
                    let storage_dir = app_storage_dir();
                    if self.dirty_notebooks.contains(&orig_id) {
                        self.force_save_notebook(&orig_id);
                    }
                    let mut clone = self.library.notebooks[idx].clone();
                    clone.id = uuid::Uuid::new_v4().to_string();
                    clone.title = format!("{} {}", clone.title, lang.tr("salinan", "copy"));

                    match crate::storage::duplicate_notebook_storage(&storage_dir, &orig_id, &clone)
                    {
                        Ok(()) => {
                            let title = clone.title.clone();
                            self.library.notebooks.push(clone);
                            self.sync_library_index();
                            self.toast_message = Some(format!(
                                "{} {title}",
                                lang.tr(
                                    "📄 Buku catatan diduplikasi:",
                                    "📄 Notebook duplicated:"
                                )
                            ));
                        }
                        Err(e) => {
                            self.toast_message = Some(format!(
                                "{}: {e}",
                                lang.tr("⚠ Gagal menduplikasi buku", "⚠ Failed to duplicate")
                            ));
                        }
                    }
                }
                if let Some(idx) = to_trash {
                    let removed_id = self.library.notebooks[idx].id.clone();
                    self.library.notebooks[idx].trashed = true;
                    self.library_selected_ids.remove(&removed_id);
                    self.persist_library();
                }
                if let Some(idx) = to_restore {
                    self.library.notebooks[idx].trashed = false;
                    self.persist_library();
                }
                if let Some(target) = to_delete_perm {
                    self.confirm_delete_notebook = Some(target);
                }
                if let Some((nb_id, nb_title)) = to_export_zip {
                    let storage_dir = app_storage_dir();
                    if self.dirty_notebooks.contains(&nb_id) {
                        self.force_save_notebook(&nb_id);
                    }
                    let safe_title =
                        nb_title.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");
                    let default_name = format!("{safe_title}.rnote");
                    if let Some(path) = rfd::FileDialog::new()
                        .set_file_name(&default_name)
                        .add_filter("RustNotes Document (*.rnote)", &["rnote"])
                        .add_filter("ZIP Archive (*.zip)", &["zip"])
                        .save_file()
                    {
                        match export_notebook_to_rnote(&storage_dir, &nb_id, &path) {
                            Ok(()) => {
                                self.toast_message = Some(format!(
                                    "✅ {} '{nb_title}' {}",
                                    lang.tr("Buku", "Notebook"),
                                    lang.tr("berhasil diekspor ke .rnote.", "exported to .rnote.")
                                ));
                            }
                            Err(e) => {
                                self.toast_message = Some(format!(
                                    "{}: {e}",
                                    lang.tr("⚠ Gagal ekspor .rnote", "⚠ Failed to export .rnote")
                                ));
                            }
                        }
                    }
                }
            });

        // Floating Drag & Drop Ghost Overlay + Cleanup
        if let Some(payload) = &self.library_drag_payload {
            if let Some(pos) = ctx.input(|i| i.pointer.hover_pos()) {
                let drag_label = match payload {
                    LibraryDragPayload::Notebooks(ids) if ids.len() == 1 => {
                        let title = self
                            .library
                            .notebooks
                            .iter()
                            .find(|n| n.id == ids[0])
                            .map(|n| n.title.as_str())
                            .unwrap_or("Notebook");
                        format!("📖 {title}")
                    }
                    LibraryDragPayload::Notebooks(ids) => match lang {
                        crate::model::AppLanguage::Indonesian => {
                            format!("📚 Memindahkan {} buku", ids.len())
                        }
                        crate::model::AppLanguage::English => {
                            format!("📚 Moving {} notebooks", ids.len())
                        }
                    },
                    LibraryDragPayload::Folder(fid) => {
                        let name = self
                            .library
                            .folders
                            .iter()
                            .find(|f| f.id == *fid)
                            .map(|f| f.name.as_str())
                            .unwrap_or("Folder");
                        format!("📁 {name}")
                    }
                };
                egui::Area::new(egui::Id::new("library_drag_ghost"))
                    .fixed_pos(pos + Vec2::new(14.0, 14.0))
                    .order(Order::Tooltip)
                    .interactable(false)
                    .show(ctx, |ui| {
                        egui::Frame::NONE
                            .fill(palette.bg_elevated)
                            .stroke(Stroke::new(1.5_f32, palette.accent))
                            .corner_radius(8.0)
                            .inner_margin(egui::Margin::symmetric(10, 6))
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new(drag_label)
                                        .size(12.5)
                                        .strong()
                                        .color(palette.text_primary),
                                );
                            });
                    });
            }
            if pointer_released {
                self.library_drag_payload = None;
            }
        }

        // Render seluruh modal Home Page & global modals
        self.render_library_modals(ctx);

        if ctx.input(|i| i.key_pressed(egui::Key::F1)) {
            self.show_shortcut_modal = !self.show_shortcut_modal;
        }

        self.render_shortcut_modal(ctx);
        self.render_about_modal(ctx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AppLanguage, PageTemplate, ThemeMode};

    #[test]
    fn test_library_view_renders_cleanly_in_light_and_dark_themes_and_both_languages() {
        for theme in [ThemeMode::Light, ThemeMode::Dark] {
            for lang in [AppLanguage::Indonesian, AppLanguage::English] {
                for style in [LibraryViewStyle::Grid, LibraryViewStyle::List] {
                    let mut app = RustNotesApp::default();
                    app.settings.theme_mode = theme;
                    app.settings.language = lang;
                    app.library_view_style = style;
                    app.library.notebooks.push(Notebook::new(
                        "Very Long Notebook Title For Responsive Check",
                        PageTemplate::Ruled,
                        Color32::from_rgb(49, 85, 119),
                    ));

                    let ctx = egui::Context::default();
                    // Uji di ukuran jendela sempit (520x480) dan lebar (1360x820)
                    for size in [Vec2::new(520.0, 480.0), Vec2::new(1360.0, 820.0)] {
                        let raw = egui::RawInput {
                            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
                            ..Default::default()
                        };
                        let _ = ctx.run(raw, |ctx| {
                            crate::app::setup_goodnotes_theme(ctx, theme);
                            app.render_library_view(ctx);
                        });
                    }
                }
            }
        }
    }

    #[test]
    fn test_library_drag_move_notebooks_and_folder_cycle_guard() {
        let mut app = RustNotesApp::default();
        let mut f1 = crate::model::Folder::new("Kuliah", None);
        f1.id = "f1".to_string();
        let mut f2 = crate::model::Folder::new("Kalkulus", Some("f1".to_string()));
        f2.id = "f2".to_string();
        app.library.folders = vec![f1, f2];

        let mut nb = Notebook::new("Catatan 1", PageTemplate::Blank, Color32::WHITE);
        nb.id = "nb1".to_string();
        app.library.notebooks.push(nb);

        // Move notebook into subfolder
        assert_eq!(
            app.move_notebooks_to_folder(&["nb1".to_string()], Some("f2")),
            1
        );
        assert_eq!(app.library.notebooks[0].folder_id.as_deref(), Some("f2"));

        // Moving folder into its own descendant must be blocked
        assert!(!app.move_folder_to_parent("f1", Some("f2")));
        assert_eq!(app.library.folders[0].parent_id, None);
    }
}

use super::state::RustNotesApp;
use super::types::ViewMode;
use crate::canvas::{ActiveTool, EraserMode};
use crate::model::{ShapeKind, ThemeMode};
use crate::storage::{
    app_storage_dir, export_notebook_to_pdf, export_page_to_png, export_page_to_svg_file,
    save_settings_to_dir,
};
use eframe::egui::{
    self, Align, Button, Color32, FontId, Layout, Pos2, Rect, RichText, Sense, Stroke, StrokeKind,
    Vec2,
};

impl RustNotesApp {
    /// Render navbar editor (bilah atas Goodnotes navy) beserta pop-up bawaannya.
    /// Mengembalikan alse jika pengguna memilih kembali ke perpustakaan.
    pub fn render_editor_navbar(&mut self, ctx: &egui::Context, nb_idx: usize) -> bool {
        let notebook_id = self.library.notebooks[nb_idx].id.clone();
        let lang = self.settings.language;

        // Authentic GoodNotes Single Navy Top Navbar (#284872) + Pop-ups for Specific & Advanced Features
        let mut return_to_library = false;
        egui::TopBottomPanel::top("editor_navbar")
            .min_height(44.0)
            .show(ctx, |ui| {
                ui.painter().rect_filled(
                    ui.max_rect(),
                    0.0,
                    Color32::from_rgb(40, 72, 114), // #284872 GoodNotes Navy
                );

                ui.add_space(5.0);
                let total_w = ui.available_width();
                let is_wide = total_w >= 1020.0;

                // Helper closures/blocks via macro-free scoped methods on ui:
                // Baris Utama: Kiri (Navigasi & Judul), Tengah (Tools Center pada layar lebar), Paling Kanan (Settings & Page Actions)
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    ui.add_space(6.0);

                    // --- 1. LEFT GROUP: Back, Document Title ▾, Sidebar, Search, Zoom Win, Bookmark ---
                    if ui
                        .add(
                            Button::new(RichText::new("◀").size(14.0).color(Color32::WHITE))
                                .fill(Color32::from_rgb(54, 95, 145))
                                .corner_radius(6.0)
                                .min_size(Vec2::new(28.0, 28.0)),
                        )
                        .on_hover_text(lang.tr("Kembali ke Perpustakaan", "Back to Library"))
                        .clicked()
                    {
                        return_to_library = true;
                    }

                    // Document Title with Dropdown (`[Title] ▾`) like GoodNotes
                    let raw_title = &self.library.notebooks[nb_idx].title;
                    let max_chars = if total_w < 1180.0 { 14 } else { 22 };
                    let display_title = if raw_title.chars().count() > max_chars {
                        format!(
                            "{}… ▾",
                            raw_title.chars().take(max_chars - 1).collect::<String>()
                        )
                    } else {
                        format!("{raw_title} ▾")
                    };
                    let title_btn = Button::new(
                        RichText::new(display_title)
                            .size(13.5)
                            .strong()
                            .color(Color32::WHITE),
                    )
                    .fill(if self.show_doc_title_popup {
                        Color32::from_rgb(54, 95, 145)
                    } else {
                        Color32::TRANSPARENT
                    })
                    .corner_radius(6.0);
                    if ui
                        .add(title_btn)
                        .on_hover_text(lang.tr(
                            "Klik untuk ubah judul & info dokumen",
                            "Click to rename & view document info",
                        ))
                        .clicked()
                    {
                        self.show_doc_title_popup = !self.show_doc_title_popup;
                        self.show_tool_popup = false;
                        self.show_more_menu_popup = false;
                    }

                    // Page Thumbnail Sidebar Icon (`◫`)
                    let sb_btn = Button::new(
                        RichText::new("◫")
                            .size(14.0)
                            .color(if self.show_thumbnail_sidebar {
                                Color32::from_rgb(130, 195, 255)
                            } else {
                                Color32::WHITE
                            }),
                    )
                    .fill(if self.show_thumbnail_sidebar {
                        Color32::from_rgb(54, 95, 145)
                    } else {
                        Color32::TRANSPARENT
                    })
                    .corner_radius(6.0)
                    .min_size(Vec2::new(28.0, 28.0));
                    if ui
                        .add(sb_btn)
                        .on_hover_text(lang.tr(
                            "Panel Halaman, Bookmark & Outline (Ctrl+B)",
                            "Pages, Bookmarks & Outline Sidebar (Ctrl+B)",
                        ))
                        .clicked()
                    {
                        self.show_thumbnail_sidebar = !self.show_thumbnail_sidebar;
                    }

                    // Search Icon (`🔍`)
                    let find_btn =
                        Button::new(RichText::new("🔍").size(13.0).color(Color32::WHITE))
                            .fill(if self.show_in_doc_search {
                                Color32::from_rgb(54, 95, 145)
                            } else {
                                Color32::TRANSPARENT
                            })
                            .corner_radius(6.0)
                            .min_size(Vec2::new(28.0, 28.0));
                    if ui
                        .add(find_btn)
                        .on_hover_text(
                            lang.tr("Cari di Dokumen (Ctrl+F)", "Find in Document (Ctrl+F)"),
                        )
                        .clicked()
                    {
                        self.show_in_doc_search = !self.show_in_doc_search;
                        self.doc_search_current_match = 0;
                    }

                    // Zoom Window Icon (`🔍+`)
                    let zw_btn = Button::new(RichText::new("🔍+").size(12.0).color(Color32::WHITE))
                        .fill(if self.canvas_state.show_zoom_window {
                            Color32::from_rgb(0, 114, 206)
                        } else {
                            Color32::TRANSPARENT
                        })
                        .corner_radius(6.0)
                        .min_size(Vec2::new(30.0, 28.0));
                    if ui
                        .add(zw_btn)
                        .on_hover_text(lang.tr(
                            "Jendela Pembesar Tulis / Zoom Window (Alt+Z)",
                            "Zoom Window for Detailed Writing (Alt+Z)",
                        ))
                        .clicked()
                    {
                        self.canvas_state.show_zoom_window = !self.canvas_state.show_zoom_window;
                    }

                    // Bookmark Icon (`★`/`☆`)
                    let is_bm =
                        self.library.notebooks[nb_idx].pages[self.current_page_index].bookmarked;
                    let bm_icon = if is_bm { "★" } else { "☆" };
                    let bm_col = if is_bm {
                        Color32::from_rgb(255, 204, 0)
                    } else {
                        Color32::WHITE
                    };
                    if ui
                        .add(
                            Button::new(RichText::new(bm_icon).size(15.0).color(bm_col))
                                .fill(Color32::TRANSPARENT)
                                .min_size(Vec2::new(26.0, 28.0)),
                        )
                        .on_hover_text(
                            lang.tr("Tandai Halaman Ini (Bookmark)", "Bookmark Current Page"),
                        )
                        .clicked()
                    {
                        self.library.notebooks[nb_idx].pages[self.current_page_index].bookmarked =
                            !is_bm;
                        self.mark_dirty(&notebook_id);
                    }

                    let left_end_x = ui.cursor().min.x;

                    // --- 3. FAR RIGHT GROUP: Settings (`⋯`), Export (`📤`), Add Page (`📄+`), Paper Size (`📐`), Scroll Mode (`📜`) ---
                    let mut right_start_x = ui.max_rect().right();
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.add_space(8.0);
                        ui.spacing_mut().item_spacing.x = 6.0;

                        // GoodNotes Circular More Menu / Settings Button (`⋯`) — Paling Kanan
                        let more_btn = Button::new(
                            RichText::new("⋯")
                                .size(16.0)
                                .strong()
                                .color(if self.show_more_menu_popup {
                                    Color32::from_rgb(30, 58, 95)
                                } else {
                                    Color32::WHITE
                                }),
                        )
                        .fill(if self.show_more_menu_popup {
                            Color32::from_rgb(214, 232, 255)
                        } else {
                            Color32::from_rgb(54, 95, 145)
                        })
                        .corner_radius(14.0)
                        .min_size(Vec2::new(30.0, 28.0));
                        if ui
                            .add(more_btn)
                            .on_hover_text(lang.tr(
                                "Pengaturan & Fitur Lanjutan (GoodNotes More Menu)",
                                "Settings & Advanced Features Pop-up",
                            ))
                            .clicked()
                        {
                            self.show_more_menu_popup = !self.show_more_menu_popup;
                            self.show_tool_popup = false;
                            self.show_doc_title_popup = false;
                        }

                        // Export Menu (`📤`)
                        ui.menu_button(
                            RichText::new("📤").size(13.5).color(Color32::WHITE),
                            |ui| {
                                if ui.button("📄 Export Notebook to PDF").clicked() {
                                    ui.close_menu();
                                    let nb = &self.library.notebooks[nb_idx];
                                    let default_name = format!(
                                        "{}.pdf",
                                        nb.title.replace(
                                            ['/', '\\', ':', '*', '?', '"', '<', '>', '|'],
                                            "_"
                                        )
                                    );
                                    if let Some(path) = rfd::FileDialog::new()
                                        .set_file_name(&default_name)
                                        .add_filter("PDF Document", &["pdf"])
                                        .save_file()
                                    {
                                        let storage_dir = app_storage_dir();
                                        match export_notebook_to_pdf(&storage_dir, nb, &path) {
                                            Ok(()) => {
                                                self.toast_message = Some(
                                                    "✅ Dokumen PDF berhasil diekspor.".to_string(),
                                                );
                                            }
                                            Err(e) => {
                                                self.toast_message =
                                                    Some(format!("⚠ Gagal ekspor PDF: {e}"));
                                            }
                                        }
                                    }
                                }
                                if ui.button("🖼 Export Page to PNG").clicked() {
                                    ui.close_menu();
                                    let page = &self.library.notebooks[nb_idx].pages
                                        [self.current_page_index];
                                    let default_name =
                                        format!("page_{}.png", self.current_page_index + 1);
                                    if let Some(path) = rfd::FileDialog::new()
                                        .set_file_name(&default_name)
                                        .add_filter("PNG Image", &["png"])
                                        .save_file()
                                    {
                                        let storage_dir = app_storage_dir();
                                        match export_page_to_png(
                                            &storage_dir,
                                            &notebook_id,
                                            page,
                                            &path,
                                        ) {
                                            Ok(()) => {
                                                self.toast_message = Some(
                                                    "✅ Halaman berhasil diekspor ke PNG."
                                                        .to_string(),
                                                );
                                            }
                                            Err(e) => {
                                                self.toast_message =
                                                    Some(format!("⚠ Gagal ekspor PNG: {e}"));
                                            }
                                        }
                                    }
                                }
                                if ui.button("📐 Export Page to SVG (Vector)").clicked() {
                                    ui.close_menu();
                                    let page = &self.library.notebooks[nb_idx].pages
                                        [self.current_page_index];
                                    let default_name =
                                        format!("page_{}.svg", self.current_page_index + 1);
                                    if let Some(path) = rfd::FileDialog::new()
                                        .set_file_name(&default_name)
                                        .add_filter("SVG Vector Graphic", &["svg"])
                                        .save_file()
                                    {
                                        match export_page_to_svg_file(&path, page) {
                                            Ok(()) => {
                                                self.toast_message = Some(
                                                    "✅ Halaman berhasil diekspor ke SVG (Vector)."
                                                        .to_string(),
                                                );
                                            }
                                            Err(e) => {
                                                self.toast_message =
                                                    Some(format!("⚠ Gagal ekspor SVG: {e}"));
                                            }
                                        }
                                    }
                                }
                            },
                        );

                        // Add Page (`📄+`)
                        let add_page_btn = Button::new(
                            RichText::new("📄+")
                                .size(13.0)
                                .strong()
                                .color(Color32::WHITE),
                        )
                        .fill(Color32::from_rgb(54, 95, 145))
                        .corner_radius(6.0)
                        .min_size(Vec2::new(34.0, 28.0));
                        if ui
                            .add(add_page_btn)
                            .on_hover_text(lang.tr(
                                "Atur & Tambah Halaman Baru...",
                                "Configure & Add New Page...",
                            ))
                            .clicked()
                        {
                            self.open_add_page_modal(nb_idx);
                        }

                        // Paper Size Quick Badge (`📐 595×842`) -> opens Per-Page / Custom Paper Size Modal
                        let (cur_pw, cur_ph) = {
                            let p = &self.library.notebooks[nb_idx].pages[self.current_page_index];
                            (p.width, p.height)
                        };
                        let size_btn = Button::new(
                            RichText::new(format!("📐 {:.0}×{:.0}", cur_pw, cur_ph))
                                .size(11.5)
                                .color(Color32::WHITE),
                        )
                        .fill(Color32::from_rgb(54, 95, 145))
                        .corner_radius(14.0)
                        .min_size(Vec2::new(72.0, 28.0));
                        if ui
                            .add(size_btn)
                            .on_hover_text(lang.tr(
                                "Atur Ukuran Kertas Spesifik / Per-Halaman & Template...",
                                "Configure Custom / Per-Page Paper Size & Template...",
                            ))
                            .clicked()
                        {
                            self.open_page_resize_modal(nb_idx, self.current_page_index);
                        }

                        // Scrollable vs Single Page Toggle (`📜 Scroll` vs `📄 1 Hal`)
                        let view_label = match self.view_mode {
                            ViewMode::ContinuousVertical => lang.tr("📜 Scroll", "📜 Scroll"),
                            ViewMode::SinglePage => lang.tr("📄 1 Hal", "📄 1 Page"),
                        };
                        let vm_btn = Button::new(
                            RichText::new(view_label)
                                .size(12.0)
                                .strong()
                                .color(Color32::WHITE),
                        )
                        .fill(match self.view_mode {
                            ViewMode::ContinuousVertical => Color32::from_rgb(0, 114, 206),
                            ViewMode::SinglePage => Color32::from_rgb(54, 95, 145),
                        })
                        .corner_radius(14.0)
                        .min_size(Vec2::new(68.0, 28.0));
                        if ui
                            .add(vm_btn)
                            .on_hover_text(lang.tr(
                                "Toggle Mode Halaman: Scroll Menyambung (Continuous) vs Satu Halaman Tunggal (Single Page, tidak scroll ke halaman lain)",
                                "Toggle Page Mode: Scrollable Connected Pages vs Single Isolated Page",
                            ))
                            .clicked()
                        {
                            let next_mode = match self.view_mode {
                                ViewMode::SinglePage => ViewMode::ContinuousVertical,
                                ViewMode::ContinuousVertical => ViewMode::SinglePage,
                            };
                            self.set_view_mode(nb_idx, next_mode);
                        }

                        right_start_x = ui.cursor().max.x;

                        // --- 2. CENTER GROUP (pada layar lebar): Tools ditaruh tepat di tengah (Center) navbar ---
                        if is_wide {
                            let tools_width = 456.0_f32;
                            let ideal_left = (total_w - tools_width) * 0.5;
                            let clamped_left = ideal_left
                                .max(left_end_x + 8.0)
                                .min((right_start_x - tools_width - 8.0).max(left_end_x + 8.0));
                            let center_rect = Rect::from_min_size(
                                Pos2::new(clamped_left, ui.max_rect().top()),
                                Vec2::new(tools_width, 30.0),
                            );
                            let mut center_ui = ui.new_child(
                                egui::UiBuilder::new()
                                    .max_rect(center_rect)
                                    .layout(Layout::left_to_right(Align::Center)),
                            );
                            center_ui.spacing_mut().item_spacing.x = 4.0;

                            let active_shape_kind = match self.canvas_state.current_tool {
                                ActiveTool::Shape(k) => k,
                                _ => ShapeKind::Rectangle,
                            };
                            let tools = [
                                (
                                    ActiveTool::Pen,
                                    "✒",
                                    "Pen (P) — Klik lagi untuk atur ketebalan & warna",
                                ),
                                (ActiveTool::Pencil, "✏", "Pencil — Goresan pensil"),
                                (
                                    ActiveTool::Highlighter,
                                    "🖍",
                                    "Highlighter (H) — Stabilo transparan",
                                ),
                                (
                                    ActiveTool::Eraser,
                                    "⌫",
                                    "Eraser (E) — Klik lagi untuk opsi Partial / Whole Stroke",
                                ),
                                (
                                    ActiveTool::Shape(active_shape_kind),
                                    "📐",
                                    "Shapes — Klik lagi untuk pilih bentuk",
                                ),
                                (
                                    ActiveTool::Connector,
                                    "🔗",
                                    "Connector (C) — Tarik garis penghubung antar bentuk",
                                ),
                                (ActiveTool::Lasso, "✂", "Lasso Selection (L)"),
                                (ActiveTool::Text, "T", "Text Box (T)"),
                                (ActiveTool::StickyNote, "🗒", "Sticky Note"),
                                (ActiveTool::Laser, "✨", "Laser Pointer"),
                                (ActiveTool::Hand, "✋", "Hand / Pan Mode (Space)"),
                            ];

                            for (tool_var, icon, tooltip) in tools {
                                let is_sel = match (self.canvas_state.current_tool, tool_var) {
                                    (ActiveTool::Shape(_), ActiveTool::Shape(_)) => true,
                                    (a, b) => a == b,
                                };
                                let btn = Button::new(
                                    RichText::new(icon)
                                        .size(14.0)
                                        .strong()
                                        .color(if is_sel {
                                            Color32::from_rgb(30, 58, 95)
                                        } else {
                                            Color32::WHITE
                                        }),
                                )
                                .fill(if is_sel {
                                    Color32::from_rgb(214, 232, 255)
                                } else {
                                    Color32::TRANSPARENT
                                })
                                .corner_radius(7.0)
                                .min_size(Vec2::new(28.0, 28.0));

                                if center_ui.add(btn).on_hover_text(tooltip).clicked() {
                                    if is_sel {
                                        self.show_tool_popup = !self.show_tool_popup;
                                        self.show_more_menu_popup = false;
                                        self.show_doc_title_popup = false;
                                    } else {
                                        self.canvas_state.current_tool = tool_var;
                                    }
                                }
                            }

                            // Insert Image Icon (`🖼`)
                            let img_btn = Button::new(
                                RichText::new("🖼").size(14.0).color(Color32::WHITE),
                            )
                            .fill(Color32::TRANSPARENT)
                            .corner_radius(7.0)
                            .min_size(Vec2::new(28.0, 28.0));
                            if center_ui
                                .add(img_btn)
                                .on_hover_text(
                                    lang.tr("Sisipkan Gambar ke Halaman", "Insert Image to Page"),
                                )
                                .clicked()
                                && let Some(path) = rfd::FileDialog::new()
                                    .add_filter("Images", &["png", "jpg", "jpeg", "webp"])
                                    .pick_file()
                                && let Ok(bytes) = std::fs::read(&path)
                            {
                                let ext =
                                    path.extension().and_then(|s| s.to_str()).unwrap_or("png");
                                if self.insert_image_to_page(
                                    nb_idx,
                                    self.current_page_index,
                                    &bytes,
                                    ext,
                                    None,
                                ) {
                                    self.toast_message = Some(
                                        "✅ Gambar berhasil disisipkan ke halaman.".to_string(),
                                    );
                                }
                            }

                            // Active Stroke & Color Pop-up Pill (`● 3.0px ▾`)
                            let (sw_rect, sw_resp) = center_ui
                                .allocate_exact_size(Vec2::new(74.0, 28.0), Sense::click());
                            let pill_bg = if self.show_tool_popup || sw_resp.hovered() {
                                Color32::from_rgb(54, 95, 145)
                            } else {
                                Color32::from_rgb(31, 56, 90)
                            };
                            center_ui.painter().rect_filled(sw_rect, 14.0, pill_bg);
                            let dot_center = Pos2::new(sw_rect.left() + 14.0, sw_rect.center().y);
                            center_ui.painter().circle(
                                dot_center,
                                7.0,
                                self.canvas_state.stroke_color,
                                Stroke::new(1.5_f32, Color32::WHITE),
                            );
                            center_ui.painter().text(
                                Pos2::new(sw_rect.left() + 26.0, sw_rect.center().y),
                                egui::Align2::LEFT_CENTER,
                                format!("{:.1}px ▾", self.canvas_state.stroke_width),
                                FontId::proportional(11.5),
                                Color32::WHITE,
                            );
                            if sw_resp
                                .on_hover_text(lang.tr(
                                    "Pengaturan Ketebalan Pena, Warna & Opsi Alat",
                                    "Stroke Width, Color Palette & Tool Options Pop-up",
                                ))
                                .clicked()
                            {
                                self.show_tool_popup = !self.show_tool_popup;
                                self.show_more_menu_popup = false;
                                self.show_doc_title_popup = false;
                            }
                        }
                    });
                });

                // Pada jendela sempit (< 1020px), baris kedua merender Tools tepat di tengah (Center) agar tidak menabrak Kiri/Kanan
                if !is_wide {
                    ui.add_space(3.0);
                    ui.vertical_centered(|ui| {
                        let tools_width = 456.0_f32.min(total_w - 16.0);
                        let start_x = ((total_w - tools_width) * 0.5).max(8.0);
                        ui.horizontal_wrapped(|ui| {
                            ui.add_space(start_x);
                            ui.spacing_mut().item_spacing.x = 4.0;
                            let active_shape_kind = match self.canvas_state.current_tool {
                                ActiveTool::Shape(k) => k,
                                _ => ShapeKind::Rectangle,
                            };
                            let tools = [
                                (
                                    ActiveTool::Pen,
                                    "✒",
                                    "Pen (P) — Klik lagi untuk atur ketebalan & warna",
                                ),
                                (ActiveTool::Pencil, "✏", "Pencil — Goresan pensil"),
                                (
                                    ActiveTool::Highlighter,
                                    "🖍",
                                    "Highlighter (H) — Stabilo transparan",
                                ),
                                (
                                    ActiveTool::Eraser,
                                    "⌫",
                                    "Eraser (E) — Klik lagi untuk opsi Partial / Whole Stroke",
                                ),
                                (
                                    ActiveTool::Shape(active_shape_kind),
                                    "📐",
                                    "Shapes — Klik lagi untuk pilih bentuk",
                                ),
                                (
                                    ActiveTool::Connector,
                                    "🔗",
                                    "Connector (C) — Tarik garis penghubung antar bentuk",
                                ),
                                (ActiveTool::Lasso, "✂", "Lasso Selection (L)"),
                                (ActiveTool::Text, "T", "Text Box (T)"),
                                (ActiveTool::StickyNote, "🗒", "Sticky Note"),
                                (ActiveTool::Laser, "✨", "Laser Pointer"),
                                (ActiveTool::Hand, "✋", "Hand / Pan Mode (Space)"),
                            ];

                            for (tool_var, icon, tooltip) in tools {
                                let is_sel = match (self.canvas_state.current_tool, tool_var) {
                                    (ActiveTool::Shape(_), ActiveTool::Shape(_)) => true,
                                    (a, b) => a == b,
                                };
                                let btn = Button::new(
                                    RichText::new(icon)
                                        .size(14.0)
                                        .strong()
                                        .color(if is_sel {
                                            Color32::from_rgb(30, 58, 95)
                                        } else {
                                            Color32::WHITE
                                        }),
                                )
                                .fill(if is_sel {
                                    Color32::from_rgb(214, 232, 255)
                                } else {
                                    Color32::TRANSPARENT
                                })
                                .corner_radius(7.0)
                                .min_size(Vec2::new(28.0, 28.0));

                                if ui.add(btn).on_hover_text(tooltip).clicked() {
                                    if is_sel {
                                        self.show_tool_popup = !self.show_tool_popup;
                                        self.show_more_menu_popup = false;
                                        self.show_doc_title_popup = false;
                                    } else {
                                        self.canvas_state.current_tool = tool_var;
                                    }
                                }
                            }

                            let img_btn = Button::new(
                                RichText::new("🖼").size(14.0).color(Color32::WHITE),
                            )
                            .fill(Color32::TRANSPARENT)
                            .corner_radius(7.0)
                            .min_size(Vec2::new(28.0, 28.0));
                            if ui
                                .add(img_btn)
                                .on_hover_text(
                                    lang.tr("Sisipkan Gambar ke Halaman", "Insert Image to Page"),
                                )
                                .clicked()
                                && let Some(path) = rfd::FileDialog::new()
                                    .add_filter("Images", &["png", "jpg", "jpeg", "webp"])
                                    .pick_file()
                                && let Ok(bytes) = std::fs::read(&path)
                            {
                                let ext =
                                    path.extension().and_then(|s| s.to_str()).unwrap_or("png");
                                if self.insert_image_to_page(
                                    nb_idx,
                                    self.current_page_index,
                                    &bytes,
                                    ext,
                                    None,
                                ) {
                                    self.toast_message = Some(
                                        "✅ Gambar berhasil disisipkan ke halaman.".to_string(),
                                    );
                                }
                            }

                            let (sw_rect, sw_resp) =
                                ui.allocate_exact_size(Vec2::new(74.0, 28.0), Sense::click());
                            let pill_bg = if self.show_tool_popup || sw_resp.hovered() {
                                Color32::from_rgb(54, 95, 145)
                            } else {
                                Color32::from_rgb(31, 56, 90)
                            };
                            ui.painter().rect_filled(sw_rect, 14.0, pill_bg);
                            let dot_center = Pos2::new(sw_rect.left() + 14.0, sw_rect.center().y);
                            ui.painter().circle(
                                dot_center,
                                7.0,
                                self.canvas_state.stroke_color,
                                Stroke::new(1.5_f32, Color32::WHITE),
                            );
                            ui.painter().text(
                                Pos2::new(sw_rect.left() + 26.0, sw_rect.center().y),
                                egui::Align2::LEFT_CENTER,
                                format!("{:.1}px ▾", self.canvas_state.stroke_width),
                                FontId::proportional(11.5),
                                Color32::WHITE,
                            );
                            if sw_resp
                                .on_hover_text(lang.tr(
                                    "Pengaturan Ketebalan Pena, Warna & Opsi Alat",
                                    "Stroke Width, Color Palette & Tool Options Pop-up",
                                ))
                                .clicked()
                            {
                                self.show_tool_popup = !self.show_tool_popup;
                                self.show_more_menu_popup = false;
                                self.show_doc_title_popup = false;
                            }
                        });
                    });
                }
                ui.add_space(4.0);
            });

        if return_to_library {
            let nb_id = notebook_id.clone();
            self.force_save_notebook(&nb_id);
            self.active_notebook_id = None;
            return false;
        }

        // =====================================================================
        // GOODNOTES POP-UP 1: Document Title & Info Pop-up (`[Title] ▾`)
        // =====================================================================
        if self.show_doc_title_popup {
            egui::Window::new("📄 Info & Judul Dokumen")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::LEFT_TOP, [44.0, 50.0])
                .show(ctx, |ui| {
                    ui.set_width(290.0);
                    ui.label(
                        RichText::new(lang.tr("Judul Catatan:", "Notebook Title:"))
                            .size(12.0)
                            .color(Color32::GRAY),
                    );
                    if ui
                        .text_edit_singleline(&mut self.library.notebooks[nb_idx].title)
                        .changed()
                    {
                        self.mark_dirty(&notebook_id);
                    }
                    ui.add_space(6.0);
                    let page_cnt = self.library.notebooks[nb_idx].pages.len();
                    let cur_p = &self.library.notebooks[nb_idx].pages[self.current_page_index];
                    ui.label(
                        RichText::new(format!(
                            "Halaman Aktif: {} dari {} ({:.0} × {:.0} pt, {:?})",
                            self.current_page_index + 1,
                            page_cnt,
                            cur_p.width,
                            cur_p.height,
                            cur_p.template
                        ))
                        .size(11.5)
                        .color(Color32::LIGHT_GRAY),
                    );
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if ui.button("📐 Atur Ukuran Kertas...").clicked() {
                            self.show_doc_title_popup = false;
                            self.open_page_resize_modal(nb_idx, self.current_page_index);
                        }
                        if ui.button("Tutup").clicked() {
                            self.show_doc_title_popup = false;
                        }
                    });
                });
        }

        // =====================================================================
        // GOODNOTES POP-UP 2: Tool Settings Pop-up (Ketebalan, Warna, Bentuk, Eraser)
        // =====================================================================
        if self.show_tool_popup {
            egui::Window::new("🎨 Pengaturan Alat & Palet")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_TOP, [0.0, 50.0])
                .show(ctx, |ui| {
                    ui.set_width(360.0);

                    // Ketebalan Goresan (Stroke Width Presets + Slider)
                    ui.label(
                        RichText::new(
                            lang.tr("Ketebalan Goresan (Stroke Width):", "Stroke Width:"),
                        )
                        .strong()
                        .size(12.0),
                    );
                    ui.horizontal_wrapped(|ui| {
                        let mut remove_width_idx = None;
                        let widths = self.settings.pen_widths.clone();
                        for (idx, &w) in widths.iter().enumerate() {
                            let is_sel = (self.canvas_state.stroke_width - w).abs() < 0.1;
                            let btn = Button::new(format!("{w:.1}px"))
                                .fill(if is_sel {
                                    Color32::from_rgb(0, 114, 206)
                                } else {
                                    Color32::from_rgb(65, 63, 64)
                                })
                                .corner_radius(6.0);
                            let resp = ui.add(btn);
                            if resp.clicked() {
                                self.canvas_state.stroke_width = w;
                            }
                            if resp.secondary_clicked() && self.settings.pen_widths.len() > 1 {
                                remove_width_idx = Some(idx);
                            }
                            resp.on_hover_text(
                                "Klik kiri: pilih ketebalan | Klik kanan: hapus preset",
                            );
                        }
                        if let Some(rem) = remove_width_idx {
                            self.settings.pen_widths.remove(rem);
                            let _ = save_settings_to_dir(&app_storage_dir(), &self.settings);
                        }
                        if ui
                            .small_button("+")
                            .on_hover_text("Tambah preset ketebalan baru (+2.0px)")
                            .clicked()
                        {
                            let next_w = (self.settings.pen_widths.last().copied().unwrap_or(2.0)
                                + 2.0)
                                .min(24.0);
                            if !self
                                .settings
                                .pen_widths
                                .iter()
                                .any(|&x| (x - next_w).abs() < 0.1)
                            {
                                self.settings.pen_widths.push(next_w);
                                let _ = save_settings_to_dir(&app_storage_dir(), &self.settings);
                            }
                        }
                    });
                    ui.add(
                        egui::Slider::new(&mut self.canvas_state.stroke_width, 0.5..=28.0)
                            .text("px"),
                    );

                    ui.separator();

                    // Palet Warna (Color Swatches + Color Picker)
                    ui.label(
                        RichText::new(lang.tr("Palet Warna (Color Swatches):", "Color Swatches:"))
                            .strong()
                            .size(12.0),
                    );
                    ui.horizontal_wrapped(|ui| {
                        let mut remove_swatch_idx = None;
                        let swatches = self.settings.color_swatches.clone();
                        for (idx, c_data) in swatches.iter().enumerate() {
                            let c = c_data.to_color32();
                            let (rect, resp) =
                                ui.allocate_exact_size(Vec2::new(24.0, 24.0), Sense::click());
                            let stroke = if self.canvas_state.stroke_color == c {
                                Stroke::new(2.5_f32, Color32::from_rgb(76, 147, 236))
                            } else {
                                Stroke::new(1.0_f32, Color32::from_rgb(100, 100, 100))
                            };
                            ui.painter_at(rect)
                                .rect(rect, 12.0, c, stroke, StrokeKind::Outside);
                            if resp.clicked() {
                                self.canvas_state.stroke_color = c;
                            }
                            if resp.secondary_clicked() && self.settings.color_swatches.len() > 1 {
                                remove_swatch_idx = Some(idx);
                            }
                            resp.on_hover_text("Klik kiri: pilih warna | Klik kanan: hapus swatch");
                        }
                        if let Some(rem) = remove_swatch_idx {
                            self.settings.color_swatches.remove(rem);
                            let _ = save_settings_to_dir(&app_storage_dir(), &self.settings);
                        }
                        ui.color_edit_button_srgba(&mut self.canvas_state.stroke_color);
                        if ui
                            .small_button("+ Simpan Warna")
                            .on_hover_text("Simpan warna aktif ke palet swatch")
                            .clicked()
                        {
                            let new_sw: crate::model::ColorData =
                                self.canvas_state.stroke_color.into();
                            if !self.settings.color_swatches.contains(&new_sw) {
                                self.settings.color_swatches.push(new_sw);
                                let _ = save_settings_to_dir(&app_storage_dir(), &self.settings);
                            }
                        }
                    });

                    // Opsi Spesifik Bentuk Geometri (Shapes)
                    if matches!(self.canvas_state.current_tool, ActiveTool::Shape(_)) {
                        ui.separator();
                        ui.label(RichText::new("Jenis Bentuk (Shape):").strong().size(12.0));
                        ui.horizontal_wrapped(|ui| {
                            let scalene_preset = ShapeKind::ScaleneTriangle {
                                u0: 0.22,
                                v0: 0.0,
                                u1: 1.0,
                                v1: 0.72,
                                u2: 0.0,
                                v2: 1.0,
                            };
                            let shapes = [
                                (ShapeKind::Rectangle, "▭ Rectangle"),
                                (ShapeKind::Parallelogram, "▱ Jajar Genjang"),
                                (ShapeKind::Ellipse, "◯ Circle / Ellipse"),
                                (ShapeKind::Line, "╱ Line"),
                                (ShapeKind::Triangle, "△ Segitiga Sama Kaki"),
                                (ShapeKind::RightTriangle, "◣ Segitiga Siku-Siku"),
                                (scalene_preset, "◿ Segitiga Sembarang"),
                            ];
                            for (kind, name) in shapes {
                                let is_sel = match (self.canvas_state.current_tool, kind) {
                                    (
                                        ActiveTool::Shape(ShapeKind::ScaleneTriangle { .. }),
                                        ShapeKind::ScaleneTriangle { .. },
                                    ) => true,
                                    (ActiveTool::Shape(cur), k) => cur == k,
                                    _ => false,
                                };
                                if ui.selectable_label(is_sel, name).clicked() {
                                    self.canvas_state.current_tool = ActiveTool::Shape(kind);
                                }
                            }
                        });

                        ui.add_space(4.0);
                        ui.label(
                            RichText::new("📋 Template Diagram Siap Pakai:")
                                .strong()
                                .size(11.5),
                        );
                        ui.horizontal_wrapped(|ui| {
                            for kind in crate::geometry::DiagramStarterKind::ALL {
                                if ui
                                    .button(kind.title())
                                    .on_hover_text(kind.description())
                                    .clicked()
                                {
                                    let cur_p = &self.library.notebooks[nb_idx].pages
                                        [self.current_page_index];
                                    let new_objs = crate::geometry::generate_diagram_starter(
                                        kind,
                                        cur_p.width,
                                        cur_p.height,
                                    );
                                    let added_count = new_objs.len();
                                    self.canvas_state.push_undo(&cur_p.id, &cur_p.objects);
                                    let new_ids =
                                        new_objs.iter().map(|o| o.id().to_string()).collect();
                                    self.library.notebooks[nb_idx].pages[self.current_page_index]
                                        .objects
                                        .extend(new_objs);
                                    self.canvas_state.selected_ids = new_ids;
                                    self.canvas_state.current_tool = ActiveTool::Lasso;
                                    self.canvas_state.mesh_cache.clear();
                                    self.canvas_state.spatial_grid.rebuild(
                                        &self.library.notebooks[nb_idx].pages
                                            [self.current_page_index]
                                            .objects,
                                    );
                                    self.mark_dirty(&notebook_id);
                                    self.toast_message = Some(format!(
                                        "✅ Template {} disisipkan ({} objek).",
                                        kind.title(),
                                        added_count
                                    ));
                                    self.show_tool_popup = false;
                                }
                            }
                        });
                    }

                    // Opsi Spesifik Penghapus (Eraser Mode)
                    if self.canvas_state.current_tool == ActiveTool::Eraser {
                        ui.separator();
                        ui.label(
                            RichText::new("Mode Penghapus (Eraser Mode):")
                                .strong()
                                .size(12.0),
                        );
                        ui.horizontal(|ui| {
                            let modes = [
                                (EraserMode::Partial, "✂ Partial (Potong Segmen)"),
                                (EraserMode::WholeStroke, "⌫ Whole Stroke (Hapus Utuh)"),
                            ];
                            for (mode, label) in modes {
                                let is_sel = self.canvas_state.eraser_mode == mode;
                                if ui.selectable_label(is_sel, label).clicked() {
                                    self.canvas_state.eraser_mode = mode;
                                }
                            }
                        });
                    }

                    // Fase G4.4: Opsi Otomatis Koreksi Bentuk (Hold-to-Snap)
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui
                            .checkbox(
                                &mut self.settings.auto_shape_enabled,
                                lang.tr(
                                    "📐 Otomatis Koreksi Bentuk (Hold-to-Snap)",
                                    "📐 Auto-convert Shapes (Hold-to-Snap)",
                                ),
                            )
                            .changed()
                        {
                            self.canvas_state.auto_shape_enabled = self.settings.auto_shape_enabled;
                            let _ = save_settings_to_dir(&app_storage_dir(), &self.settings);
                        }
                    });

                    // Fase G11: Snapping Options (Smart Guides & Snap to Grid)
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.checkbox(
                            &mut self.canvas_state.smart_guides,
                            lang.tr(
                                "🧲 Garis Pemandu Magnetik (Smart Guides)",
                                "🧲 Smart Alignment Guides",
                            ),
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.checkbox(
                            &mut self.canvas_state.snap_to_grid,
                            lang.tr("▦ Tempel ke Kisi Kertas (Snap to Grid)", "▦ Snap to Grid"),
                        );
                    });

                    ui.add_space(6.0);
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui
                            .button(
                                lang.tr("⌨ Atur Pintasan Keyboard...", "⌨ Customize Shortcuts..."),
                            )
                            .clicked()
                        {
                            self.show_tool_popup = false;
                            self.show_shortcut_modal = true;
                        }
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.button("Selesai").clicked() {
                                self.show_tool_popup = false;
                            }
                        });
                    });
                });
        }

        // =====================================================================
        // GOODNOTES POP-UP 3: Authentic GoodNotes Right Pop-up Menu (`⋯`)
        // =====================================================================
        if self.show_more_menu_popup {
            let page_count = self.library.notebooks[nb_idx].pages.len();
            egui::Window::new(format!("Page {}", self.current_page_index + 1))
                .id(egui::Id::new("goodnotes_more_menu_popup"))
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::RIGHT_TOP, [-12.0, 50.0])
                .frame(
                    egui::Frame::window(&ctx.style())
                        .fill(Color32::from_rgb(26, 26, 28))
                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(62, 62, 66)))
                        .corner_radius(12.0)
                        .inner_margin(12.0),
                )
                .show(ctx, |ui| {
                    ui.set_width(275.0);
                    ui.spacing_mut().item_spacing.y = 5.0;

                    // --- Section 1: Page Actions (seperti GoodNotes) ---
                    let is_bm =
                        self.library.notebooks[nb_idx].pages[self.current_page_index].bookmarked;
                    if ui
                        .add(
                            Button::new(
                                RichText::new(if is_bm {
                                    "★ Remove Bookmark"
                                } else {
                                    "🔖 Bookmark Page"
                                })
                                .color(Color32::WHITE),
                            )
                            .fill(Color32::TRANSPARENT)
                            .min_size(Vec2::new(260.0, 26.0)),
                        )
                        .clicked()
                    {
                        self.library.notebooks[nb_idx].pages[self.current_page_index].bookmarked =
                            !is_bm;
                        self.mark_dirty(&notebook_id);
                        self.show_more_menu_popup = false;
                    }

                    if ui
                        .add(
                            Button::new(RichText::new("📄 Duplicate Page").color(Color32::WHITE))
                                .fill(Color32::TRANSPARENT)
                                .min_size(Vec2::new(260.0, 26.0)),
                        )
                        .clicked()
                    {
                        self.push_doc_undo(nb_idx);
                        if let Some(new_idx) =
                            self.library.notebooks[nb_idx].duplicate_page(self.current_page_index)
                        {
                            self.navigate_to_page(nb_idx, new_idx);
                            self.mark_dirty(&notebook_id);
                        }
                        self.show_more_menu_popup = false;
                    }

                    if ui
                        .add(
                            Button::new(
                                RichText::new("↻ Rotate Page 90° CW (Searah Jarum Jam)")
                                    .color(Color32::WHITE),
                            )
                            .fill(Color32::TRANSPARENT)
                            .min_size(Vec2::new(260.0, 26.0)),
                        )
                        .clicked()
                    {
                        self.push_doc_undo(nb_idx);
                        self.library.notebooks[nb_idx].rotate_page(self.current_page_index, 90);
                        self.canvas_state.mesh_cache.clear();
                        if let Some(p) = self.library.notebooks[nb_idx]
                            .pages
                            .get(self.current_page_index)
                        {
                            self.canvas_state.spatial_grid.rebuild(&p.objects);
                        }
                        self.mark_dirty(&notebook_id);
                        self.toast_message =
                            Some("↻ Halaman diputar 90° searah jarum jam.".to_string());
                        self.show_more_menu_popup = false;
                    }

                    if ui
                        .add(
                            Button::new(
                                RichText::new("↺ Rotate Page 90° CCW (Berlawanan Jarum Jam)")
                                    .color(Color32::WHITE),
                            )
                            .fill(Color32::TRANSPARENT)
                            .min_size(Vec2::new(260.0, 26.0)),
                        )
                        .clicked()
                    {
                        self.push_doc_undo(nb_idx);
                        self.library.notebooks[nb_idx].rotate_page(self.current_page_index, -90);
                        self.canvas_state.mesh_cache.clear();
                        if let Some(p) = self.library.notebooks[nb_idx]
                            .pages
                            .get(self.current_page_index)
                        {
                            self.canvas_state.spatial_grid.rebuild(&p.objects);
                        }
                        self.mark_dirty(&notebook_id);
                        self.toast_message =
                            Some("↺ Halaman diputar 90° berlawanan arah jarum jam.".to_string());
                        self.show_more_menu_popup = false;
                    }

                    if ui
                        .add(
                            Button::new(RichText::new("⟲ Rotate Page 180°").color(Color32::WHITE))
                                .fill(Color32::TRANSPARENT)
                                .min_size(Vec2::new(260.0, 26.0)),
                        )
                        .clicked()
                    {
                        self.push_doc_undo(nb_idx);
                        self.library.notebooks[nb_idx].rotate_page(self.current_page_index, 180);
                        self.canvas_state.mesh_cache.clear();
                        if let Some(p) = self.library.notebooks[nb_idx]
                            .pages
                            .get(self.current_page_index)
                        {
                            self.canvas_state.spatial_grid.rebuild(&p.objects);
                        }
                        self.mark_dirty(&notebook_id);
                        self.toast_message = Some("⟲ Halaman diputar 180°.".to_string());
                        self.show_more_menu_popup = false;
                    }

                    if ui
                        .add(
                            Button::new(
                                RichText::new("📑 Add Page to Outline...").color(Color32::WHITE),
                            )
                            .fill(Color32::TRANSPARENT)
                            .min_size(Vec2::new(260.0, 26.0)),
                        )
                        .clicked()
                    {
                        self.new_outline_title = format!("Halaman {}", self.current_page_index + 1);
                        self.show_add_outline_modal = true;
                        self.show_more_menu_popup = false;
                    }

                    if ui
                        .add(
                            Button::new(
                                RichText::new("📐 Change Template & Paper Size...")
                                    .color(Color32::WHITE),
                            )
                            .fill(Color32::TRANSPARENT)
                            .min_size(Vec2::new(260.0, 26.0)),
                        )
                        .clicked()
                    {
                        self.show_more_menu_popup = false;
                        self.open_page_resize_modal(nb_idx, self.current_page_index);
                    }

                    if ui
                        .add(
                            Button::new(
                                RichText::new("💾 Save Page as Custom Template...")
                                    .color(Color32::WHITE),
                            )
                            .fill(Color32::TRANSPARENT)
                            .min_size(Vec2::new(260.0, 26.0)),
                        )
                        .clicked()
                    {
                        self.show_more_menu_popup = false;
                        self.show_save_template_modal = true;
                        self.new_template_name =
                            format!("Template {}", self.current_page_index + 1);
                    }

                    ui.menu_button(
                        RichText::new("📊 Insert Starter Diagram ▶").color(Color32::WHITE),
                        |ui| {
                            for kind in crate::geometry::DiagramStarterKind::ALL {
                                if ui
                                    .button(kind.title())
                                    .on_hover_text(kind.description())
                                    .clicked()
                                {
                                    ui.close_menu();
                                    let cur_p = &self.library.notebooks[nb_idx].pages
                                        [self.current_page_index];
                                    let new_objs = crate::geometry::generate_diagram_starter(
                                        kind,
                                        cur_p.width,
                                        cur_p.height,
                                    );
                                    let added_count = new_objs.len();
                                    self.canvas_state.push_undo(&cur_p.id, &cur_p.objects);
                                    let new_ids =
                                        new_objs.iter().map(|o| o.id().to_string()).collect();
                                    self.library.notebooks[nb_idx].pages[self.current_page_index]
                                        .objects
                                        .extend(new_objs);
                                    self.canvas_state.selected_ids = new_ids;
                                    self.canvas_state.current_tool = ActiveTool::Lasso;
                                    self.canvas_state.mesh_cache.clear();
                                    self.canvas_state.spatial_grid.rebuild(
                                        &self.library.notebooks[nb_idx].pages
                                            [self.current_page_index]
                                            .objects,
                                    );
                                    self.mark_dirty(&notebook_id);
                                    self.toast_message = Some(format!(
                                        "✅ Template {} disisipkan ({} objek).",
                                        kind.title(),
                                        added_count
                                    ));
                                    self.show_more_menu_popup = false;
                                }
                            }
                        },
                    );

                    // Go to Page row (`Go to Page   ◀ 1 of N ▶`)
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("➡ Go to Page").color(Color32::WHITE));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if self.current_page_index + 1 < page_count
                                && ui.small_button("▶").clicked()
                            {
                                let next_idx = self.current_page_index + 1;
                                self.navigate_to_page(nb_idx, next_idx);
                            }
                            ui.label(
                                RichText::new(format!(
                                    "{} of {}",
                                    self.current_page_index + 1,
                                    page_count
                                ))
                                .size(11.5)
                                .color(Color32::GRAY),
                            );
                            if self.current_page_index > 0 && ui.small_button("◀").clicked() {
                                let prev_idx = self.current_page_index - 1;
                                self.navigate_to_page(nb_idx, prev_idx);
                            }
                        });
                    });

                    ui.separator();

                    // --- Section 2: Clear or Trash Page (GoodNotes Red Actions) ---
                    ui.label(
                        RichText::new("Clear or Trash Page")
                            .size(11.0)
                            .strong()
                            .color(Color32::GRAY),
                    );
                    if ui
                        .add(
                            Button::new(
                                RichText::new("🧹 Clear Page")
                                    .color(Color32::from_rgb(255, 99, 92)),
                            )
                            .fill(Color32::TRANSPARENT)
                            .min_size(Vec2::new(260.0, 24.0)),
                        )
                        .clicked()
                    {
                        if !self.library.notebooks[nb_idx].pages[self.current_page_index]
                            .objects
                            .is_empty()
                        {
                            self.push_doc_undo(nb_idx);
                            self.library.notebooks[nb_idx].pages[self.current_page_index]
                                .objects
                                .clear();
                            self.mark_dirty(&notebook_id);
                        }
                        self.show_more_menu_popup = false;
                    }

                    if page_count > 1
                        && ui
                            .add(
                                Button::new(
                                    RichText::new("🗑 Move Page to Trash")
                                        .color(Color32::from_rgb(255, 99, 92)),
                                )
                                .fill(Color32::TRANSPARENT)
                                .min_size(Vec2::new(260.0, 24.0)),
                            )
                            .clicked()
                    {
                        self.push_doc_undo(nb_idx);
                        if let Ok(new_idx) =
                            self.library.notebooks[nb_idx].delete_page(self.current_page_index)
                        {
                            self.navigate_to_page(nb_idx, new_idx);
                            self.mark_dirty(&notebook_id);
                        }
                        self.show_more_menu_popup = false;
                    }

                    ui.separator();

                    // --- Section 3: Fitur Lanjutan (Advanced Features) ---
                    ui.label(
                        RichText::new("Advanced Features")
                            .size(11.0)
                            .strong()
                            .color(Color32::GRAY),
                    );
                    if ui
                        .add(
                            Button::new(
                                RichText::new("🔍 Handwriting OCR (Ekstrak Teks)...")
                                    .color(Color32::WHITE),
                            )
                            .fill(Color32::TRANSPARENT)
                            .min_size(Vec2::new(260.0, 24.0)),
                        )
                        .clicked()
                    {
                        let p = &mut self.library.notebooks[nb_idx].pages[self.current_page_index];
                        let extracted =
                            crate::geometry::recognize_handwriting_from_strokes(&p.objects);
                        if !extracted.is_empty() {
                            p.ocr_text = extracted;
                            self.mark_dirty(&notebook_id);
                        }
                        self.show_ocr_modal = true;
                        self.show_more_menu_popup = false;
                    }

                    ui.horizontal(|ui| {
                        ui.label(RichText::new("✋ Palm Rejection").color(Color32::WHITE));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.checkbox(&mut self.settings.palm_rejection, "").changed() {
                                self.canvas_state.palm_rejection_enabled =
                                    self.settings.palm_rejection;
                                let _ = save_settings_to_dir(&app_storage_dir(), &self.settings);
                            }
                        });
                    });

                    ui.horizontal(|ui| {
                        ui.label(RichText::new("🔍 Zoom Window").color(Color32::WHITE));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.checkbox(&mut self.canvas_state.show_zoom_window, "");
                        });
                    });

                    ui.separator();

                    // --- Section 4: Settings (Scrolling Direction, Zoom, Theme, Language, Shortcuts) ---
                    ui.label(
                        RichText::new("Settings")
                            .size(11.0)
                            .strong()
                            .color(Color32::GRAY),
                    );

                    ui.horizontal(|ui| {
                        ui.label(RichText::new("📜 Page Mode").color(Color32::WHITE));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui
                                .selectable_label(
                                    self.view_mode == ViewMode::SinglePage,
                                    "Single Page",
                                )
                                .clicked()
                            {
                                self.set_view_mode(nb_idx, ViewMode::SinglePage);
                            }
                            if ui
                                .selectable_label(
                                    self.view_mode == ViewMode::ContinuousVertical,
                                    "Scrollable",
                                )
                                .clicked()
                            {
                                self.set_view_mode(nb_idx, ViewMode::ContinuousVertical);
                            }
                        });
                    });

                    ui.horizontal(|ui| {
                        ui.label(RichText::new("🔎 Canvas Zoom").color(Color32::WHITE));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.small_button("+").clicked() {
                                let z = (self.canvas_state.zoom + 0.1).min(2.5);
                                self.apply_zoom_anchored(nb_idx, z, None);
                            }
                            if ui
                                .small_button(format!(
                                    "{}%",
                                    (self.canvas_state.zoom * 100.0) as i32
                                ))
                                .on_hover_text("Klik untuk reset Zoom ke 100%")
                                .clicked()
                            {
                                self.apply_zoom_anchored(nb_idx, 1.0, None);
                                self.canvas_pan_offset = Vec2::ZERO;
                            }
                            if ui.small_button("-").clicked() {
                                let z = (self.canvas_state.zoom - 0.1).max(0.4);
                                self.apply_zoom_anchored(nb_idx, z, None);
                            }
                        });
                    });

                    ui.horizontal(|ui| {
                        ui.label(RichText::new("🎨 Theme").color(Color32::WHITE));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let theme_icon = match self.settings.theme_mode {
                                ThemeMode::Dark => "🌙 Dark",
                                ThemeMode::Light => "☀ Light",
                                ThemeMode::System => "💻 System",
                            };
                            if ui.small_button(theme_icon).clicked() {
                                self.settings.theme_mode = self.settings.theme_mode.next();
                                let _ = save_settings_to_dir(&app_storage_dir(), &self.settings);
                            }
                        });
                    });

                    ui.horizontal(|ui| {
                        ui.label(RichText::new("🌐 Language").color(Color32::WHITE));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.small_button(lang.badge()).clicked() {
                                self.settings.language = self.settings.language.toggle();
                                let _ = save_settings_to_dir(&app_storage_dir(), &self.settings);
                            }
                        });
                    });

                    if ui
                        .add(
                            Button::new(
                                RichText::new(lang.tr(
                                    "⌨ Atur Pintasan Keyboard (F1)",
                                    "⌨ Keyboard Shortcuts & Settings (F1)",
                                ))
                                .color(Color32::WHITE),
                            )
                            .fill(Color32::TRANSPARENT)
                            .min_size(Vec2::new(260.0, 24.0)),
                        )
                        .clicked()
                    {
                        self.show_more_menu_popup = false;
                        self.show_shortcut_modal = true;
                    }
                });
        }

        true
    }
}

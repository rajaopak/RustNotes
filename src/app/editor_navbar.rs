use super::state::RustNotesApp;
use super::types::ViewMode;
use crate::canvas::{
    ActiveTool, EraserMode, hsv_to_rgb, rainbow_laser_color, tessellate_laser_neon_beam,
};
use crate::model::{LaserColorMode, ShapeKind};
use crate::storage::{
    app_storage_dir, export_notebook_to_pdf, export_page_to_png, export_page_to_svg_file,
};
use eframe::egui::{
    self, Align, Button, Color32, FontId, Layout, Pos2, Rect, RichText, Sense, Stroke, StrokeKind,
    Vec2,
};

/// Menggambar ikon lingkaran pelangi (12 segmen spektrum HSV) untuk tombol/indikator mode Rainbow.
pub(super) fn paint_rainbow_circle(
    painter: &egui::Painter,
    center: Pos2,
    radius: f32,
    stroke: Stroke,
) {
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(center, Color32::WHITE);
    const SEGMENTS: usize = 12;
    for i in 0..=SEGMENTS {
        let frac = i as f32 / SEGMENTS as f32;
        let angle = frac * std::f32::consts::TAU;
        let (r, g, b) = hsv_to_rgb(frac * 360.0, 0.95, 1.0);
        let pos = center + Vec2::new(angle.cos(), angle.sin()) * radius;
        mesh.colored_vertex(pos, Color32::from_rgb(r, g, b));
    }
    for i in 0..SEGMENTS as u32 {
        mesh.add_triangle(0, 1 + i, 2 + i);
    }
    painter.add(mesh);
    painter.circle_stroke(center, radius, stroke);
}

/// Menggambar ikon penghapus (eraser) vektor dengan proporsi seimbang dan detail dua-bagian (rubber & sleeve).
fn paint_eraser_glyph(painter: &egui::Painter, rect: Rect, color: Color32) {
    let c = rect.center();
    // Kemiringan ikon penghapus sudut -40 derajat
    let cos = 0.7660_f32;
    let sin = -0.6428_f32;
    let u = Vec2::new(cos, sin); // sumbu panjang
    let v = Vec2::new(-sin, cos); // sumbu lebar

    let half_l = 6.4;
    let half_w = 3.6;

    // Titik sudut: Kiri-bawah (sleeve/selongsong) -> Kanan-atas (karet penghapus)
    let p_sleeve_back_t = c - u * half_l - v * half_w;
    let p_rubber_tip_t = c + u * half_l - v * half_w;
    let p_rubber_tip_b = c + u * half_l + v * half_w;
    let p_sleeve_back_b = c - u * half_l + v * half_w;

    // Garis batas karet vs gagang (karet 55% depan, gagang 45% belakang)
    let p_mid_t = c - u * 0.6 - v * half_w;
    let p_mid_b = c - u * 0.6 + v * half_w;

    // Kepala karet: poligon isi solid
    painter.add(egui::Shape::convex_polygon(
        vec![p_mid_t, p_rubber_tip_t, p_rubber_tip_b, p_mid_b],
        color,
        Stroke::NONE,
    ));

    // Selongsong / sleeve: poligon bergaris dengan isi transparan
    painter.add(egui::Shape::convex_polygon(
        vec![p_sleeve_back_t, p_mid_t, p_mid_b, p_sleeve_back_b],
        Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 60),
        Stroke::new(1.3_f32, color),
    ));

    // Garis aksen selongsong
    let p_stripe_t = c - u * 3.5 - v * (half_w * 0.75);
    let p_stripe_b = c - u * 3.5 + v * (half_w * 0.75);
    painter.line_segment([p_stripe_t, p_stripe_b], Stroke::new(1.0_f32, color));
}

impl RustNotesApp {
    /// Render navbar editor (bilah atas Goodnotes navy) beserta pop-up bawaannya (digunakan terpadu maupun oleh test).
    /// Mengembalikan `false` jika pengguna memilih kembali ke perpustakaan.
    pub fn render_editor_navbar(&mut self, ctx: &egui::Context, nb_idx: usize) -> bool {
        if !self.render_editor_navbar_bar(ctx, nb_idx) {
            return false;
        }
        self.render_navbar_popups(ctx, nb_idx);
        true
    }

    /// Render bilah atas editor (`TopBottomPanel::top`) tanpa merender pop-up overlay agar z-order
    /// pop-up dan Floating Quick Tool Bar tetap tertata bersih setelah `CentralPanel`.
    pub(crate) fn render_editor_navbar_bar(&mut self, ctx: &egui::Context, nb_idx: usize) -> bool {
        let Some(nb) = self.library.notebooks.get(nb_idx) else {
            return false;
        };
        if nb.pages.is_empty() {
            return false;
        }
        if self.current_page_index >= nb.pages.len() {
            self.current_page_index = nb.pages.len() - 1;
        }
        let notebook_id = nb.id.clone();
        let lang = self.settings.language;

        // Modern Floating GoodNotes Navy Navbar (#223e62) hovering above the workspace
        let mut return_to_library = false;
        let outer_panel_bg = ctx.style().visuals.panel_fill;

        egui::TopBottomPanel::top("editor_navbar")
            .frame(
                egui::Frame::NONE
                    .fill(outer_panel_bg)
                    .inner_margin(egui::Margin {
                        left: 12,
                        right: 12,
                        top: 6,
                        bottom: 6,
                    }),
            )
            .show(ctx, |ui| {
                let floating_frame = egui::Frame::NONE
                    .fill(Color32::from_rgb(34, 62, 98)) // Authentic GoodNotes Navy #223e62
                    .stroke(Stroke::new(
                        1.0_f32,
                        Color32::from_rgba_unmultiplied(75, 120, 175, 180),
                    )) // Crisp subtle border
                    .corner_radius(10.0) // Noticeably rounded, modern
                    .shadow(egui::epaint::Shadow {
                        offset: [0, 4],
                        blur: 14,
                        spread: 0,
                        color: Color32::from_rgba_unmultiplied(0, 0, 0, 75), // Smooth elevation shadow
                    })
                    .inner_margin(egui::Margin::symmetric(10, 5));

                floating_frame.show(ui, |ui| {
                    let total_w = ui.available_width();
                    let is_wide = total_w >= 1020.0;

                    // Baris Utama: Kiri (Navigasi & Judul), Tengah (Tools Center pada layar lebar), Paling Kanan (Settings & Page Actions)
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        ui.add_space(2.0);

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
                        .on_hover_text(self.action_hover_text(
                            crate::model::ShortcutAction::ToggleSidebar,
                            "Panel Halaman, Bookmark & Outline",
                            "Pages, Bookmarks & Outline Sidebar",
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
                        .on_hover_text(self.action_hover_text(
                            crate::model::ShortcutAction::Search,
                            "Cari di Dokumen",
                            "Find in Document",
                        ))
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
                        .on_hover_text(self.action_hover_text(
                            crate::model::ShortcutAction::ToggleZoomWindow,
                            "Jendela Pembesar Tulis / Zoom Window",
                            "Zoom Window for Detailed Writing",
                        ))
                        .clicked()
                    {
                        self.canvas_state.show_zoom_window = !self.canvas_state.show_zoom_window;
                    }

                    // Toggle Floating Quick Tool Bar (`🎛`)
                    let fb_btn = Button::new(
                        RichText::new("🎛").size(13.0).color(
                            if self.settings.floating_tool_bar_visible {
                                Color32::from_rgb(130, 195, 255)
                            } else {
                                Color32::WHITE
                            },
                        ),
                    )
                    .fill(if self.settings.floating_tool_bar_visible {
                        Color32::from_rgb(54, 95, 145)
                    } else {
                        Color32::TRANSPARENT
                    })
                    .corner_radius(6.0)
                    .min_size(Vec2::new(28.0, 28.0));
                    if ui
                        .add(fb_btn)
                        .on_hover_text(lang.tr(
                            "Tampilkan / Sembunyikan Floating Quick Tool Bar (4 sisi fleksibel)",
                            "Toggle Flexible 4-Sided Floating Quick Tool Bar",
                        ))
                        .clicked()
                    {
                        self.settings.floating_tool_bar_visible =
                            !self.settings.floating_tool_bar_visible;
                        if !self.settings.floating_tool_bar_visible {
                            self.floating_tool_bar_last_rect = None;
                        }
                        self.persist_settings();
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

                    let div_top = ui.cursor().top() + 4.0;
                    let div_bot = ui.cursor().bottom().max(div_top + 24.0) - 4.0;
                    let div_left_x = ui.cursor().min.x + 3.0;
                    ui.painter().line_segment(
                        [Pos2::new(div_left_x, div_top), Pos2::new(div_left_x, div_bot)],
                        Stroke::new(
                            1.0_f32,
                            Color32::from_rgba_unmultiplied(255, 255, 255, 45),
                        ),
                    );
                    ui.add_space(8.0);
                    let left_end_x = ui.cursor().min.x;

                    // --- 3. FAR RIGHT GROUP: Settings (`⚙`), Export (`📤`), Add Page (`📄+`), Paper Size (`📐`), Scroll Mode (`📜`) ---
                    let mut right_start_x = ui.max_rect().right();
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.add_space(4.0);
                        ui.spacing_mut().item_spacing.x = 6.0;

                        // GoodNotes Circular More Menu / Settings Button (`⚙`) — Paling Kanan
                        let more_btn = Button::new(
                            RichText::new("⚙")
                                .size(15.0)
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
                                "Pengaturan & Fitur Lanjutan (Settings)",
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
                                if ui
                                    .button(lang.tr(
                                        "📄 Ekspor Buku ke PDF",
                                        "📄 Export Notebook to PDF",
                                    ))
                                    .clicked()
                                {
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
                                                    lang.tr(
                                                        "✅ Dokumen PDF berhasil diekspor.",
                                                        "✅ PDF document exported successfully.",
                                                    )
                                                    .to_string(),
                                                );
                                            }
                                            Err(e) => {
                                                self.toast_message = Some(format!(
                                                    "{}: {e}",
                                                    lang.tr(
                                                        "⚠ Gagal ekspor PDF",
                                                        "⚠ Failed to export PDF"
                                                    )
                                                ));
                                            }
                                        }
                                    }
                                }
                                if ui
                                    .button(lang.tr(
                                        "🖼 Ekspor Halaman ke PNG",
                                        "🖼 Export Page to PNG",
                                    ))
                                    .clicked()
                                {
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
                                                    lang.tr(
                                                        "✅ Halaman berhasil diekspor ke PNG.",
                                                        "✅ Page exported to PNG.",
                                                    )
                                                    .to_string(),
                                                );
                                            }
                                            Err(e) => {
                                                self.toast_message = Some(format!(
                                                    "{}: {e}",
                                                    lang.tr(
                                                        "⚠ Gagal ekspor PNG",
                                                        "⚠ Failed to export PNG"
                                                    )
                                                ));
                                            }
                                        }
                                    }
                                }
                                if ui
                                    .button(lang.tr(
                                        "📐 Ekspor Halaman ke SVG (Vektor)",
                                        "📐 Export Page to SVG (Vector)",
                                    ))
                                    .clicked()
                                {
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
                                                    lang.tr(
                                                        "✅ Halaman berhasil diekspor ke SVG (Vektor).",
                                                        "✅ Page exported to SVG (Vector).",
                                                    )
                                                    .to_string(),
                                                );
                                            }
                                            Err(e) => {
                                                self.toast_message = Some(format!(
                                                    "{}: {e}",
                                                    lang.tr(
                                                        "⚠ Gagal ekspor SVG",
                                                        "⚠ Failed to export SVG"
                                                    )
                                                ));
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
                            self.render_editor_tools_strip(&mut center_ui, nb_idx);
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
                            self.render_editor_tools_strip(ui, nb_idx);
                        });
                    });
                }
            });
        });

        if return_to_library {
            let nb_id = notebook_id.clone();
            self.force_save_notebook(&nb_id);
            self.active_notebook_id = None;
            return false;
        }

        true
    }

    /// Render pop-up navbar (`Info Dokumen`, `Pengaturan Alat & Palet`, `More Menu`) pada lapisan atas
    /// setelah kanvas dan Floating Quick Tool Bar agar tidak pernah tertutup floating bar.
    pub(crate) fn render_navbar_popups(&mut self, ctx: &egui::Context, nb_idx: usize) {
        let Some(nb) = self.library.notebooks.get(nb_idx) else {
            return;
        };
        let notebook_id = nb.id.clone();
        self.render_doc_title_popup(ctx, nb_idx, &notebook_id);
        self.render_tool_popup(ctx, nb_idx, &notebook_id);
        self.render_more_menu_popup(ctx, nb_idx, &notebook_id);
    }

    fn render_doc_title_popup(&mut self, ctx: &egui::Context, nb_idx: usize, notebook_id: &str) {
        let lang = self.settings.language;
        let palette = super::theme::UiPalette::from_ctx(ctx, self.settings.theme_mode);
        // =====================================================================
        // GOODNOTES POP-UP 1: Document Title & Info Pop-up (`[Title] ▾`)
        // =====================================================================
        if self.show_doc_title_popup {
            egui::Window::new(lang.tr("📄 Info & Judul Dokumen", "📄 Document Title & Info"))
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::LEFT_TOP, [44.0, 56.0])
                .show(ctx, |ui| {
                    ui.set_width(290.0);
                    ui.label(
                        RichText::new(lang.tr("Judul Catatan:", "Notebook Title:"))
                            .size(12.0)
                            .color(palette.text_muted),
                    );
                    if ui
                        .text_edit_singleline(&mut self.library.notebooks[nb_idx].title)
                        .changed()
                    {
                        self.mark_dirty(notebook_id);
                    }
                    ui.add_space(6.0);
                    let page_cnt = self.library.notebooks[nb_idx].pages.len();
                    let cur_p = &self.library.notebooks[nb_idx].pages[self.current_page_index];
                    ui.label(
                        RichText::new(format!(
                            "{}: {} {} {} ({:.0} × {:.0} pt, {})",
                            lang.tr("Halaman Aktif", "Active Page"),
                            self.current_page_index + 1,
                            lang.tr("dari", "of"),
                            page_cnt,
                            cur_p.width,
                            cur_p.height,
                            cur_p.template.name_tr(lang)
                        ))
                        .size(11.5)
                        .color(palette.text_secondary),
                    );
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if ui
                            .button(
                                lang.tr("📐 Atur Ukuran Kertas...", "📐 Paper Size & Template..."),
                            )
                            .clicked()
                        {
                            self.show_doc_title_popup = false;
                            self.open_page_resize_modal(nb_idx, self.current_page_index);
                        }
                        if ui.button(lang.tr("Tutup", "Close")).clicked() {
                            self.show_doc_title_popup = false;
                        }
                    });
                });
        }
    }

    fn render_tool_popup(&mut self, ctx: &egui::Context, nb_idx: usize, notebook_id: &str) {
        let lang = self.settings.language;
        let palette = super::theme::UiPalette::from_ctx(ctx, self.settings.theme_mode);
        // =====================================================================
        // GOODNOTES POP-UP 2: Tool Settings Pop-up (Ketebalan, Warna, Bentuk, Eraser)
        // =====================================================================
        if self.show_tool_popup {
            let top_y = if self.settings.floating_tool_bar_visible
                && self.settings.floating_tool_bar_dock == crate::model::FloatingToolBarDock::Top
            {
                104.0
            } else {
                56.0
            };
            egui::Window::new(lang.tr("🎨 Pengaturan Alat & Palet", "🎨 Tool & Palette Settings"))
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_TOP, [0.0, top_y])
                .show(ctx, |ui| {
                    ui.set_width(360.0);

                    if self.canvas_state.current_tool == ActiveTool::Laser {
                        self.render_laser_tool_settings(ui, ctx);
                        ui.separator();
                    }

                    // Ketebalan Goresan (Stroke Width Presets + Slider)
                    ui.label(
                        RichText::new(lang.tr("Ketebalan Goresan:", "Stroke Width:"))
                            .strong()
                            .size(12.0),
                    );
                    ui.horizontal_wrapped(|ui| {
                        let mut remove_width_idx = None;
                        let widths = self.settings.pen_widths.clone();
                        for (idx, &w) in widths.iter().enumerate() {
                            let is_sel = (self.canvas_state.stroke_width - w).abs() < 0.1;
                            let btn =
                                Button::new(RichText::new(format!("{w:.1}px")).color(if is_sel {
                                    Color32::WHITE
                                } else {
                                    palette.btn_neutral_text
                                }))
                                .fill(if is_sel {
                                    palette.accent
                                } else {
                                    palette.btn_neutral_bg
                                })
                                .corner_radius(6.0);
                            let resp = ui.add(btn);
                            if resp.clicked() {
                                self.canvas_state.stroke_width = w;
                            }
                            if resp.secondary_clicked() && self.settings.pen_widths.len() > 1 {
                                remove_width_idx = Some(idx);
                            }
                            resp.on_hover_text(lang.tr(
                                "Klik kiri: pilih ketebalan | Klik kanan: hapus preset",
                                "Left-click: pick width | Right-click: remove preset",
                            ));
                        }
                        if let Some(rem) = remove_width_idx {
                            self.settings.pen_widths.remove(rem);
                            self.persist_settings();
                        }
                        if ui
                            .small_button("+")
                            .on_hover_text(lang.tr(
                                "Tambah preset ketebalan baru (+2.0px)",
                                "Add new width preset (+2.0px)",
                            ))
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
                                self.persist_settings();
                            }
                        }
                    });
                    ui.add(
                        egui::Slider::new(&mut self.canvas_state.stroke_width, 0.5..=28.0)
                            .text("px"),
                    );

                    ui.separator();

                    // Gaya Garis (Stroke Style: Solid, Dashed, DashDot, Dotted)
                    ui.label(
                        RichText::new(lang.tr("Gaya Garis:", "Stroke Style:"))
                            .strong()
                            .size(12.0),
                    );
                    ui.horizontal_wrapped(|ui| {
                        for style in crate::model::StrokeStyle::ALL {
                            let is_sel = self.canvas_state.stroke_style == style;
                            if ui.selectable_label(is_sel, style.name_tr(lang)).clicked() {
                                self.canvas_state.stroke_style = style;
                                if let Some(nb_id) = &self.active_notebook_id
                                    && let Some(nb) =
                                        self.library.notebooks.iter_mut().find(|n| &n.id == nb_id)
                                    && let Some(page) = nb.pages.get_mut(self.current_page_index)
                                {
                                    let mut changed = false;
                                    for obj in &mut page.objects {
                                        if self
                                            .canvas_state
                                            .selected_ids
                                            .contains(&obj.id().to_string())
                                            && let crate::model::DrawObject::Stroke {
                                                style: stroke_st,
                                                ..
                                            } = obj
                                        {
                                            *stroke_st = style;
                                            changed = true;
                                        }
                                    }
                                    if changed {
                                        self.canvas_state.mesh_cache.clear();
                                        self.canvas_state.push_undo(&page.id, &page.objects);
                                    }
                                }
                            }
                        }
                    });

                    ui.separator();

                    // Palet Warna (Color Swatches + Color Picker)
                    ui.label(
                        RichText::new(lang.tr("Palet Warna:", "Color Swatches:"))
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
                                Stroke::new(2.5_f32, palette.border_accent)
                            } else {
                                Stroke::new(1.0_f32, palette.border_strong)
                            };
                            ui.painter_at(rect)
                                .rect(rect, 12.0, c, stroke, StrokeKind::Outside);
                            if resp.clicked() {
                                self.canvas_state.stroke_color = c;
                            }
                            if resp.secondary_clicked() && self.settings.color_swatches.len() > 1 {
                                remove_swatch_idx = Some(idx);
                            }
                            resp.on_hover_text(lang.tr(
                                "Klik kiri: pilih warna | Klik kanan: hapus swatch",
                                "Left-click: pick color | Right-click: remove swatch",
                            ));
                        }
                        if let Some(rem) = remove_swatch_idx {
                            self.settings.color_swatches.remove(rem);
                            self.persist_settings();
                        }
                        ui.color_edit_button_srgba(&mut self.canvas_state.stroke_color);
                        if ui
                            .small_button(lang.tr("+ Simpan Warna", "+ Save Color"))
                            .on_hover_text(lang.tr(
                                "Simpan warna aktif ke palet swatch",
                                "Save active color to swatch palette",
                            ))
                            .clicked()
                        {
                            let new_sw: crate::model::ColorData =
                                self.canvas_state.stroke_color.into();
                            if !self.settings.color_swatches.contains(&new_sw) {
                                self.settings.color_swatches.push(new_sw);
                                self.persist_settings();
                            }
                        }
                    });

                    // Opsi Spesifik Bentuk Geometri (Shapes)
                    if matches!(self.canvas_state.current_tool, ActiveTool::Shape(_)) {
                        ui.separator();
                        ui.label(
                            RichText::new(lang.tr("Jenis Bentuk:", "Shape Type:"))
                                .strong()
                                .size(12.0),
                        );
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
                                (ShapeKind::Rectangle, lang.tr("▭ Persegi", "▭ Rectangle")),
                                (
                                    ShapeKind::Parallelogram,
                                    lang.tr("▱ Jajar Genjang", "▱ Parallelogram"),
                                ),
                                (
                                    ShapeKind::Ellipse,
                                    lang.tr("◯ Lingkaran / Elips", "◯ Circle / Ellipse"),
                                ),
                                (ShapeKind::Line, lang.tr("╱ Garis", "╱ Line")),
                                (
                                    ShapeKind::Triangle,
                                    lang.tr("△ Segitiga Sama Kaki", "△ Isosceles Triangle"),
                                ),
                                (
                                    ShapeKind::RightTriangle,
                                    lang.tr("◣ Segitiga Siku-Siku", "◣ Right Triangle"),
                                ),
                                (
                                    scalene_preset,
                                    lang.tr("◿ Segitiga Sembarang", "◿ Scalene Triangle"),
                                ),
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
                            RichText::new(lang.tr(
                                "📋 Template Diagram Siap Pakai:",
                                "📋 Starter Diagram Templates:",
                            ))
                            .strong()
                            .size(11.5),
                        );
                        ui.horizontal_wrapped(|ui| {
                            for kind in crate::geometry::DiagramStarterKind::ALL {
                                if ui
                                    .button(kind.title_tr(lang))
                                    .on_hover_text(kind.description_tr(lang))
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
                                    self.mark_dirty(notebook_id);
                                    self.toast_message = Some(format!(
                                        "✅ {} {} ({} {}).",
                                        lang.tr("Template", "Inserted"),
                                        kind.title_tr(lang),
                                        added_count,
                                        lang.tr("objek disisipkan", "objects")
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
                            RichText::new(lang.tr("Mode Penghapus:", "Eraser Mode:"))
                                .strong()
                                .size(12.0),
                        );
                        ui.horizontal(|ui| {
                            let modes = [
                                (
                                    EraserMode::Partial,
                                    lang.tr("✂ Parsial (Potong Segmen)", "✂ Partial Stroke"),
                                ),
                                (
                                    EraserMode::WholeStroke,
                                    lang.tr("⌫ Utuh (Hapus Objek)", "⌫ Whole Stroke"),
                                ),
                            ];
                            for (mode, label) in modes {
                                let is_sel = self.canvas_state.eraser_mode == mode;
                                if ui.selectable_label(is_sel, label).clicked() {
                                    self.canvas_state.eraser_mode = mode;
                                }
                            }
                        });
                    }

                    // Opsi Spesifik Lasso (Lasso Selection Mode - Task 6)
                    if self.canvas_state.current_tool == ActiveTool::Lasso {
                        ui.separator();
                        ui.label(
                            RichText::new(lang.tr("Mode Seleksi Lasso:", "Lasso Selection Mode:"))
                                .strong()
                                .size(12.0),
                        );
                        ui.horizontal(|ui| {
                            for mode in crate::model::LassoSelectionMode::ALL {
                                let is_sel = self.canvas_state.lasso_selection_mode == mode;
                                if ui.selectable_label(is_sel, mode.label_tr(lang)).clicked() {
                                    self.canvas_state.lasso_selection_mode = mode;
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
                            self.persist_settings();
                        }
                    });

                    // Fase G11: Snapping Options (Smart Guides & Snap to Grid)
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui
                            .checkbox(
                                &mut self.canvas_state.smart_guides,
                                lang.tr(
                                    "🧲 Garis Pemandu Magnetik (Smart Guides)",
                                    "🧲 Smart Alignment Guides",
                                ),
                            )
                            .changed()
                            && !self.canvas_state.smart_guides
                        {
                            self.canvas_state.active_alignment_guides.clear();
                        }
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
                            if ui.button(lang.tr("Selesai", "Done")).clicked() {
                                self.show_tool_popup = false;
                            }
                        });
                    });
                });
        }
    }

    fn render_more_menu_popup(&mut self, ctx: &egui::Context, nb_idx: usize, notebook_id: &str) {
        let lang = self.settings.language;
        let palette = super::theme::UiPalette::from_ctx(ctx, self.settings.theme_mode);
        // =====================================================================
        // GOODNOTES POP-UP 3: Authentic GoodNotes Right Pop-up Menu (`⚙` / More)
        // =====================================================================
        if self.show_more_menu_popup {
            let page_count = self.library.notebooks[nb_idx].pages.len();
            let right_x = if self.settings.floating_tool_bar_visible
                && self.settings.floating_tool_bar_dock == crate::model::FloatingToolBarDock::Right
            {
                -80.0
            } else {
                -12.0
            };
            let menu_btn = |ui: &mut egui::Ui, label: &str, col: Color32| -> bool {
                ui.add(
                    Button::new(RichText::new(label).color(col))
                        .fill(Color32::TRANSPARENT)
                        .min_size(Vec2::new(260.0, 25.0)),
                )
                .clicked()
            };

            egui::Window::new(format!(
                "{} {}",
                lang.tr("Halaman", "Page"),
                self.current_page_index + 1
            ))
            .id(egui::Id::new("goodnotes_more_menu_popup"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::RIGHT_TOP, [right_x, 56.0])
            .frame(
                egui::Frame::window(&ctx.style())
                    .fill(palette.bg_elevated)
                    .stroke(Stroke::new(1.0_f32, palette.border_subtle))
                    .corner_radius(12.0)
                    .inner_margin(12.0),
            )
            .show(ctx, |ui| {
                ui.set_width(275.0);
                ui.spacing_mut().item_spacing.y = 5.0;

                // --- Section 1: Page Actions ---
                let is_bm =
                    self.library.notebooks[nb_idx].pages[self.current_page_index].bookmarked;
                if menu_btn(
                    ui,
                    if is_bm {
                        lang.tr("★ Hapus Penanda Halaman", "★ Remove Bookmark")
                    } else {
                        lang.tr("🔖 Tandai Halaman", "🔖 Bookmark Page")
                    },
                    palette.text_primary,
                ) {
                    self.library.notebooks[nb_idx].pages[self.current_page_index].bookmarked =
                        !is_bm;
                    self.mark_dirty(notebook_id);
                    self.show_more_menu_popup = false;
                }

                if menu_btn(
                    ui,
                    lang.tr("📄 Duplikasi Halaman", "📄 Duplicate Page"),
                    palette.text_primary,
                ) {
                    self.push_doc_undo(nb_idx);
                    if let Some(new_idx) =
                        self.library.notebooks[nb_idx].duplicate_page(self.current_page_index)
                    {
                        self.navigate_to_page(nb_idx, new_idx);
                        self.mark_dirty(notebook_id);
                    }
                    self.show_more_menu_popup = false;
                }

                if menu_btn(
                    ui,
                    lang.tr(
                        "↻ Putar Halaman 90° Searah Jarum Jam",
                        "↻ Rotate Page 90° CW",
                    ),
                    palette.text_primary,
                ) {
                    self.push_doc_undo(nb_idx);
                    self.library.notebooks[nb_idx].rotate_page(self.current_page_index, 90);
                    self.canvas_state.mesh_cache.clear();
                    if let Some(p) = self.library.notebooks[nb_idx]
                        .pages
                        .get(self.current_page_index)
                    {
                        self.canvas_state.spatial_grid.rebuild(&p.objects);
                    }
                    self.mark_dirty(notebook_id);
                    self.toast_message = Some(
                        lang.tr(
                            "↻ Halaman diputar 90° searah jarum jam.",
                            "↻ Page rotated 90° clockwise.",
                        )
                        .to_string(),
                    );
                    self.show_more_menu_popup = false;
                }

                if menu_btn(
                    ui,
                    lang.tr(
                        "↺ Putar Halaman 90° Berlawanan Jarum Jam",
                        "↺ Rotate Page 90° CCW",
                    ),
                    palette.text_primary,
                ) {
                    self.push_doc_undo(nb_idx);
                    self.library.notebooks[nb_idx].rotate_page(self.current_page_index, -90);
                    self.canvas_state.mesh_cache.clear();
                    if let Some(p) = self.library.notebooks[nb_idx]
                        .pages
                        .get(self.current_page_index)
                    {
                        self.canvas_state.spatial_grid.rebuild(&p.objects);
                    }
                    self.mark_dirty(notebook_id);
                    self.toast_message = Some(
                        lang.tr(
                            "↺ Halaman diputar 90° berlawanan arah jarum jam.",
                            "↺ Page rotated 90° counter-clockwise.",
                        )
                        .to_string(),
                    );
                    self.show_more_menu_popup = false;
                }

                if menu_btn(
                    ui,
                    lang.tr("⟲ Putar Halaman 180°", "⟲ Rotate Page 180°"),
                    palette.text_primary,
                ) {
                    self.push_doc_undo(nb_idx);
                    self.library.notebooks[nb_idx].rotate_page(self.current_page_index, 180);
                    self.canvas_state.mesh_cache.clear();
                    if let Some(p) = self.library.notebooks[nb_idx]
                        .pages
                        .get(self.current_page_index)
                    {
                        self.canvas_state.spatial_grid.rebuild(&p.objects);
                    }
                    self.mark_dirty(notebook_id);
                    self.toast_message = Some(
                        lang.tr("⟲ Halaman diputar 180°.", "⟲ Page rotated 180°.")
                            .to_string(),
                    );
                    self.show_more_menu_popup = false;
                }

                if menu_btn(
                    ui,
                    lang.tr("📑 Tambah ke Daftar Isi...", "📑 Add Page to Outline..."),
                    palette.text_primary,
                ) {
                    self.new_outline_title = format!(
                        "{} {}",
                        lang.tr("Halaman", "Page"),
                        self.current_page_index + 1
                    );
                    self.show_add_outline_modal = true;
                    self.show_more_menu_popup = false;
                }

                if menu_btn(
                    ui,
                    lang.tr(
                        "📐 Ubah Template & Ukuran Kertas...",
                        "📐 Change Template & Paper Size...",
                    ),
                    palette.text_primary,
                ) {
                    self.show_more_menu_popup = false;
                    self.open_page_resize_modal(nb_idx, self.current_page_index);
                }

                if menu_btn(
                    ui,
                    lang.tr(
                        "💾 Simpan sebagai Template Kustom...",
                        "💾 Save Page as Custom Template...",
                    ),
                    palette.text_primary,
                ) {
                    self.show_more_menu_popup = false;
                    self.show_save_template_modal = true;
                    self.new_template_name = format!("Template {}", self.current_page_index + 1);
                }

                ui.menu_button(
                    RichText::new(
                        lang.tr("📊 Sisipkan Diagram Awal ▶", "📊 Insert Starter Diagram ▶"),
                    )
                    .color(palette.text_primary),
                    |ui| {
                        for kind in crate::geometry::DiagramStarterKind::ALL {
                            if ui
                                .button(kind.title_tr(lang))
                                .on_hover_text(kind.description_tr(lang))
                                .clicked()
                            {
                                ui.close_menu();
                                let cur_p =
                                    &self.library.notebooks[nb_idx].pages[self.current_page_index];
                                let new_objs = crate::geometry::generate_diagram_starter(
                                    kind,
                                    cur_p.width,
                                    cur_p.height,
                                );
                                let added_count = new_objs.len();
                                self.canvas_state.push_undo(&cur_p.id, &cur_p.objects);
                                let new_ids = new_objs.iter().map(|o| o.id().to_string()).collect();
                                self.library.notebooks[nb_idx].pages[self.current_page_index]
                                    .objects
                                    .extend(new_objs);
                                self.canvas_state.selected_ids = new_ids;
                                self.canvas_state.current_tool = ActiveTool::Lasso;
                                self.canvas_state.mesh_cache.clear();
                                self.canvas_state.spatial_grid.rebuild(
                                    &self.library.notebooks[nb_idx].pages[self.current_page_index]
                                        .objects,
                                );
                                self.mark_dirty(notebook_id);
                                self.toast_message = Some(format!(
                                    "✅ {} {} ({} {}).",
                                    lang.tr("Template", "Inserted"),
                                    kind.title_tr(lang),
                                    added_count,
                                    lang.tr("objek disisipkan", "objects")
                                ));
                                self.show_more_menu_popup = false;
                            }
                        }
                    },
                );

                // Go to Page row
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(lang.tr("➡ Lompat ke Halaman", "➡ Go to Page"))
                            .color(palette.text_primary),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if self.current_page_index + 1 < page_count
                            && ui.small_button("▶").clicked()
                        {
                            let next_idx = self.current_page_index + 1;
                            self.navigate_to_page(nb_idx, next_idx);
                        }
                        ui.label(
                            RichText::new(format!(
                                "{} {} {}",
                                self.current_page_index + 1,
                                lang.tr("dari", "of"),
                                page_count
                            ))
                            .size(11.5)
                            .color(palette.text_muted),
                        );
                        if self.current_page_index > 0 && ui.small_button("◀").clicked() {
                            let prev_idx = self.current_page_index - 1;
                            self.navigate_to_page(nb_idx, prev_idx);
                        }
                    });
                });

                ui.separator();

                // --- Section 2: Clear or Delete Page ---
                ui.label(
                    RichText::new(lang.tr("Bersihkan atau Hapus Halaman", "Clear or Delete Page"))
                        .size(11.0)
                        .strong()
                        .color(palette.text_muted),
                );
                if menu_btn(
                    ui,
                    lang.tr("🧹 Kosongkan Halaman Ini", "🧹 Clear Page"),
                    palette.danger,
                ) {
                    if !self.library.notebooks[nb_idx].pages[self.current_page_index]
                        .objects
                        .is_empty()
                    {
                        self.push_doc_undo(nb_idx);
                        self.library.notebooks[nb_idx].pages[self.current_page_index]
                            .objects
                            .clear();
                        self.mark_dirty(notebook_id);
                    }
                    self.show_more_menu_popup = false;
                }

                if page_count > 1
                    && menu_btn(
                        ui,
                        lang.tr("🗑 Hapus Halaman Ini", "🗑 Delete Current Page"),
                        palette.danger,
                    )
                {
                    self.push_doc_undo(nb_idx);
                    if let Ok(new_idx) =
                        self.library.notebooks[nb_idx].delete_page(self.current_page_index)
                    {
                        self.navigate_to_page(nb_idx, new_idx);
                        self.mark_dirty(notebook_id);
                    }
                    self.show_more_menu_popup = false;
                }

                ui.separator();

                // --- Section 3: Advanced Features ---
                ui.label(
                    RichText::new(lang.tr("Fitur Lanjutan", "Advanced Features"))
                        .size(11.0)
                        .strong()
                        .color(palette.text_muted),
                );
                if menu_btn(
                    ui,
                    lang.tr(
                        "🔍 OCR Tulisan Tangan (Ekstrak Teks)...",
                        "🔍 Handwriting OCR (Extract Text)...",
                    ),
                    palette.text_primary,
                ) {
                    let p = &mut self.library.notebooks[nb_idx].pages[self.current_page_index];
                    let extracted = crate::geometry::recognize_handwriting_from_strokes(&p.objects);
                    if !extracted.is_empty() {
                        p.ocr_text = extracted;
                        self.mark_dirty(notebook_id);
                    }
                    self.show_ocr_modal = true;
                    self.show_more_menu_popup = false;
                }

                ui.horizontal(|ui| {
                    ui.label(RichText::new("✋ Palm Rejection").color(palette.text_primary));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.checkbox(&mut self.settings.palm_rejection, "").changed() {
                            self.canvas_state.palm_rejection_enabled = self.settings.palm_rejection;
                            self.persist_settings();
                        }
                    });
                });

                ui.horizontal(|ui| {
                    ui.label(RichText::new("🔍 Zoom Window").color(palette.text_primary));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.checkbox(&mut self.canvas_state.show_zoom_window, "");
                    });
                });

                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(lang.tr("🎛 Floating Tool Bar", "🎛 Floating Tool Bar"))
                            .color(palette.text_primary),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .checkbox(&mut self.settings.floating_tool_bar_visible, "")
                            .changed()
                        {
                            if !self.settings.floating_tool_bar_visible {
                                self.floating_tool_bar_last_rect = None;
                            }
                            self.persist_settings();
                        }
                    });
                });

                ui.separator();

                // --- Section 4: Settings ---
                ui.label(
                    RichText::new(lang.tr("Pengaturan", "Settings"))
                        .size(11.0)
                        .strong()
                        .color(palette.text_muted),
                );

                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(lang.tr("📜 Mode Halaman", "📜 Page Mode"))
                            .color(palette.text_primary),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .selectable_label(
                                self.view_mode == ViewMode::SinglePage,
                                lang.tr("1 Halaman", "Single Page"),
                            )
                            .clicked()
                        {
                            self.set_view_mode(nb_idx, ViewMode::SinglePage);
                        }
                        if ui
                            .selectable_label(
                                self.view_mode == ViewMode::ContinuousVertical,
                                lang.tr("Scroll", "Scrollable"),
                            )
                            .clicked()
                        {
                            self.set_view_mode(nb_idx, ViewMode::ContinuousVertical);
                        }
                    });
                });

                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(lang.tr("🔎 Zoom Kanvas", "🔎 Canvas Zoom"))
                            .color(palette.text_primary),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.small_button("+").clicked() {
                            let z = (self.canvas_state.zoom + 0.1)
                                .min(crate::canvas::CanvasState::MAX_ZOOM);
                            self.apply_zoom_anchored(nb_idx, z, None);
                        }
                        if ui
                            .small_button(format!(
                                "{}%",
                                (self.canvas_state.zoom * 100.0).round() as i32
                            ))
                            .on_hover_text(self.action_hover_text(
                                crate::model::ShortcutAction::ResetZoom,
                                "Klik untuk reset Zoom ke 100%",
                                "Click to Reset Zoom to 100%",
                            ))
                            .clicked()
                        {
                            self.apply_zoom_anchored(nb_idx, 1.0, None);
                            self.canvas_pan_offset = Vec2::ZERO;
                        }
                        if ui.small_button("-").clicked() {
                            let z = (self.canvas_state.zoom - 0.1)
                                .max(crate::canvas::CanvasState::MIN_ZOOM);
                            self.apply_zoom_anchored(nb_idx, z, None);
                        }
                    });
                });

                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(lang.tr("🎨 Tema", "🎨 Theme")).color(palette.text_primary),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .small_button(self.settings.theme_mode.label_tr(lang))
                            .clicked()
                        {
                            self.settings.theme_mode = self.settings.theme_mode.next();
                            self.persist_settings();
                        }
                    });
                });

                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(lang.tr("🌐 Bahasa", "🌐 Language"))
                            .color(palette.text_primary),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.small_button(lang.badge()).clicked() {
                            self.settings.language = self.settings.language.toggle();
                            self.persist_settings();
                        }
                    });
                });

                if menu_btn(
                    ui,
                    lang.tr(
                        "⌨ Atur Pintasan Keyboard (F1)",
                        "⌨ Keyboard Shortcuts & Settings (F1)",
                    ),
                    palette.text_primary,
                ) {
                    self.show_more_menu_popup = false;
                    self.show_shortcut_modal = true;
                }
            });
        }
    }

    pub(crate) fn tool_shortcut_action(tool: ActiveTool) -> crate::model::ShortcutAction {
        use crate::model::ShortcutAction;
        match tool {
            ActiveTool::Pen => ShortcutAction::ToolPen,
            ActiveTool::Pencil => ShortcutAction::ToolPencil,
            ActiveTool::Highlighter => ShortcutAction::ToolHighlighter,
            ActiveTool::Eraser => ShortcutAction::ToolEraser,
            ActiveTool::Shape(_) => ShortcutAction::ToolShape,
            ActiveTool::Connector => ShortcutAction::ToolConnector,
            ActiveTool::Lasso => ShortcutAction::ToolLasso,
            ActiveTool::Text => ShortcutAction::ToolText,
            ActiveTool::StickyNote => ShortcutAction::ToolStickyNote,
            ActiveTool::Equation => ShortcutAction::ToolEquation,
            ActiveTool::Laser => ShortcutAction::ToolLaser,
            ActiveTool::Hand => ShortcutAction::ToolHand,
        }
    }

    pub(crate) fn action_hover_text(
        &self,
        action: crate::model::ShortcutAction,
        label_id: &str,
        label_en: &str,
    ) -> String {
        let lang = self.settings.language;
        let key = self.settings.shortcuts.get(action).display_string();
        format!("{} ({key})", lang.tr(label_id, label_en))
    }

    pub(crate) fn tool_hover_text(&self, tool: ActiveTool) -> String {
        let lang = self.settings.language;
        let key = self
            .settings
            .shortcuts
            .get(Self::tool_shortcut_action(tool))
            .display_string();
        match tool {
            ActiveTool::Pen => format!(
                "{} ({key}) — {}",
                lang.tr("Pena", "Pen"),
                lang.tr(
                    "Klik lagi untuk atur ketebalan & warna",
                    "Click again for width & color",
                )
            ),
            ActiveTool::Pencil => format!(
                "{} ({key}) — {}",
                lang.tr("Pensil", "Pencil"),
                lang.tr("Goresan pensil", "Pencil stroke")
            ),
            ActiveTool::Highlighter => format!(
                "{} ({key}) — {}",
                lang.tr("Highlighter", "Highlighter"),
                lang.tr("Stabilo transparan", "Transparent highlighter")
            ),
            ActiveTool::Eraser => format!(
                "{} ({key}) — {}",
                lang.tr("Penghapus", "Eraser"),
                lang.tr(
                    "Klik lagi untuk opsi Partial / Whole Stroke",
                    "Click again for Partial / Whole Stroke",
                )
            ),
            ActiveTool::Shape(_) => format!(
                "{} ({key}) — {}",
                lang.tr("Bentuk", "Shapes"),
                lang.tr("Klik lagi untuk pilih bentuk", "Click again to pick shape")
            ),
            ActiveTool::Connector => format!(
                "{} ({key}) — {}",
                lang.tr("Konektor", "Connector"),
                lang.tr(
                    "Tarik garis penghubung antar bentuk",
                    "Connect shapes with smart lines",
                )
            ),
            ActiveTool::Lasso => format!("{} ({key})", lang.tr("Seleksi Lasso", "Lasso Selection")),
            ActiveTool::Text => format!("{} ({key})", lang.tr("Kotak Teks", "Text Box")),
            ActiveTool::StickyNote => {
                format!("{} ({key})", lang.tr("Catatan Tempel", "Sticky Note"))
            }
            ActiveTool::Equation => format!(
                "{} ({key})",
                lang.tr("Alat Rumus / LaTeX", "Equation / LaTeX Tool")
            ),
            ActiveTool::Laser => format!("{} ({key})", lang.tr("Penunjuk Laser", "Laser Pointer")),
            ActiveTool::Hand => format!(
                "{} ({key})",
                lang.tr("Mode Tangan / Geser Kanvas", "Hand / Pan Mode")
            ),
        }
    }

    fn render_editor_tools_strip(&mut self, ui: &mut egui::Ui, nb_idx: usize) {
        let lang = self.settings.language;
        let active_shape_kind = match self.canvas_state.current_tool {
            ActiveTool::Shape(k) => k,
            _ => ShapeKind::Rectangle,
        };
        let tools = [
            (ActiveTool::Pen, "✒"),
            (ActiveTool::Pencil, "✏"),
            (ActiveTool::Highlighter, "🖍"),
            (ActiveTool::Eraser, "⌫"),
            (ActiveTool::Shape(active_shape_kind), "📐"),
            (ActiveTool::Connector, "🔗"),
            (ActiveTool::Lasso, "✂"),
            (ActiveTool::Text, "T"),
            (ActiveTool::StickyNote, "🗒"),
            (ActiveTool::Equation, "∑"),
            (ActiveTool::Laser, "✨"),
            (ActiveTool::Hand, "✋"),
        ];

        for (tool_var, icon) in tools {
            let tooltip = self.tool_hover_text(tool_var);
            let is_sel = match (self.canvas_state.current_tool, tool_var) {
                (ActiveTool::Shape(_), ActiveTool::Shape(_)) => true,
                (a, b) => a == b,
            };
            let icon_col = if is_sel {
                Color32::from_rgb(30, 58, 95)
            } else {
                Color32::WHITE
            };
            let btn = if tool_var == ActiveTool::Eraser {
                Button::new(" ")
            } else {
                Button::new(RichText::new(icon).size(14.0).strong().color(icon_col))
            }
            .fill(if is_sel {
                Color32::from_rgb(214, 232, 255)
            } else {
                Color32::TRANSPARENT
            })
            .corner_radius(7.0)
            .min_size(Vec2::new(28.0, 28.0));

            let resp = ui.add(btn).on_hover_text(tooltip);
            if tool_var == ActiveTool::Eraser {
                paint_eraser_glyph(ui.painter(), resp.rect, icon_col);
            }

            if resp.clicked() {
                if tool_var == ActiveTool::Equation {
                    self.canvas_state.current_tool = ActiveTool::Equation;
                    self.canvas_state.show_equation_dialog = true;
                    self.canvas_state.editing_equation_id = None;
                    self.canvas_state.equation_latex_buffer.clear();
                    self.canvas_state.equation_target_page_id = self.library.notebooks[nb_idx]
                        .pages
                        .get(self.current_page_index)
                        .map(|p| p.id.clone());
                    self.show_tool_popup = false;
                } else if is_sel {
                    self.show_tool_popup = !self.show_tool_popup;
                    self.show_more_menu_popup = false;
                    self.show_doc_title_popup = false;
                } else {
                    self.canvas_state.current_tool = tool_var;
                }
            }
        }

        // Insert Image Icon (`🖼`)
        let img_btn = Button::new(RichText::new("🖼").size(14.0).color(Color32::WHITE))
            .fill(Color32::TRANSPARENT)
            .corner_radius(7.0)
            .min_size(Vec2::new(28.0, 28.0));
        if ui
            .add(img_btn)
            .on_hover_text(lang.tr("Sisipkan Gambar ke Halaman", "Insert Image to Page"))
            .clicked()
            && let Some(path) = rfd::FileDialog::new()
                .add_filter("Images", &["png", "jpg", "jpeg", "webp"])
                .pick_file()
            && let Ok(bytes) = std::fs::read(&path)
        {
            let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("png");
            if self.insert_image_to_page(nb_idx, self.current_page_index, &bytes, ext, None) {
                self.toast_message = Some("✅ Gambar berhasil disisipkan ke halaman.".to_string());
            }
        }

        // Active Stroke & Color Pop-up Pill (`● 3.0px ▾`)
        let (sw_rect, sw_resp) = ui.allocate_exact_size(Vec2::new(74.0, 28.0), Sense::click());
        let pill_bg = if self.show_tool_popup || sw_resp.hovered() {
            Color32::from_rgb(54, 95, 145)
        } else {
            Color32::from_rgb(31, 56, 90)
        };
        ui.painter().rect_filled(sw_rect, 14.0, pill_bg);
        let dot_center = Pos2::new(sw_rect.left() + 14.0, sw_rect.center().y);
        if self.canvas_state.current_tool == ActiveTool::Laser {
            if self.canvas_state.laser_color_mode == LaserColorMode::Rainbow {
                paint_rainbow_circle(
                    ui.painter(),
                    dot_center,
                    7.0,
                    Stroke::new(1.5_f32, Color32::WHITE),
                );
            } else {
                ui.painter().circle(
                    dot_center,
                    7.0,
                    self.canvas_state.laser_color,
                    Stroke::new(1.5_f32, Color32::WHITE),
                );
            }
            let pill_label = if self.canvas_state.laser_color_mode == LaserColorMode::Rainbow {
                "RGB ▾"
            } else {
                "Laser ▾"
            };
            ui.painter().text(
                Pos2::new(sw_rect.left() + 26.0, sw_rect.center().y),
                egui::Align2::LEFT_CENTER,
                pill_label,
                FontId::proportional(11.5),
                Color32::WHITE,
            );
        } else {
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
        }
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

    /// Merender pengaturan khusus Penunjuk Laser (mode Solid/Rainbow, pratinjau langsung, preset & picker warna).
    fn render_laser_tool_settings(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let lang = self.settings.language;
        ui.label(
            RichText::new(lang.tr(
                "✨ Warna Penunjuk Laser (Laser Pointer):",
                "✨ Laser Pointer Color:",
            ))
            .strong()
            .size(12.0),
        );

        ui.horizontal(|ui| {
            let modes = [
                (
                    LaserColorMode::Solid,
                    lang.tr("● Warna Solid", "● Solid Color"),
                ),
                (
                    LaserColorMode::Rainbow,
                    lang.tr("🌈 Rainbow (Pelangi)", "🌈 Rainbow"),
                ),
            ];
            for (mode, label) in modes {
                let is_sel = self.canvas_state.laser_color_mode == mode;
                if ui.selectable_label(is_sel, label).clicked() {
                    self.settings.laser_color_mode = mode;
                    self.canvas_state.laser_color_mode = mode;
                    self.persist_settings();
                }
            }
        });

        // Pratinjau efek neon laser langsung pada strip gelap
        let (preview_rect, _) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), 28.0), Sense::hover());
        let p = ui.painter_at(preview_rect);
        p.rect_filled(preview_rect, 6.0, Color32::from_rgb(20, 22, 28));
        p.rect_stroke(
            preview_rect,
            6.0,
            Stroke::new(1.0_f32, Color32::from_rgb(55, 60, 72)),
            StrokeKind::Inside,
        );
        let pad_x = 18.0;
        let x0 = preview_rect.left() + pad_x;
        let x1 = (preview_rect.right() - pad_x).max(x0 + 20.0);
        let cy = preview_rect.center().y;
        let preview_pts: Vec<Pos2> = (0..=16)
            .map(|i| {
                let t = i as f32 / 16.0;
                let x = x0 + (x1 - x0) * t;
                let y = cy + (t * std::f32::consts::TAU).sin() * 5.0;
                Pos2::new(x, y)
            })
            .collect();
        let time_secs = ctx.input(|i| i.time) as f32;
        let preview_mesh = match self.canvas_state.laser_color_mode {
            LaserColorMode::Solid => {
                let c = self.canvas_state.laser_color;
                let rgb = (c.r(), c.g(), c.b());
                tessellate_laser_neon_beam(&preview_pts, 0.72, 1.0, |_| rgb)
            }
            LaserColorMode::Rainbow => {
                ctx.request_repaint();
                tessellate_laser_neon_beam(&preview_pts, 0.72, 1.0, |arc| {
                    let c = rainbow_laser_color(arc, time_secs, 255);
                    (c.r(), c.g(), c.b())
                })
            }
        };
        p.add(preview_mesh);

        ui.add_space(4.0);
        let neon_presets: [(Color32, &str); 8] = [
            (Color32::from_rgb(255, 0, 35), "Neon Red"),
            (Color32::from_rgb(0, 212, 255), "Electric Cyan"),
            (Color32::from_rgb(24, 232, 90), "Neon Green"),
            (Color32::from_rgb(255, 213, 0), "Solar Yellow"),
            (Color32::from_rgb(176, 66, 255), "Neon Purple"),
            (Color32::from_rgb(255, 46, 184), "Hot Pink"),
            (Color32::from_rgb(255, 106, 0), "Vivid Orange"),
            (Color32::from_rgb(41, 121, 255), "Royal Blue"),
        ];

        ui.horizontal_wrapped(|ui| {
            for (preset_col, preset_name) in neon_presets {
                let (rect, resp) = ui.allocate_exact_size(Vec2::new(24.0, 24.0), Sense::click());
                let is_sel = self.canvas_state.laser_color_mode == LaserColorMode::Solid
                    && self.canvas_state.laser_color == preset_col;
                let stroke = if is_sel {
                    Stroke::new(2.5_f32, Color32::from_rgb(76, 147, 236))
                } else {
                    Stroke::new(1.0_f32, Color32::from_rgb(100, 100, 100))
                };
                ui.painter_at(rect)
                    .rect(rect, 12.0, preset_col, stroke, StrokeKind::Outside);
                if resp.on_hover_text(preset_name).clicked() {
                    self.canvas_state.laser_color = preset_col;
                    self.canvas_state.laser_color_mode = LaserColorMode::Solid;
                    self.settings.laser_color = preset_col.into();
                    self.settings.laser_color_mode = LaserColorMode::Solid;
                    self.persist_settings();
                }
            }

            // Swatch cepat untuk mode Rainbow
            let (rb_rect, rb_resp) = ui.allocate_exact_size(Vec2::new(24.0, 24.0), Sense::click());
            let is_rb = self.canvas_state.laser_color_mode == LaserColorMode::Rainbow;
            let rb_stroke = if is_rb {
                Stroke::new(2.5_f32, Color32::from_rgb(76, 147, 236))
            } else {
                Stroke::new(1.0_f32, Color32::from_rgb(100, 100, 100))
            };
            paint_rainbow_circle(&ui.painter_at(rb_rect), rb_rect.center(), 10.5, rb_stroke);
            if rb_resp
                .on_hover_text(lang.tr("Mode Rainbow (Pelangi)", "Rainbow Spectrum Mode"))
                .clicked()
            {
                self.canvas_state.laser_color_mode = LaserColorMode::Rainbow;
                self.settings.laser_color_mode = LaserColorMode::Rainbow;
                self.persist_settings();
            }

            let mut custom_laser = self.canvas_state.laser_color;
            if ui
                .color_edit_button_srgba(&mut custom_laser)
                .on_hover_text(lang.tr("Pilih warna laser kustom", "Pick custom laser color"))
                .changed()
            {
                self.canvas_state.laser_color = custom_laser;
                self.canvas_state.laser_color_mode = LaserColorMode::Solid;
                self.settings.laser_color = custom_laser.into();
                self.settings.laser_color_mode = LaserColorMode::Solid;
                self.persist_settings();
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{KeyBinding, ShortcutAction};

    #[test]
    fn test_all_tool_and_navbar_hover_tooltips_follow_custom_keybinds() {
        let mut app = RustNotesApp::default();

        let all_tools = [
            (ActiveTool::Pen, "(P)"),
            (ActiveTool::Pencil, "(B)"),
            (ActiveTool::Highlighter, "(H)"),
            (ActiveTool::Eraser, "(E)"),
            (ActiveTool::Shape(ShapeKind::Rectangle), "(S)"),
            (ActiveTool::Connector, "(C)"),
            (ActiveTool::Lasso, "(L)"),
            (ActiveTool::Text, "(T)"),
            (ActiveTool::StickyNote, "(N)"),
            (ActiveTool::Equation, "(Alt + E)"),
            (ActiveTool::Laser, "(R)"),
            (ActiveTool::Hand, "(Space)"),
        ];

        for (tool, expected_key) in all_tools {
            let tip = app.tool_hover_text(tool);
            assert!(
                tip.contains(expected_key),
                "Tooltip '{tip}' for {tool:?} must contain default keybind '{expected_key}'"
            );
        }

        // Rebind several tools and navbar actions -> hover placeholders must update immediately
        app.settings
            .shortcuts
            .set(ShortcutAction::ToolPen, KeyBinding::plain("Q"));
        app.settings
            .shortcuts
            .set(ShortcutAction::ToolPencil, KeyBinding::plain("K"));
        app.settings
            .shortcuts
            .set(ShortcutAction::ToolLaser, KeyBinding::plain("J"));
        app.settings
            .shortcuts
            .set(ShortcutAction::ToolHand, KeyBinding::plain("M"));
        app.settings
            .shortcuts
            .set(ShortcutAction::ToolEquation, KeyBinding::alt("Q"));
        app.settings
            .shortcuts
            .set(ShortcutAction::ToggleSidebar, KeyBinding::ctrl("B"));
        app.settings
            .shortcuts
            .set(ShortcutAction::Search, KeyBinding::ctrl("K"));
        app.settings
            .shortcuts
            .set(ShortcutAction::ToggleZoomWindow, KeyBinding::alt("W"));
        app.settings
            .shortcuts
            .set(ShortcutAction::ResetZoom, KeyBinding::ctrl("1"));

        let pen_tip = app.tool_hover_text(ActiveTool::Pen);
        assert!(pen_tip.contains("(Q)") && !pen_tip.contains("(P)"));

        let pencil_tip = app.tool_hover_text(ActiveTool::Pencil);
        assert!(pencil_tip.contains("(K)") && !pencil_tip.contains("(B)"));

        let laser_tip = app.tool_hover_text(ActiveTool::Laser);
        assert!(laser_tip.contains("(J)") && !laser_tip.contains("(R)"));

        let hand_tip = app.tool_hover_text(ActiveTool::Hand);
        assert!(hand_tip.contains("(M)") && !hand_tip.contains("(Space)"));

        let eq_tip = app.tool_hover_text(ActiveTool::Equation);
        assert!(eq_tip.contains("(Alt + Q)") && !eq_tip.contains("(Alt + E)"));

        let sb_tip = app.action_hover_text(ShortcutAction::ToggleSidebar, "Panel", "Sidebar");
        assert!(sb_tip.contains("(Ctrl + B)") && !sb_tip.contains("(F6)"));

        let find_tip = app.action_hover_text(ShortcutAction::Search, "Cari", "Find");
        assert!(find_tip.contains("(Ctrl + K)") && !find_tip.contains("(Ctrl + F)"));

        let zw_tip = app.action_hover_text(ShortcutAction::ToggleZoomWindow, "Zoom", "Zoom");
        assert!(zw_tip.contains("(Alt + W)") && !zw_tip.contains("(Alt + Z)"));

        let rz_tip = app.action_hover_text(ShortcutAction::ResetZoom, "Reset", "Reset");
        assert!(rz_tip.contains("(Ctrl + 1)") && !rz_tip.contains("(Ctrl + 0)"));
    }

    #[test]
    fn test_laser_navbar_and_popup_render_for_solid_and_rainbow_modes() {
        let mut app = RustNotesApp::default();
        let nb = crate::model::Notebook::new(
            "Laser Test Notebook",
            crate::model::PageTemplate::Blank,
            Color32::WHITE,
        );
        app.active_notebook_id = Some(nb.id.clone());
        app.library.notebooks = vec![nb];
        app.canvas_state.current_tool = ActiveTool::Laser;
        app.show_tool_popup = true;

        // Solid custom color mode
        let cyan = Color32::from_rgb(0, 212, 255);
        app.settings.laser_color = cyan.into();
        app.settings.laser_color_mode = LaserColorMode::Solid;
        app.canvas_state.laser_color = cyan;
        app.canvas_state.laser_color_mode = LaserColorMode::Solid;

        let ctx = egui::Context::default();
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
                ..Default::default()
            },
            |ctx| {
                assert!(app.render_editor_navbar(ctx, 0));
            },
        );
        assert_eq!(app.canvas_state.laser_color, cyan);
        assert_eq!(app.canvas_state.laser_color_mode, LaserColorMode::Solid);

        // Rainbow mode
        app.settings.laser_color_mode = LaserColorMode::Rainbow;
        app.canvas_state.laser_color_mode = LaserColorMode::Rainbow;
        let out_rb = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
                ..Default::default()
            },
            |ctx| {
                assert!(app.render_editor_navbar(ctx, 0));
            },
        );
        assert!(!out_rb.shapes.is_empty());
        assert_eq!(app.canvas_state.laser_color_mode, LaserColorMode::Rainbow);
    }
}

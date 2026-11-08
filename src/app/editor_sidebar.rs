use super::state::RustNotesApp;
use super::types::SidebarTab;
use crate::model::{Page, PageTemplate, ShapeKind};
use crate::storage::{app_storage_dir, export_notebook_to_pdf};
use eframe::egui::{
    self, Align, Button, Color32, FontId, Layout, Pos2, Rect, RichText, ScrollArea, Sense, Stroke,
    StrokeKind, Vec2,
};

impl RustNotesApp {
    /// Render sidebar kiri editor (Thumbnail galeri halaman, drag-drop reordering, bookmarks, PDF outline).
    pub fn render_editor_sidebar(&mut self, ctx: &egui::Context, nb_idx: usize) {
        let notebook_id = self.library.notebooks[nb_idx].id.clone();
        if self.show_thumbnail_sidebar {
            egui::SidePanel::left("editor_thumbnail_sidebar")
                .exact_width(185.0)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.add_space(8.0);

                    // Sidebar Tab Selector (Pages, Bookmarks, Outline)
                    ui.horizontal(|ui| {
                        let tabs = [
                            (SidebarTab::Thumbnails, "📄 Pages"),
                            (SidebarTab::Bookmarks, "🔖 Marks"),
                            (SidebarTab::Outline, "📑 Outline"),
                        ];
                        for (tab, label) in tabs {
                            let is_sel = self.sidebar_tab == tab;
                            if ui
                                .selectable_label(is_sel, RichText::new(label).size(11.0))
                                .clicked()
                            {
                                self.sidebar_tab = tab;
                            }
                        }
                    });
                    ui.separator();

                    match self.sidebar_tab {
                        SidebarTab::Thumbnails => {
                            let page_count = self.library.notebooks[nb_idx].pages.len();
                            ui.horizontal(|ui| {
                                if self.thumb_select_mode {
                                    ui.label(
                                        RichText::new(format!(
                                            "{} dipilih",
                                            self.thumb_selected_pages.len()
                                        ))
                                        .strong()
                                        .color(Color32::WHITE),
                                    );
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        if ui.button("✕").on_hover_text("Batal mode seleksi").clicked() {
                                            self.thumb_select_mode = false;
                                            self.thumb_selected_pages.clear();
                                        }
                                        if ui.button("Semua").on_hover_text("Pilih semua halaman").clicked() {
                                            for i in 0..page_count {
                                                self.thumb_selected_pages.insert(i);
                                            }
                                        }
                                    });
                                } else {
                                    ui.label(RichText::new("Thumbnails").strong().color(Color32::WHITE));
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        if ui.button("➕ Add").on_hover_text("Atur & Tambah Halaman Baru").clicked() {
                                            self.open_add_page_modal(nb_idx);
                                        }
                                        if ui.button("☑ Pilih").on_hover_text("Pilih multi halaman untuk aksi massal").clicked() {
                                            self.thumb_select_mode = true;
                                            self.thumb_selected_pages.clear();
                                            self.thumb_selected_pages.insert(self.current_page_index);
                                        }
                                    });
                                }
                            });

                            // Bulk Actions Toolbar jika dalam mode multi-select
                            if self.thumb_select_mode && !self.thumb_selected_pages.is_empty() {
                                ui.add_space(4.0);
                                ui.horizontal_wrapped(|ui| {
                                    ui.spacing_mut().item_spacing.x = 4.0;
                                    // 1. Bulk Delete (hanya jika sisa minimal 1 halaman)
                                    if self.thumb_selected_pages.len() < page_count
                                        && ui.button("🗑 Hapus").on_hover_text("Hapus halaman terpilih").clicked()
                                    {
                                        self.push_doc_undo(nb_idx);
                                        let to_delete: Vec<usize> =
                                            self.thumb_selected_pages.iter().copied().rev().collect();
                                        for idx in to_delete {
                                            self.library.notebooks[nb_idx].pages.remove(idx);
                                        }
                                        self.thumb_selected_pages.clear();
                                        self.thumb_select_mode = false;
                                        self.current_page_index = self
                                            .current_page_index
                                            .min(self.library.notebooks[nb_idx].pages.len().saturating_sub(1));
                                        self.mark_dirty(&notebook_id);
                                        self.toast_message =
                                            Some("🗑 Halaman terpilih berhasil dihapus.".to_string());
                                    }

                                    // 2. Bulk Duplicate
                                    if ui.button("📄 Salin").on_hover_text("Gandakan halaman terpilih").clicked() {
                                        self.push_doc_undo(nb_idx);
                                        let to_dup: Vec<usize> =
                                            self.thumb_selected_pages.iter().copied().collect();
                                        for (offset, idx) in to_dup.into_iter().enumerate() {
                                            let page_to_copy = self.library.notebooks[nb_idx].pages[idx + offset].clone();
                                            self.library.notebooks[nb_idx]
                                                .pages
                                                .insert(idx + offset + 1, page_to_copy);
                                        }
                                        self.thumb_selected_pages.clear();
                                        self.thumb_select_mode = false;
                                        self.mark_dirty(&notebook_id);
                                        self.toast_message =
                                            Some("📄 Halaman terpilih berhasil digandakan.".to_string());
                                    }

                                    // 3. Bulk Rotate 90 CW
                                    if ui.button("↻ Putar").on_hover_text("Putar halaman terpilih 90° CW").clicked() {
                                        self.push_doc_undo(nb_idx);
                                        for &idx in &self.thumb_selected_pages {
                                            self.library.notebooks[nb_idx].rotate_page(idx, 90);
                                        }
                                        self.canvas_state.mesh_cache.clear();
                                        if let Some(p) = self.library.notebooks[nb_idx].pages.get(self.current_page_index) {
                                            self.canvas_state.spatial_grid.rebuild(&p.objects);
                                        }
                                        self.mark_dirty(&notebook_id);
                                        self.toast_message =
                                            Some("↻ Halaman terpilih berhasil diputar 90°.".to_string());
                                    }

                                    // 4. Bulk Export to PDF
                                    if ui.button("📤 Ekspor").on_hover_text("Ekspor halaman terpilih ke PDF").clicked() {
                                        let default_name = format!(
                                            "{}_selected.pdf",
                                            self.library.notebooks[nb_idx].title.replace(
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
                                            let mut temp_nb = self.library.notebooks[nb_idx].clone();
                                            temp_nb.pages = self
                                                .thumb_selected_pages
                                                .iter()
                                                .filter_map(|&i| {
                                                    self.library.notebooks[nb_idx].pages.get(i).cloned()
                                                })
                                                .collect();
                                            match export_notebook_to_pdf(&storage_dir, &temp_nb, &path) {
                                                Ok(()) => {
                                                    self.toast_message = Some(
                                                        "✅ Halaman terpilih berhasil diekspor ke PDF."
                                                            .to_string(),
                                                    );
                                                }
                                                Err(e) => {
                                                    self.toast_message =
                                                        Some(format!("⚠ Gagal ekspor PDF: {e}"));
                                                }
                                            }
                                        }
                                    }
                                });
                            }
                            ui.separator();

                            ScrollArea::vertical()
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    let mut action_select = None;
                                    let mut action_duplicate = None;
                                    let mut action_delete = None;
                                    let mut action_move_up = None;
                                    let mut action_move_down = None;
                                    let mut action_insert_before = None;
                                    let mut action_insert_after = None;
                                    let mut action_change_size = None;
                                    let mut action_rotate_cw = None;
                                    let mut action_rotate_ccw = None;
                                    let mut action_rotate_180 = None;
                                    let mut action_move_drag = None;
                                    let page_count = self.library.notebooks[nb_idx].pages.len();
                                    self.canvas_state.active_notebook_id = Some(notebook_id.clone());

                                    let pointer_released = ui.input(|i| i.pointer.any_released());

                                    for p_i in 0..page_count {
                                        // Pre-cache PDF/Image background & Image object textures for this thumbnail
                                        let mut needed_assets: Vec<String> = Vec::new();
                                        {
                                            let p_ref = &self.library.notebooks[nb_idx].pages[p_i];
                                            if let crate::model::PageBackground::Image { asset_id }
                                            | crate::model::PageBackground::Pdf {
                                                asset_id, ..
                                            } = p_ref.effective_background()
                                            {
                                                needed_assets.push(asset_id);
                                            }
                                            for obj in &p_ref.objects {
                                                if let crate::model::DrawObject::Image {
                                                    asset_id,
                                                    ..
                                                } = obj
                                                {
                                                    needed_assets.push(asset_id.clone());
                                                }
                                            }
                                        }
                                        for asset_id in needed_assets {
                                            self.canvas_state.get_or_load_texture(ui.ctx(), &asset_id);
                                        }

                                        let p = &self.library.notebooks[nb_idx].pages[p_i];
                                        let is_active = p_i == self.current_page_index;
                                        ui.vertical_centered(|ui| {
                                            let thumb_w = 110.0;
                                            let ratio = p.height / p.width.max(1.0);
                                            let thumb_h = (thumb_w * ratio).clamp(60.0, 160.0);

                                            let (rect, resp) = ui.allocate_exact_size(
                                                Vec2::new(thumb_w, thumb_h),
                                                Sense::click_and_drag(),
                                            );
                                            let painter = ui.painter_at(rect);

                                            // Deteksi Drag-and-Drop Reorder Halaman
                                            if resp.drag_started() && !self.thumb_select_mode {
                                                self.dragged_thumb_page = Some(p_i);
                                            }

                                            // Visual Drop Indicator Line saat halaman sedang di-drag
                                            if let Some(from_i) = self.dragged_thumb_page {
                                                if from_i != p_i && resp.hovered() {
                                                    let hover_y = ui
                                                        .input(|i| i.pointer.hover_pos())
                                                        .map_or(rect.center().y, |p| p.y);
                                                    let is_top = hover_y < rect.center().y;
                                                    let line_y = if is_top { rect.top() } else { rect.bottom() };
                                                    let indicator_stroke =
                                                        Stroke::new(3.0_f32, Color32::from_rgb(0, 114, 206));
                                                    painter.line_segment(
                                                        [
                                                            Pos2::new(rect.left(), line_y),
                                                            Pos2::new(rect.right(), line_y),
                                                        ],
                                                        indicator_stroke,
                                                    );
                                                    painter.circle_filled(
                                                        Pos2::new(rect.left() + 2.0, line_y),
                                                        4.0,
                                                        Color32::from_rgb(0, 114, 206),
                                                    );
                                                    painter.circle_filled(
                                                        Pos2::new(rect.right() - 2.0, line_y),
                                                        4.0,
                                                        Color32::from_rgb(0, 114, 206),
                                                    );
                                                }

                                                if pointer_released && resp.hovered() && from_i != p_i {
                                                    let hover_y = ui
                                                        .input(|i| i.pointer.hover_pos())
                                                        .map_or(rect.center().y, |p| p.y);
                                                    let target_pos = if hover_y < rect.center().y {
                                                        p_i
                                                    } else {
                                                        (p_i + 1).min(page_count.saturating_sub(1))
                                                    };
                                                    action_move_drag = Some((from_i, target_pos));
                                                }
                                            }

                                            // 1. Draw miniature paper base color
                                            painter.rect_filled(rect, 3.0, p.paper_color.to_color32());

                                            // 2. Draw PDF / Image background or miniature paper template
                                            let scale = thumb_w / p.width.max(1.0);
                                            let origin = rect.min;
                                            match p.effective_background() {
                                                crate::model::PageBackground::Image { asset_id }
                                                | crate::model::PageBackground::Pdf {
                                                    asset_id,
                                                    ..
                                                } => {
                                                    if let Some(tex) =
                                                        self.canvas_state.texture_cache.get(&asset_id)
                                                    {
                                                        crate::canvas::draw_rotated_background_image(
                                                            &painter,
                                                            tex.id(),
                                                            rect,
                                                            p.rotation,
                                                        );
                                                    }
                                                }
                                                crate::model::PageBackground::Template(tmpl) => {
                                                    let spacing = (30.0 * scale).max(4.0);
                                                    let line_col = Color32::from_rgb(220, 226, 231);
                                                    match tmpl {
                                                        PageTemplate::Blank => {}
                                                        PageTemplate::Ruled | PageTemplate::Cornell => {
                                                            let mut y = rect.top() + spacing;
                                                            while y < rect.bottom() {
                                                                painter.line_segment(
                                                                    [
                                                                        Pos2::new(rect.left(), y),
                                                                        Pos2::new(rect.right(), y),
                                                                    ],
                                                                    Stroke::new(0.5_f32, line_col),
                                                                );
                                                                y += spacing;
                                                            }
                                                        }
                                                        PageTemplate::Grid => {
                                                            let mut y = rect.top() + spacing;
                                                            while y < rect.bottom() {
                                                                painter.line_segment(
                                                                    [
                                                                        Pos2::new(rect.left(), y),
                                                                        Pos2::new(rect.right(), y),
                                                                    ],
                                                                    Stroke::new(0.5_f32, line_col),
                                                                );
                                                                y += spacing;
                                                            }
                                                            let mut x = rect.left() + spacing;
                                                            while x < rect.right() {
                                                                painter.line_segment(
                                                                    [
                                                                        Pos2::new(x, rect.top()),
                                                                        Pos2::new(x, rect.bottom()),
                                                                    ],
                                                                    Stroke::new(0.5_f32, line_col),
                                                                );
                                                                x += spacing;
                                                            }
                                                        }
                                                        PageTemplate::Dotted => {
                                                            let mut y = rect.top() + spacing;
                                                            while y < rect.bottom() {
                                                                let mut x = rect.left() + spacing;
                                                                while x < rect.right() {
                                                                    painter.circle_filled(
                                                                        Pos2::new(x, y),
                                                                        0.6,
                                                                        line_col,
                                                                    );
                                                                    x += spacing;
                                                                }
                                                                y += spacing;
                                                            }
                                                        }
                                                    }
                                                }
                                            }

                                            // 3. Miniature objects preview (strokes, shapes, images, sticky notes, text)
                                            for obj in &p.objects {
                                                match obj {
                                                    crate::model::DrawObject::Stroke {
                                                        points,
                                                        color,
                                                        width,
                                                        is_highlighter,
                                                        ..
                                                    } => {
                                                        if points.len() > 1 {
                                                            let c = if *is_highlighter {
                                                                Color32::from_rgba_unmultiplied(
                                                                    color.r, color.g, color.b, 95,
                                                                )
                                                            } else {
                                                                color.to_color32()
                                                            };
                                                            let stroke = Stroke::new(
                                                                (width * scale).max(0.8),
                                                                c,
                                                            );
                                                            for w in points.windows(2) {
                                                                let p1 = origin
                                                                    + Vec2::new(
                                                                        w[0].x * scale,
                                                                        w[0].y * scale,
                                                                    );
                                                                let p2 = origin
                                                                    + Vec2::new(
                                                                        w[1].x * scale,
                                                                        w[1].y * scale,
                                                                    );
                                                                painter.line_segment([p1, p2], stroke);
                                                            }
                                                        }
                                                    }
                                                    crate::model::DrawObject::Shape {
                                                        shape_type,
                                                        x,
                                                        y,
                                                        width,
                                                        height,
                                                        color,
                                                        stroke_width,
                                                        ..
                                                    } => {
                                                        let stroke = Stroke::new(
                                                            (stroke_width * scale).max(0.9),
                                                            color.to_color32(),
                                                        );
                                                        match shape_type {
                                                            ShapeKind::Ellipse => {
                                                                let p1 = origin
                                                                    + Vec2::new(x * scale, y * scale);
                                                                let p2 = origin
                                                                    + Vec2::new(
                                                                        (x + width) * scale,
                                                                        (y + height) * scale,
                                                                    );
                                                                let r = Rect::from_two_pos(p1, p2);
                                                                let rad = (r.width().min(r.height())
                                                                    * 0.5)
                                                                    .max(1.0);
                                                                painter.circle_stroke(
                                                                    r.center(),
                                                                    rad,
                                                                    stroke,
                                                                );
                                                            }
                                                            ShapeKind::Line => {
                                                                let p1 = origin
                                                                    + Vec2::new(x * scale, y * scale);
                                                                let p2 = origin
                                                                    + Vec2::new(
                                                                        (x + width) * scale,
                                                                        (y + height) * scale,
                                                                    );
                                                                painter.line_segment([p1, p2], stroke);
                                                            }
                                                            _ => {
                                                                let verts =
                                                                    crate::geometry::shape_vertices(
                                                                        *shape_type,
                                                                        *x,
                                                                        *y,
                                                                        *width,
                                                                        *height,
                                                                    );
                                                                for i in 0..verts.len() {
                                                                    let a = origin
                                                                        + Vec2::new(
                                                                            verts[i].x * scale,
                                                                            verts[i].y * scale,
                                                                        );
                                                                    let b = origin
                                                                        + Vec2::new(
                                                                            verts[(i + 1) % verts.len()]
                                                                                .x
                                                                                * scale,
                                                                            verts[(i + 1) % verts.len()]
                                                                                .y
                                                                                * scale,
                                                                        );
                                                                    painter.line_segment([a, b], stroke);
                                                                }
                                                            }
                                                        }
                                                    }
                                                    crate::model::DrawObject::Image {
                                                        asset_id,
                                                        x,
                                                        y,
                                                        width,
                                                        height,
                                                        rotation,
                                                        ..
                                                    } => {
                                                        let center = origin + Vec2::new((*x + *width / 2.0) * scale, (*y + *height / 2.0) * scale);
                                                        let half_w = (*width * scale) / 2.0;
                                                        let half_h = (*height * scale) / 2.0;
                                                        let r = Rect::from_center_size(center, Vec2::new(half_w * 2.0, half_h * 2.0));
                                                        if let Some(tex) =
                                                            self.canvas_state.texture_cache.get(asset_id)
                                                        {
                                                            let rot_deg = (rotation.to_degrees().round() as i32).rem_euclid(360);
                                                            crate::canvas::draw_rotated_background_image(
                                                                &painter,
                                                                tex.id(),
                                                                r,
                                                                rot_deg,
                                                            );
                                                        }
                                                    }
                                                    crate::model::DrawObject::StickyNote {
                                                        x,
                                                        y,
                                                        width,
                                                        height,
                                                        color,
                                                        ..
                                                    } => {
                                                        let p1 =
                                                            origin + Vec2::new(x * scale, y * scale);
                                                        let p2 = origin
                                                            + Vec2::new(
                                                                (x + width) * scale,
                                                                (y + height) * scale,
                                                            );
                                                        let r = Rect::from_two_pos(p1, p2);
                                                        painter.rect_filled(r, 1.5, color.to_color32());
                                                    }
                                                    crate::model::DrawObject::Text {
                                                        x,
                                                        y,
                                                        text,
                                                        color,
                                                        rotation,
                                                        ..
                                                    } => {
                                                        let pos =
                                                            origin + Vec2::new(x * scale, y * scale);
                                                        let preview_text = if text.chars().count() > 8 {
                                                            format!(
                                                                "{}…",
                                                                text.chars().take(7).collect::<String>()
                                                            )
                                                        } else {
                                                            text.clone()
                                                        };
                                                        let galley = painter.layout_no_wrap(
                                                            preview_text,
                                                            FontId::proportional(8.0),
                                                            color.to_color32(),
                                                        );
                                                        let mut text_shape = egui::epaint::TextShape::new(
                                                            pos,
                                                            galley,
                                                            color.to_color32(),
                                                        );
                                                        text_shape.angle = *rotation;
                                                        painter.add(text_shape);
                                                    }
                                                }
                                            }

                                            // Bookmark badge if page is bookmarked
                                            if p.bookmarked {
                                                let bm_pos = rect.right_top() + Vec2::new(-14.0, 12.0);
                                                painter.text(
                                                    bm_pos,
                                                    egui::Align2::CENTER_CENTER,
                                                    "🔖",
                                                    FontId::proportional(11.0),
                                                    Color32::from_rgb(255, 204, 0),
                                                );
                                            }

                                            // Checkbox badge jika dalam mode multi-select
                                            if self.thumb_select_mode {
                                                let is_selected = self.thumb_selected_pages.contains(&p_i);
                                                let badge_rect = Rect::from_min_size(
                                                    rect.left_top() + Vec2::new(4.0, 4.0),
                                                    Vec2::new(16.0, 16.0),
                                                );
                                                let badge_bg = if is_selected {
                                                    Color32::from_rgb(0, 114, 206)
                                                } else {
                                                    Color32::from_rgba_unmultiplied(30, 30, 30, 190)
                                                };
                                                painter.rect_filled(badge_rect, 3.0, badge_bg);
                                                painter.rect_stroke(
                                                    badge_rect,
                                                    3.0,
                                                    Stroke::new(1.0_f32, Color32::WHITE),
                                                    StrokeKind::Outside,
                                                );
                                                if is_selected {
                                                    painter.text(
                                                        badge_rect.center(),
                                                        egui::Align2::CENTER_CENTER,
                                                        "✓",
                                                        FontId::proportional(11.0),
                                                        Color32::WHITE,
                                                    );
                                                }
                                            }

                                            // Border highlight for active page or selected pages
                                            let is_thumb_selected = self.thumb_selected_pages.contains(&p_i);
                                            let border_color = if is_thumb_selected {
                                                Color32::from_rgb(0, 180, 255)
                                            } else if is_active {
                                                Color32::from_rgb(0, 114, 206)
                                            } else if resp.hovered() {
                                                Color32::from_rgb(90, 90, 90)
                                            } else {
                                                Color32::from_rgb(50, 50, 50)
                                            };
                                            let border_w = if is_active || is_thumb_selected { 2.5_f32 } else { 1.0_f32 };
                                            painter.rect_stroke(
                                                rect,
                                                3.0,
                                                Stroke::new(border_w, border_color),
                                                StrokeKind::Outside,
                                            );

                                            if resp.clicked() {
                                                if self.thumb_select_mode {
                                                    if self.thumb_selected_pages.contains(&p_i) {
                                                        self.thumb_selected_pages.remove(&p_i);
                                                    } else {
                                                        self.thumb_selected_pages.insert(p_i);
                                                    }
                                                } else if ctx.input(|i| {
                                                    i.modifiers.command || i.modifiers.ctrl || i.modifiers.shift
                                                }) {
                                                    self.thumb_select_mode = true;
                                                    self.thumb_selected_pages.clear();
                                                    self.thumb_selected_pages.insert(self.current_page_index);
                                                    self.thumb_selected_pages.insert(p_i);
                                                } else {
                                                    action_select = Some(p_i);
                                                }
                                            }

                                            resp.context_menu(|ui| {
                                                if ui.button("📄 Duplicate Page").clicked() {
                                                    action_duplicate = Some(p_i);
                                                    ui.close_menu();
                                                }
                                                if page_count > 1 && ui.button("🗑 Delete Page").clicked()
                                                {
                                                    action_delete = Some(p_i);
                                                    ui.close_menu();
                                                }
                                                ui.separator();
                                                if ui.button("↻ Rotate 90° CW").clicked() {
                                                    action_rotate_cw = Some(p_i);
                                                    ui.close_menu();
                                                }
                                                if ui.button("↺ Rotate 90° CCW").clicked() {
                                                    action_rotate_ccw = Some(p_i);
                                                    ui.close_menu();
                                                }
                                                if ui.button("⟲ Rotate 180°").clicked() {
                                                    action_rotate_180 = Some(p_i);
                                                    ui.close_menu();
                                                }
                                                ui.separator();
                                                if p_i > 0 && ui.button("⬆ Move Up").clicked() {
                                                    action_move_up = Some(p_i);
                                                    ui.close_menu();
                                                }
                                                if p_i + 1 < page_count
                                                    && ui.button("⬇ Move Down").clicked()
                                                {
                                                    action_move_down = Some(p_i);
                                                    ui.close_menu();
                                                }
                                                ui.separator();
                                                if ui.button("➕ Insert Page Before").clicked() {
                                                    action_insert_before = Some(p_i);
                                                    ui.close_menu();
                                                }
                                                if ui.button("➕ Insert Page After").clicked() {
                                                    action_insert_after = Some(p_i);
                                                    ui.close_menu();
                                                }
                                                ui.separator();
                                                if ui.button("📐 Change Size...").clicked() {
                                                    action_change_size = Some(p_i);
                                                    ui.close_menu();
                                                }
                                            });

                                            ui.label(
                                                RichText::new(format!("{}", p_i + 1)).size(11.0).color(
                                                    if is_active {
                                                        Color32::from_rgb(76, 147, 236)
                                                    } else {
                                                        Color32::GRAY
                                                    },
                                                ),
                                            );
                                            ui.add_space(8.0);
                                        });
                                    }

                                    // Execute deferred context menu actions
                                    if let Some((from_i, to_i)) = action_move_drag
                                        && from_i != to_i
                                    {
                                        self.push_doc_undo(nb_idx);
                                        self.library.notebooks[nb_idx].move_page(from_i, to_i);
                                        self.navigate_to_page(nb_idx, to_i);
                                        self.mark_dirty(&notebook_id);
                                        self.toast_message = Some(format!(
                                            "📑 Halaman dipindahkan ke posisi {}.",
                                            to_i + 1
                                        ));
                                    }
                                    if pointer_released {
                                        self.dragged_thumb_page = None;
                                    }
                                    if let Some(idx) = action_rotate_cw {
                                        self.push_doc_undo(nb_idx);
                                        self.library.notebooks[nb_idx].rotate_page(idx, 90);
                                        self.canvas_state.mesh_cache.clear();
                                        if let Some(p) = self.library.notebooks[nb_idx].pages.get(self.current_page_index) {
                                            self.canvas_state.spatial_grid.rebuild(&p.objects);
                                        }
                                        self.mark_dirty(&notebook_id);
                                        self.toast_message =
                                            Some(format!("↻ Halaman {} diputar 90° CW.", idx + 1));
                                    }
                                    if let Some(idx) = action_rotate_ccw {
                                        self.push_doc_undo(nb_idx);
                                        self.library.notebooks[nb_idx].rotate_page(idx, -90);
                                        self.canvas_state.mesh_cache.clear();
                                        if let Some(p) = self.library.notebooks[nb_idx].pages.get(self.current_page_index) {
                                            self.canvas_state.spatial_grid.rebuild(&p.objects);
                                        }
                                        self.mark_dirty(&notebook_id);
                                        self.toast_message =
                                            Some(format!("↺ Halaman {} diputar 90° CCW.", idx + 1));
                                    }
                                    if let Some(idx) = action_rotate_180 {
                                        self.push_doc_undo(nb_idx);
                                        self.library.notebooks[nb_idx].rotate_page(idx, 180);
                                        self.canvas_state.mesh_cache.clear();
                                        if let Some(p) = self.library.notebooks[nb_idx].pages.get(self.current_page_index) {
                                            self.canvas_state.spatial_grid.rebuild(&p.objects);
                                        }
                                        self.mark_dirty(&notebook_id);
                                        self.toast_message =
                                            Some(format!("⟲ Halaman {} diputar 180°.", idx + 1));
                                    }
                                    if let Some(idx) = action_select {
                                        self.navigate_to_page(nb_idx, idx);
                                    }
                                    if let Some(idx) = action_duplicate {
                                        self.push_doc_undo(nb_idx);
                                        if let Some(new_idx) =
                                            self.library.notebooks[nb_idx].duplicate_page(idx)
                                        {
                                            self.navigate_to_page(nb_idx, new_idx);
                                            self.mark_dirty(&notebook_id);
                                        }
                                    }
                                    if let Some(idx) = action_delete {
                                        self.push_doc_undo(nb_idx);
                                        if let Ok(new_idx) = self.library.notebooks[nb_idx].delete_page(idx)
                                        {
                                            self.navigate_to_page(nb_idx, new_idx);
                                            self.mark_dirty(&notebook_id);
                                        }
                                    }
                                    if let Some(idx) = action_move_up {
                                        self.push_doc_undo(nb_idx);
                                        if self.library.notebooks[nb_idx].move_page(idx, idx - 1) {
                                            self.navigate_to_page(nb_idx, idx - 1);
                                            self.mark_dirty(&notebook_id);
                                        }
                                    }
                                    if let Some(idx) = action_move_down {
                                        self.push_doc_undo(nb_idx);
                                        if self.library.notebooks[nb_idx].move_page(idx, idx + 1) {
                                            self.navigate_to_page(nb_idx, idx + 1);
                                            self.mark_dirty(&notebook_id);
                                        }
                                    }
                                    if let Some(idx) = action_insert_before {
                                        let (w, h, tmpl) = {
                                            let p = &self.library.notebooks[nb_idx].pages[idx];
                                            (p.width, p.height, p.template)
                                        };
                                        self.push_doc_undo(nb_idx);
                                        self.library.notebooks[nb_idx].insert_page(idx, tmpl, w, h);
                                        self.navigate_to_page(nb_idx, idx);
                                        self.mark_dirty(&notebook_id);
                                    }
                                    if let Some(idx) = action_insert_after {
                                        let (w, h, tmpl) = {
                                            let p = &self.library.notebooks[nb_idx].pages[idx];
                                            (p.width, p.height, p.template)
                                        };
                                        self.push_doc_undo(nb_idx);
                                        self.library.notebooks[nb_idx].insert_page(idx + 1, tmpl, w, h);
                                        self.navigate_to_page(nb_idx, idx + 1);
                                        self.mark_dirty(&notebook_id);
                                    }
                                    if let Some(idx) = action_change_size {
                                        self.navigate_to_page(nb_idx, idx);
                                        self.open_page_resize_modal(nb_idx, idx);
                                    }
                                });
                        }
                        SidebarTab::Bookmarks => {
                            ui.label(RichText::new("Bookmarked Pages").strong().color(Color32::WHITE));
                            ui.separator();
                            ScrollArea::vertical()
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    let mut unbookmark_idx = None;
                                    let mut jump_idx = None;
                                    let pages = &self.library.notebooks[nb_idx].pages;
                                    let bookmarked_list: Vec<(usize, &Page)> = pages
                                        .iter()
                                        .enumerate()
                                        .filter(|(_, p)| p.bookmarked)
                                        .collect();

                                    if bookmarked_list.is_empty() {
                                        ui.add_space(20.0);
                                        ui.label(
                                            RichText::new("Tidak ada bookmark.\nKlik ikon 🔖 di bilah atas untuk menandai halaman.")
                                                .size(11.0)
                                                .color(Color32::GRAY),
                                        );
                                    } else {
                                        for (p_i, p) in bookmarked_list {
                                            let is_cur = p_i == self.current_page_index;
                                            ui.horizontal(|ui| {
                                                let btn_text = format!("🔖 Hal. {}", p_i + 1);
                                                let btn = Button::new(
                                                    RichText::new(btn_text)
                                                        .size(12.0)
                                                        .color(if is_cur {
                                                            Color32::from_rgb(76, 147, 236)
                                                        } else {
                                                            Color32::WHITE
                                                        }),
                                                )
                                                .fill(if is_cur {
                                                    Color32::from_rgb(39, 54, 91)
                                                } else {
                                                    Color32::TRANSPARENT
                                                });
                                                if ui.add(btn).clicked() {
                                                    jump_idx = Some(p_i);
                                                }
                                                if ui.small_button("✕").on_hover_text("Hapus bookmark").clicked() {
                                                    unbookmark_idx = Some(p_i);
                                                }
                                            });
                                            // Optional mini summary if page has text
                                            let first_text = p.objects.iter().find_map(|o| {
                                                if let crate::model::DrawObject::Text { text, .. } = o {
                                                    Some(text.as_str())
                                                } else if let crate::model::DrawObject::StickyNote { text, .. } = o {
                                                    Some(text.as_str())
                                                } else {
                                                    None
                                                }
                                            });
                                            if let Some(txt) = first_text {
                                                let preview: String = txt.chars().take(22).collect();
                                                ui.label(RichText::new(format!("  \"{preview}…\"")).size(10.0).color(Color32::DARK_GRAY));
                                            }
                                            ui.add_space(4.0);
                                        }
                                    }

                                    if let Some(j) = jump_idx {
                                        self.navigate_to_page(nb_idx, j);
                                    }
                                    if let Some(u) = unbookmark_idx {
                                        self.library.notebooks[nb_idx].pages[u].bookmarked = false;
                                        self.mark_dirty(&notebook_id);
                                    }
                                });
                        }
                        SidebarTab::Outline => {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("Daftar Isi").strong().color(Color32::WHITE));
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if ui.button("➕ Tambah").clicked() {
                                        self.new_outline_title = format!("Halaman {}", self.current_page_index + 1);
                                        self.show_add_outline_modal = true;
                                    }
                                });
                            });
                            ui.separator();
                            ScrollArea::vertical()
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    let mut delete_outline_idx = None;
                                    let mut jump_page = None;
                                    let outline = &self.library.notebooks[nb_idx].outline;

                                    if outline.is_empty() {
                                        ui.add_space(20.0);
                                        ui.label(
                                            RichText::new("Belum ada daftar isi.\nKlik '➕ Tambah' untuk menandai bab/topik di halaman ini.")
                                                .size(11.0)
                                                .color(Color32::GRAY),
                                        );
                                    } else {
                                        for (idx, item) in outline.iter().enumerate() {
                                            let is_cur = item.page_index == self.current_page_index;
                                            ui.horizontal(|ui| {
                                                let label_text = format!("{} (Hal. {})", item.title, item.page_index + 1);
                                                let btn = Button::new(
                                                    RichText::new(label_text)
                                                        .size(12.0)
                                                        .color(if is_cur {
                                                            Color32::from_rgb(76, 147, 236)
                                                        } else {
                                                            Color32::WHITE
                                                        }),
                                                )
                                                .fill(if is_cur {
                                                    Color32::from_rgb(39, 54, 91)
                                                } else {
                                                    Color32::TRANSPARENT
                                                });
                                                if ui.add(btn).clicked() {
                                                    jump_page = Some(item.page_index);
                                                }
                                                if ui.small_button("✕").on_hover_text("Hapus dari outline").clicked() {
                                                    delete_outline_idx = Some(idx);
                                                }
                                            });
                                            ui.add_space(4.0);
                                        }
                                    }

                                    if let Some(p) = jump_page
                                        && p < self.library.notebooks[nb_idx].pages.len()
                                    {
                                        self.navigate_to_page(nb_idx, p);
                                    }
                                    if let Some(d) = delete_outline_idx {
                                        self.library.notebooks[nb_idx].outline.remove(d);
                                        self.mark_dirty(&notebook_id);
                                    }
                                });
                        }
                    }
                });
        }
    }
}

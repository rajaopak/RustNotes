use crate::canvas::state::CanvasState;
use crate::geometry::{
    calculate_auto_fit_font_size, calculate_shape_text_bounds, change_shape_kind,
};
use crate::model::{ConnectorRoute, DrawObject, MarkerKind, Page, Point2D, ShapeKind};
use eframe::egui::{self, Color32, Key, Pos2, Rect, Stroke, Ui, Vec2};

/// Tema visual dinamis untuk bilah alat mengambang (Floating Toolbars: Connector, Shape).
/// Menyesuaikan warna latar belakang, teks, border, dan tombol dengan tema aktif aplikasi (Dark / Light).
#[derive(Clone, Copy, Debug)]
pub struct FloatingToolbarStyle {
    pub bg_color: Color32,
    pub border_color: Color32,
    pub border_stroke: Stroke,
    pub primary_text_color: Color32,
    pub secondary_text_color: Color32,
    pub inactive_btn_bg: Color32,
    pub hovered_btn_bg: Color32,
    pub active_btn_bg: Color32,
    pub divider_color: Color32,
    pub divider_stroke: Stroke,
    pub shadow: egui::epaint::Shadow,
    pub is_dark: bool,
}

impl FloatingToolbarStyle {
    pub fn from_ui(ui: &egui::Ui) -> Self {
        let is_dark = ui.visuals().dark_mode;
        if is_dark {
            let bg_color = Color32::from_rgba_unmultiplied(36, 34, 36, 250);
            let border_color = Color32::from_rgb(68, 65, 68);
            let primary_text_color = Color32::from_rgb(240, 240, 242);
            let secondary_text_color = Color32::from_rgb(168, 166, 172);
            let inactive_btn_bg = Color32::from_rgb(48, 45, 48);
            let hovered_btn_bg = Color32::from_rgb(64, 60, 64);
            let active_btn_bg = Color32::from_rgb(0, 114, 206);
            let divider_color = Color32::from_rgb(65, 62, 65);
            Self {
                bg_color,
                border_color,
                border_stroke: Stroke::new(1.0_f32, border_color),
                primary_text_color,
                secondary_text_color,
                inactive_btn_bg,
                hovered_btn_bg,
                active_btn_bg,
                divider_color,
                divider_stroke: Stroke::new(1.0_f32, divider_color),
                shadow: egui::epaint::Shadow {
                    offset: [0, 4],
                    blur: 16,
                    spread: 1,
                    color: Color32::from_rgba_unmultiplied(0, 0, 0, 110),
                },
                is_dark: true,
            }
        } else {
            let bg_color = Color32::from_rgba_unmultiplied(252, 251, 249, 250);
            let border_color = Color32::from_rgb(205, 218, 232);
            let primary_text_color = Color32::from_rgb(33, 37, 41);
            let secondary_text_color = Color32::from_rgb(108, 117, 125);
            let inactive_btn_bg = Color32::from_rgb(238, 236, 232);
            let hovered_btn_bg = Color32::from_rgb(222, 228, 238);
            let active_btn_bg = Color32::from_rgb(0, 114, 206);
            let divider_color = Color32::from_rgb(215, 212, 206);
            Self {
                bg_color,
                border_color,
                border_stroke: Stroke::new(1.0_f32, border_color),
                primary_text_color,
                secondary_text_color,
                inactive_btn_bg,
                hovered_btn_bg,
                active_btn_bg,
                divider_color,
                divider_stroke: Stroke::new(1.0_f32, divider_color),
                shadow: egui::epaint::Shadow {
                    offset: [0, 3],
                    blur: 12,
                    spread: 0,
                    color: Color32::from_rgba_unmultiplied(0, 0, 0, 40),
                },
                is_dark: false,
            }
        }
    }
}

/// Merender Floating Toolbar untuk Connector Terpilih (Task 7.6)
pub fn render_floating_connector_toolbar(
    ui: &mut Ui,
    page: &mut Page,
    state: &mut CanvasState,
    conn_id: &str,
    poly: &[Point2D],
    canvas_rect: Rect,
    page_to_screen: impl Fn(Pos2) -> Pos2,
) {
    let s_mid = page_to_screen(poly[poly.len() / 2].to_pos2());
    let panel_pos = Pos2::new(
        s_mid
            .x
            .clamp(canvas_rect.left() + 250.0, canvas_rect.right() - 250.0),
        (s_mid.y - 28.0).max(canvas_rect.top() + 8.0),
    );

    let tb_style = FloatingToolbarStyle::from_ui(ui);
    let area_id = egui::Id::new("floating_connector_toolbar");
    egui::Area::new(area_id)
        .fixed_pos(panel_pos)
        .pivot(egui::Align2::CENTER_BOTTOM)
        .show(ui.ctx(), |ui| {
            egui::Frame::popup(ui.style())
                .fill(tb_style.bg_color)
                .stroke(tb_style.border_stroke)
                .corner_radius(8.0)
                .shadow(tb_style.shadow)
                .inner_margin(egui::Margin::symmetric(8, 4))
                .show(ui, |ui| {
                    let visuals = ui.visuals_mut();
                    visuals.override_text_color = Some(tb_style.primary_text_color);
                    visuals.widgets.inactive.bg_fill = tb_style.inactive_btn_bg;
                    visuals.widgets.hovered.bg_fill = tb_style.hovered_btn_bg;
                    visuals.widgets.active.bg_fill = tb_style.active_btn_bg;
                    visuals.widgets.noninteractive.bg_stroke = tb_style.divider_stroke;

                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(6.0, 0.0);

                        // 1. Jenis Rute Jalur
                        ui.menu_button("🔀 Rute", |ui| {
                            if ui.button("📏 Garis Lurus").clicked() {
                                state.push_undo(&page.id, &page.objects);
                                for obj in &mut page.objects {
                                    if obj.id() == conn_id
                                        && let DrawObject::Connector {
                                            route,
                                            control_points,
                                            ..
                                        } = obj
                                    {
                                        *route = ConnectorRoute::Straight;
                                        control_points.clear();
                                    }
                                }
                                ui.close_menu();
                            }
                            if ui.button("🔀 Siku (Orthogonal)").clicked() {
                                state.push_undo(&page.id, &page.objects);
                                for obj in &mut page.objects {
                                    if obj.id() == conn_id
                                        && let DrawObject::Connector {
                                            route,
                                            control_points,
                                            ..
                                        } = obj
                                    {
                                        *route = ConnectorRoute::Orthogonal;
                                        control_points.clear();
                                    }
                                }
                                ui.close_menu();
                            }
                            if ui.button("〰 Lengkung (Curved)").clicked() {
                                state.push_undo(&page.id, &page.objects);
                                for obj in &mut page.objects {
                                    if obj.id() == conn_id
                                        && let DrawObject::Connector {
                                            route,
                                            control_points,
                                            ..
                                        } = obj
                                    {
                                        *route = ConnectorRoute::Curved;
                                        control_points.clear();
                                    }
                                }
                                ui.close_menu();
                            }
                        });

                        ui.separator();

                        // 2. Marker Ujung Panah
                        ui.menu_button("➔ Ujung", |ui| {
                            if ui.button("Tanpa Panah").clicked() {
                                state.push_undo(&page.id, &page.objects);
                                for obj in &mut page.objects {
                                    if obj.id() == conn_id
                                        && let DrawObject::Connector { style, .. } = obj
                                    {
                                        style.start_marker = MarkerKind::None;
                                        style.end_marker = MarkerKind::None;
                                    }
                                }
                                ui.close_menu();
                            }
                            if ui.button("➔ Panah di Akhir").clicked() {
                                state.push_undo(&page.id, &page.objects);
                                for obj in &mut page.objects {
                                    if obj.id() == conn_id
                                        && let DrawObject::Connector { style, .. } = obj
                                    {
                                        style.start_marker = MarkerKind::None;
                                        style.end_marker = MarkerKind::Arrow;
                                    }
                                }
                                ui.close_menu();
                            }
                            if ui.button("↔ Panah di Kedua Ujung").clicked() {
                                state.push_undo(&page.id, &page.objects);
                                for obj in &mut page.objects {
                                    if obj.id() == conn_id
                                        && let DrawObject::Connector { style, .. } = obj
                                    {
                                        style.start_marker = MarkerKind::Arrow;
                                        style.end_marker = MarkerKind::Arrow;
                                    }
                                }
                                ui.close_menu();
                            }
                        });

                        ui.separator();

                        // 3. Warna Garis
                        let cur_col: Option<Color32> = page.objects.iter().find_map(|o| {
                            if o.id() == conn_id
                                && let DrawObject::Connector { style, .. } = o
                            {
                                Some(style.stroke_color.to_color32())
                            } else {
                                None
                            }
                        });

                        for &col in &[
                            Color32::from_rgb(33, 37, 41),
                            Color32::from_rgb(0, 114, 206),
                            Color32::from_rgb(220, 53, 69),
                            Color32::from_rgb(40, 167, 69),
                            Color32::from_rgb(253, 126, 20),
                            Color32::from_rgb(111, 66, 193),
                        ] {
                            let is_selected = cur_col == Some(col);
                            let (c_rect, resp) =
                                ui.allocate_exact_size(Vec2::splat(18.0), egui::Sense::click());
                            ui.painter().circle_filled(c_rect.center(), 7.0, col);
                            if tb_style.is_dark && col == Color32::from_rgb(33, 37, 41) {
                                ui.painter().circle_stroke(
                                    c_rect.center(),
                                    7.0,
                                    Stroke::new(1.0_f32, Color32::from_rgb(90, 88, 92)),
                                );
                            }
                            if is_selected {
                                ui.painter().circle_stroke(
                                    c_rect.center(),
                                    9.0,
                                    Stroke::new(1.5_f32, tb_style.active_btn_bg),
                                );
                            } else if resp.hovered() {
                                ui.painter().circle_stroke(
                                    c_rect.center(),
                                    8.5,
                                    Stroke::new(1.2_f32, tb_style.secondary_text_color),
                                );
                            }
                            if resp.clicked() {
                                state.push_undo(&page.id, &page.objects);
                                for obj in &mut page.objects {
                                    if obj.id() == conn_id
                                        && let DrawObject::Connector { style, .. } = obj
                                    {
                                        style.stroke_color = col.into();
                                    }
                                }
                            }
                        }

                        ui.separator();

                        // 4. Ketebalan Garis
                        let cur_w = page
                            .objects
                            .iter()
                            .find_map(|o| {
                                if o.id() == conn_id
                                    && let DrawObject::Connector { style, .. } = o
                                {
                                    Some(style.stroke_width)
                                } else {
                                    None
                                }
                            })
                            .unwrap_or(2.5);

                        for &w in &[1.5_f32, 2.5_f32, 4.0_f32, 6.0_f32] {
                            let is_active = (cur_w - w).abs() < 0.2;
                            let label = format!("{:.0}px", w);
                            if ui.selectable_label(is_active, label).clicked() {
                                state.push_undo(&page.id, &page.objects);
                                for obj in &mut page.objects {
                                    if obj.id() == conn_id
                                        && let DrawObject::Connector { style, .. } = obj
                                    {
                                        style.stroke_width = w;
                                    }
                                }
                            }
                        }

                        ui.separator();

                        // 5. Kunci / Buka Kunci
                        let is_locked = page.objects.iter().any(|o| {
                            o.id() == conn_id
                                && matches!(o, DrawObject::Connector { locked: true, .. })
                        });
                        let lock_text = if is_locked {
                            "🔓 Buka"
                        } else {
                            "🔒 Kunci"
                        };
                        if ui
                            .button(lock_text)
                            .on_hover_text("Kunci atau Buka Kunci Connector")
                            .clicked()
                        {
                            state.push_undo(&page.id, &page.objects);
                            for obj in &mut page.objects {
                                if obj.id() == conn_id
                                    && let DrawObject::Connector { locked, .. } = obj
                                {
                                    *locked = !is_locked;
                                }
                            }
                        }

                        // Task 8.3 & 8.6: Reset Jalur Otomatis bila memiliki titik kontrol manual
                        let has_cps = page.objects.iter().any(|o| {
                            o.id() == conn_id
                                && matches!(o, DrawObject::Connector { control_points, .. } if !control_points.is_empty())
                        });
                        if has_cps {
                            ui.separator();
                            if ui
                                .button("↺ Jalur Otomatis")
                                .on_hover_text(
                                    "Hapus semua tikungan manual dan kembali ke rute otomatis",
                                )
                                .clicked()
                            {
                                state.push_undo(&page.id, &page.objects);
                                for obj in &mut page.objects {
                                    if obj.id() == conn_id
                                        && let DrawObject::Connector { control_points, .. } = obj
                                    {
                                        control_points.clear();
                                    }
                                }
                            }
                        }
                    });
                });
        });
}

/// Merender Floating Toolbar untuk Bentuk (Shapes) yang dipilih
#[allow(clippy::too_many_arguments)]
pub fn render_floating_shape_toolbar(
    ui: &mut Ui,
    page: &mut Page,
    state: &mut CanvasState,
    s_nw: Pos2,
    s_ne: Pos2,
    s_se: Pos2,
    s_rot: Pos2,
    canvas_rect: Rect,
    all_locked: bool,
) {
    let min_screen_y = s_nw.y.min(s_ne.y).min(s_rot.y);
    let center_screen_x = (s_nw.x + s_se.x) * 0.5;
    let panel_pos = Pos2::new(
        center_screen_x.clamp(canvas_rect.left() + 250.0, canvas_rect.right() - 250.0),
        (min_screen_y - 24.0).max(canvas_rect.top() + 8.0),
    );

    let tb_style = FloatingToolbarStyle::from_ui(ui);
    let area_id = egui::Id::new("floating_shape_toolbar");
    egui::Area::new(area_id)
        .fixed_pos(panel_pos)
        .pivot(egui::Align2::CENTER_BOTTOM)
        .show(ui.ctx(), |ui| {
            egui::Frame::popup(ui.style())
                .fill(tb_style.bg_color)
                .stroke(tb_style.border_stroke)
                .corner_radius(8.0)
                .shadow(tb_style.shadow)
                .inner_margin(egui::Margin::symmetric(8, 4))
                .show(ui, |ui| {
                    let visuals = ui.visuals_mut();
                    visuals.override_text_color = Some(tb_style.primary_text_color);
                    visuals.widgets.inactive.bg_fill = tb_style.inactive_btn_bg;
                    visuals.widgets.hovered.bg_fill = tb_style.hovered_btn_bg;
                    visuals.widgets.active.bg_fill = tb_style.active_btn_bg;
                    visuals.widgets.noninteractive.bg_stroke = tb_style.divider_stroke;

                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(6.0, 0.0);

                        // 1. Tombol Cepat "Rounded Rectangle" (Task 5.5) & Jenis Bentuk (5.3)
                        ui.menu_button("📐 Bentuk", |ui| {
                            if ui.button("▢ Rounded Rect").clicked() {
                                state.push_undo(&page.id, &page.objects);
                                for obj in &mut page.objects {
                                    if state.selected_ids.contains(&obj.id().to_string())
                                        && let DrawObject::Shape { width, height, .. } = obj
                                    {
                                        let cr = ((*width).min(*height) * 0.12).clamp(4.0, 14.0);
                                        change_shape_kind(obj, ShapeKind::Rectangle, cr);
                                    }
                                }
                                state.invalidate_mesh_for_selection();
                                state.spatial_grid.rebuild(&page.objects);
                                ui.close_menu();
                            }
                            if ui.button("▭ Persegi Panjang").clicked() {
                                state.push_undo(&page.id, &page.objects);
                                for obj in &mut page.objects {
                                    if state.selected_ids.contains(&obj.id().to_string()) {
                                        change_shape_kind(obj, ShapeKind::Rectangle, 0.0);
                                    }
                                }
                                state.invalidate_mesh_for_selection();
                                state.spatial_grid.rebuild(&page.objects);
                                ui.close_menu();
                            }
                            if ui.button("⬭ Elips").clicked() {
                                state.push_undo(&page.id, &page.objects);
                                for obj in &mut page.objects {
                                    if state.selected_ids.contains(&obj.id().to_string()) {
                                        change_shape_kind(obj, ShapeKind::Ellipse, 0.0);
                                    }
                                }
                                state.invalidate_mesh_for_selection();
                                state.spatial_grid.rebuild(&page.objects);
                                ui.close_menu();
                            }
                            if ui.button("◇ Belah Ketupat").clicked() {
                                state.push_undo(&page.id, &page.objects);
                                for obj in &mut page.objects {
                                    if state.selected_ids.contains(&obj.id().to_string()) {
                                        change_shape_kind(obj, ShapeKind::Diamond, 0.0);
                                    }
                                }
                                state.invalidate_mesh_for_selection();
                                state.spatial_grid.rebuild(&page.objects);
                                ui.close_menu();
                            }
                            if ui.button("△ Segitiga").clicked() {
                                state.push_undo(&page.id, &page.objects);
                                for obj in &mut page.objects {
                                    if state.selected_ids.contains(&obj.id().to_string()) {
                                        change_shape_kind(obj, ShapeKind::Triangle, 0.0);
                                    }
                                }
                                state.invalidate_mesh_for_selection();
                                state.spatial_grid.rebuild(&page.objects);
                                ui.close_menu();
                            }
                            if ui.button("▱ Jajar Genjang").clicked() {
                                state.push_undo(&page.id, &page.objects);
                                for obj in &mut page.objects {
                                    if state.selected_ids.contains(&obj.id().to_string()) {
                                        change_shape_kind(obj, ShapeKind::Parallelogram, 0.0);
                                    }
                                }
                                state.invalidate_mesh_for_selection();
                                state.spatial_grid.rebuild(&page.objects);
                                ui.close_menu();
                            }
                            if ui.button("╱ Garis").clicked() {
                                state.push_undo(&page.id, &page.objects);
                                for obj in &mut page.objects {
                                    if state.selected_ids.contains(&obj.id().to_string()) {
                                        change_shape_kind(obj, ShapeKind::Line, 0.0);
                                    }
                                }
                                state.invalidate_mesh_for_selection();
                                state.spatial_grid.rebuild(&page.objects);
                                ui.close_menu();
                            }
                        });

                        // Tombol Cepat "Rounded Rectangle" (Task 5.5)
                        if ui
                            .button("▢ Bulat")
                            .on_hover_text("Beralih ke Rounded Rectangle")
                            .clicked()
                        {
                            state.push_undo(&page.id, &page.objects);
                            for obj in &mut page.objects {
                                if state.selected_ids.contains(&obj.id().to_string())
                                    && let DrawObject::Shape { width, height, .. } = obj
                                {
                                    let cr = ((*width).min(*height) * 0.12).clamp(4.0, 14.0);
                                    change_shape_kind(obj, ShapeKind::Rectangle, cr);
                                }
                            }
                            state.invalidate_mesh_for_selection();
                            state.spatial_grid.rebuild(&page.objects);
                        }

                        ui.separator();

                        // 2. Warna Garis
                        for &col in &[
                            Color32::from_rgb(33, 37, 41),   // Hitam
                            Color32::from_rgb(0, 114, 206),  // Biru
                            Color32::from_rgb(220, 53, 69),  // Merah
                            Color32::from_rgb(40, 167, 69),  // Hijau
                            Color32::from_rgb(253, 126, 20), // Oranye
                            Color32::from_rgb(111, 66, 193), // Ungu
                        ] {
                            let (c_rect, resp) =
                                ui.allocate_exact_size(Vec2::splat(16.0), egui::Sense::click());
                            ui.painter().circle_filled(c_rect.center(), 7.0, col);
                            if resp.hovered() {
                                ui.painter().circle_stroke(
                                    c_rect.center(),
                                    8.0,
                                    Stroke::new(1.5_f32, Color32::from_rgb(0, 114, 206)),
                                );
                            }
                            if resp.clicked() {
                                state.push_undo(&page.id, &page.objects);
                                for obj in &mut page.objects {
                                    if state.selected_ids.contains(&obj.id().to_string())
                                        && let DrawObject::Shape { color, .. } = obj
                                    {
                                        *color = col.into();
                                    }
                                }
                                state.invalidate_mesh_for_selection();
                            }
                        }

                        ui.separator();

                        // 3. Ketebalan Garis
                        for &w in &[1.5_f32, 2.5_f32, 4.0_f32, 6.0_f32] {
                            let label = format!("{:.0}px", w);
                            if ui.selectable_label(false, label).clicked() {
                                state.push_undo(&page.id, &page.objects);
                                for obj in &mut page.objects {
                                    if state.selected_ids.contains(&obj.id().to_string())
                                        && let DrawObject::Shape { stroke_width, .. } = obj
                                    {
                                        *stroke_width = w;
                                    }
                                }
                                state.invalidate_mesh_for_selection();
                            }
                        }

                        ui.separator();

                        // 4. Warna Isi
                        if ui
                            .button("⊘ Isi")
                            .on_hover_text("Tanpa Isi (Transparan)")
                            .clicked()
                        {
                            state.push_undo(&page.id, &page.objects);
                            for obj in &mut page.objects {
                                if state.selected_ids.contains(&obj.id().to_string())
                                    && let DrawObject::Shape { fill, .. } = obj
                                {
                                    *fill = None;
                                }
                            }
                            state.invalidate_mesh_for_selection();
                        }

                        for &(f_col, tooltip) in &[
                            (
                                Color32::from_rgba_unmultiplied(0, 114, 206, 45),
                                "Biru Lembut",
                            ),
                            (
                                Color32::from_rgba_unmultiplied(255, 220, 50, 55),
                                "Kuning Lembut",
                            ),
                            (
                                Color32::from_rgba_unmultiplied(40, 167, 69, 45),
                                "Hijau Lembut",
                            ),
                            (
                                Color32::from_rgba_unmultiplied(220, 53, 69, 45),
                                "Merah Lembut",
                            ),
                            (
                                Color32::from_rgba_unmultiplied(240, 243, 246, 220),
                                "Putih Kartu",
                            ),
                        ] {
                            let (f_rect, resp) =
                                ui.allocate_exact_size(Vec2::splat(16.0), egui::Sense::click());
                            ui.painter().rect_filled(f_rect, 2.0, f_col);
                            ui.painter().rect_stroke(
                                f_rect,
                                2.0,
                                Stroke::new(1.0_f32, Color32::from_rgb(180, 195, 210)),
                                egui::StrokeKind::Outside,
                            );
                            let resp = resp.on_hover_text(tooltip);
                            if resp.hovered() {
                                ui.painter().rect_stroke(
                                    f_rect,
                                    2.0,
                                    Stroke::new(1.5_f32, Color32::from_rgb(0, 114, 206)),
                                    egui::StrokeKind::Outside,
                                );
                            }
                            if resp.clicked() {
                                state.push_undo(&page.id, &page.objects);
                                for obj in &mut page.objects {
                                    if state.selected_ids.contains(&obj.id().to_string())
                                        && let DrawObject::Shape { fill, .. } = obj
                                    {
                                        *fill = Some(f_col.into());
                                    }
                                }
                                state.invalidate_mesh_for_selection();
                            }
                        }

                        ui.separator();

                        // Tombol Tambah/Edit Teks di dalam Bentuk (Task 6.1)
                        if ui
                            .button("✍ Teks")
                            .on_hover_text("Tulis atau Sunting Teks di dalam Bentuk")
                            .clicked()
                            && let Some(id) = state.selected_ids.first().cloned()
                            && let Some(obj) = page.objects.iter_mut().find(|o| o.id() == id)
                            && !obj.is_locked()
                        {
                            if let DrawObject::Shape { label, color, .. } = obj
                                && label.is_none()
                            {
                                *label = Some(crate::model::ShapeLabel {
                                    text: String::new(),
                                    font_size: 16.0,
                                    color: color.clone(),
                                    alignment: crate::model::TextAlign::Center,
                                    vertical_alignment: crate::model::VerticalAlign::Middle,
                                });
                            }
                            state.push_undo(&page.id, &page.objects);
                            state.editing_text_id = Some(id);
                            state.focus_text_edit = true;
                        }

                        ui.separator();

                        // 5. Kunci / Buka Kunci (Lock / Unlock)
                        let lock_text = if all_locked {
                            "🔓 Buka"
                        } else {
                            "🔒 Kunci"
                        };
                        if ui
                            .button(lock_text)
                            .on_hover_text("Kunci atau Buka Kunci Bentuk")
                            .clicked()
                        {
                            state.push_undo(&page.id, &page.objects);
                            let new_state = !all_locked;
                            for obj in &mut page.objects {
                                if state.selected_ids.contains(&obj.id().to_string())
                                    && let DrawObject::Shape { locked, .. } = obj
                                {
                                    *locked = new_state;
                                }
                            }
                        }
                    });
                });
        });
}

#[allow(clippy::too_many_arguments)]
pub fn render_floating_text_toolbar(
    ui: &mut Ui,
    page: &mut Page,
    state: &mut CanvasState,
    target_id: &str,
    screen_top_center: Pos2,
    canvas_rect: Rect,
    all_locked: bool,
) {
    let panel_pos = Pos2::new(
        screen_top_center
            .x
            .clamp(canvas_rect.left() + 200.0, canvas_rect.right() - 200.0),
        (screen_top_center.y - 18.0).max(canvas_rect.top() + 8.0),
    );

    let tb_style = FloatingToolbarStyle::from_ui(ui);
    let area_id = egui::Id::new(("floating_text_toolbar", target_id));
    egui::Area::new(area_id)
        .fixed_pos(panel_pos)
        .pivot(egui::Align2::CENTER_BOTTOM)
        .show(ui.ctx(), |ui| {
            egui::Frame::popup(ui.style())
                .fill(tb_style.bg_color)
                .stroke(tb_style.border_stroke)
                .corner_radius(8.0)
                .shadow(tb_style.shadow)
                .inner_margin(egui::Margin::symmetric(8, 4))
                .show(ui, |ui| {
                    let visuals = ui.visuals_mut();
                    visuals.override_text_color = Some(tb_style.primary_text_color);
                    visuals.widgets.inactive.bg_fill = tb_style.inactive_btn_bg;
                    visuals.widgets.hovered.bg_fill = tb_style.hovered_btn_bg;
                    visuals.widgets.active.bg_fill = tb_style.active_btn_bg;
                    visuals.widgets.noninteractive.bg_stroke = tb_style.divider_stroke;

                    if all_locked {
                        ui.horizontal(|ui| {
                            ui.label("🔒 Terkunci");
                        });
                        return;
                    }

                    // Ambil properti objek saat ini
                    let (
                        is_sticky,
                        curr_font_size,
                        curr_family,
                        curr_bold,
                        curr_italic,
                        curr_align,
                        curr_text_color,
                        curr_sticky_color,
                    ) = if let Some(obj) = page.objects.iter().find(|o| o.id() == target_id) {
                        match obj {
                            DrawObject::Text {
                                font_size,
                                font_family,
                                bold,
                                italic,
                                alignment,
                                color,
                                ..
                            } => (
                                false,
                                *font_size,
                                *font_family,
                                *bold,
                                *italic,
                                *alignment,
                                color.to_color32(),
                                state.sticky_bg_color,
                            ),
                            DrawObject::StickyNote {
                                color,
                                text_color,
                                font_size,
                                font_family,
                                bold,
                                italic,
                                alignment,
                                ..
                            } => (
                                true,
                                *font_size,
                                *font_family,
                                *bold,
                                *italic,
                                *alignment,
                                text_color.to_color32(),
                                color.to_color32(),
                            ),
                            _ => return,
                        }
                    } else {
                        return;
                    };

                    let mut mutated = false;
                    let mut new_font_size = curr_font_size;
                    let mut new_family = curr_family;
                    let mut new_bold = curr_bold;
                    let mut new_italic = curr_italic;
                    let mut new_align = curr_align;
                    let mut new_text_color = curr_text_color;
                    let mut new_sticky_color = curr_sticky_color;

                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(6.0, 0.0);

                        // 1. Pilihan Font Family
                        ui.menu_button(format!("🔤 {}", curr_family.name()), |ui| {
                            for f in crate::model::FontFamilyPreset::ALL {
                                if ui.button(f.name()).clicked() {
                                    new_family = f;
                                    mutated = true;
                                    ui.close_menu();
                                }
                            }
                        });

                        ui.separator();

                        // 2. Ukuran Font Stepper & Presets
                        if ui.button("➖").clicked() && curr_font_size > 10.0 {
                            new_font_size = (curr_font_size - 2.0).max(8.0);
                            mutated = true;
                        }
                        ui.menu_button(format!("{curr_font_size:.0} pt"), |ui| {
                            for sz in [
                                12.0, 14.0, 16.0, 18.0, 20.0, 24.0, 28.0, 32.0, 40.0, 48.0, 64.0,
                            ] {
                                if ui.button(format!("{sz:.0} pt")).clicked() {
                                    new_font_size = sz;
                                    mutated = true;
                                    ui.close_menu();
                                }
                            }
                        });
                        if ui.button("➕").clicked() && curr_font_size < 96.0 {
                            new_font_size = (curr_font_size + 2.0).min(96.0);
                            mutated = true;
                        }

                        ui.separator();

                        // 3. Gaya Teks: Bold (B) & Italic (I)
                        let b_label = if curr_bold {
                            egui::RichText::new("B").strong().color(Color32::WHITE)
                        } else {
                            egui::RichText::new("B").strong()
                        };
                        let b_btn = if curr_bold {
                            ui.add(egui::Button::new(b_label).fill(tb_style.active_btn_bg))
                        } else {
                            ui.button(b_label)
                        };
                        if b_btn.clicked() {
                            new_bold = !curr_bold;
                            mutated = true;
                        }

                        let i_label = if curr_italic {
                            egui::RichText::new("I").italics().color(Color32::WHITE)
                        } else {
                            egui::RichText::new("I").italics()
                        };
                        let i_btn = if curr_italic {
                            ui.add(egui::Button::new(i_label).fill(tb_style.active_btn_bg))
                        } else {
                            ui.button(i_label)
                        };
                        if i_btn.clicked() {
                            new_italic = !curr_italic;
                            mutated = true;
                        }

                        ui.separator();

                        // 4. Alignment: Left, Center, Right
                        let aligns = [
                            (crate::model::TextAlign::Left, "⇤", "Rata Kiri"),
                            (crate::model::TextAlign::Center, "≡", "Rata Tengah"),
                            (crate::model::TextAlign::Right, "⇥", "Rata Kanan"),
                        ];
                        for (al, icon, tip) in aligns {
                            let is_curr = curr_align == al;
                            let btn = if is_curr {
                                ui.add(
                                    egui::Button::new(
                                        egui::RichText::new(icon).color(Color32::WHITE),
                                    )
                                    .fill(tb_style.active_btn_bg),
                                )
                            } else {
                                ui.button(icon)
                            };
                            if btn.on_hover_text(tip).clicked() {
                                new_align = al;
                                mutated = true;
                            }
                        }

                        ui.separator();

                        // 5. Palet Warna Teks
                        ui.menu_button("🎨 Teks", |ui| {
                            let text_colors = [
                                (Color32::from_rgb(25, 27, 32), "Hitam"),
                                (Color32::from_rgb(100, 105, 115), "Abu-abu"),
                                (Color32::from_rgb(15, 82, 186), "Biru Navy"),
                                (Color32::from_rgb(2, 132, 199), "Biru Muda"),
                                (Color32::from_rgb(22, 128, 61), "Hijau"),
                                (Color32::from_rgb(220, 38, 38), "Merah"),
                                (Color32::from_rgb(217, 119, 6), "Oranye"),
                                (Color32::from_rgb(126, 34, 206), "Ungu"),
                            ];
                            for (c, label) in text_colors {
                                ui.horizontal(|ui| {
                                    let (rect, resp) = ui.allocate_exact_size(
                                        Vec2::splat(16.0),
                                        egui::Sense::click(),
                                    );
                                    ui.painter().rect_filled(rect, 3.0, c);
                                    if resp.clicked() || ui.button(label).clicked() {
                                        new_text_color = c;
                                        mutated = true;
                                        ui.close_menu();
                                    }
                                });
                            }
                        });

                        // 6. Warna Kertas Tempel (Bila StickyNote)
                        if is_sticky {
                            ui.separator();
                            ui.menu_button("📝 Warna Tempel", |ui| {
                                let sticky_colors = [
                                    (Color32::from_rgb(254, 243, 189), "Kuning Klasik"),
                                    (Color32::from_rgb(252, 231, 243), "Pink Pastel"),
                                    (Color32::from_rgb(220, 252, 231), "Hijau Mint"),
                                    (Color32::from_rgb(224, 242, 254), "Biru Langit"),
                                    (Color32::from_rgb(243, 232, 255), "Ungu Lavender"),
                                    (Color32::from_rgb(254, 215, 170), "Oranye Peach"),
                                ];
                                for (c, label) in sticky_colors {
                                    ui.horizontal(|ui| {
                                        let (rect, resp) = ui.allocate_exact_size(
                                            Vec2::splat(16.0),
                                            egui::Sense::click(),
                                        );
                                        ui.painter().rect_filled(rect, 3.0, c);
                                        if resp.clicked() || ui.button(label).clicked() {
                                            new_sticky_color = c;
                                            mutated = true;
                                            ui.close_menu();
                                        }
                                    });
                                }
                            });
                        }

                        // 7. Tombol Sunting (Aktifkan inline editor bila belum aktif)
                        if state.editing_text_id.as_deref() != Some(target_id) {
                            ui.separator();
                            if ui
                                .button("✏ Sunting")
                                .on_hover_text("Buka penyunting teks langsung")
                                .clicked()
                            {
                                state.editing_text_id = Some(target_id.to_string());
                                state.focus_text_edit = true;
                            }
                        }
                    });

                    if mutated {
                        state.push_undo(&page.id, &page.objects);
                        state.text_font_size = new_font_size;
                        state.text_font_family = new_family;
                        state.text_bold = new_bold;
                        state.text_italic = new_italic;
                        state.text_alignment = new_align;
                        state.text_color = new_text_color;
                        if is_sticky {
                            state.sticky_bg_color = new_sticky_color;
                        }

                        for obj in &mut page.objects {
                            if obj.id() == target_id {
                                match obj {
                                    DrawObject::Text {
                                        font_size,
                                        font_family,
                                        bold,
                                        italic,
                                        alignment,
                                        color,
                                        ..
                                    } => {
                                        *font_size = new_font_size;
                                        *font_family = new_family;
                                        *bold = new_bold;
                                        *italic = new_italic;
                                        *alignment = new_align;
                                        *color = new_text_color.into();
                                    }
                                    DrawObject::StickyNote {
                                        color,
                                        text_color,
                                        font_size,
                                        font_family,
                                        bold,
                                        italic,
                                        alignment,
                                        ..
                                    } => {
                                        *font_size = new_font_size;
                                        *font_family = new_family;
                                        *bold = new_bold;
                                        *italic = new_italic;
                                        *alignment = new_align;
                                        *text_color = new_text_color.into();
                                        *color = new_sticky_color.into();
                                    }
                                    _ => {}
                                }
                            }
                        }
                        state.spatial_grid.rebuild(&page.objects);
                        ui.ctx().request_repaint();
                    }
                });
        });
}

/// Merender Inline Text Editor langsung di atas kanvas halaman (Text, StickyNote, Shape Label)
pub fn render_inline_text_editor(
    ui: &mut Ui,
    painter: &egui::Painter,
    page: &mut Page,
    state: &mut CanvasState,
    page_to_screen: impl Fn(Pos2) -> Pos2,
) {
    if let Some(edit_id) = state.editing_text_id.clone() {
        let mut close_editor = ui.input(|i| i.key_pressed(Key::Escape));
        if let Some(obj) = page.objects.iter_mut().find(|o| o.id() == edit_id) {
            match obj {
                DrawObject::Text {
                    x,
                    y,
                    text,
                    font_size,
                    color,
                    font_family,
                    ..
                } => {
                    let screen_tl = page_to_screen(Pos2::new(*x, *y));
                    let lines_cnt = text.lines().count().max(1) as f32;
                    let max_chars = text
                        .lines()
                        .map(|l| l.chars().count())
                        .max()
                        .unwrap_or(12)
                        .max(12) as f32;
                    let scaled_font = (*font_size * state.zoom).clamp(10.0, 96.0);
                    let egui_family = font_family.to_egui_font_family();
                    let box_w = (max_chars * scaled_font * 0.62 + 28.0).max(180.0);
                    let box_h = ((lines_cnt + 0.6) * scaled_font * 1.35 + 12.0).max(36.0);
                    let edit_rect = Rect::from_min_size(screen_tl, Vec2::new(box_w, box_h));

                    painter.rect_filled(
                        edit_rect.expand(4.0),
                        4.0,
                        Color32::from_rgba_unmultiplied(255, 255, 255, 215),
                    );
                    painter.rect_stroke(
                        edit_rect.expand(4.0),
                        4.0,
                        Stroke::new(1.8_f32, Color32::from_rgb(0, 114, 206)),
                        egui::StrokeKind::Outside,
                    );

                    let te_id = egui::Id::new(("canvas_inline_text", &edit_id));
                    let te = egui::TextEdit::multiline(text)
                        .id(te_id)
                        .font(egui::FontId::new(scaled_font, egui_family))
                        .text_color(color.to_color32())
                        .hint_text("Ketik teks di sini...")
                        .frame(false)
                        .desired_width(box_w);
                    let te_resp = ui.put(edit_rect, te);
                    if state.focus_text_edit {
                        te_resp.request_focus();
                        state.focus_text_edit = false;
                    } else if te_resp.lost_focus() {
                        close_editor = true;
                    }
                }
                DrawObject::StickyNote {
                    x,
                    y,
                    width,
                    height,
                    text,
                    text_color,
                    font_size,
                    font_family,
                    ..
                } => {
                    let p1 = page_to_screen(Pos2::new(*x, *y)) + Vec2::new(8.0, 8.0);
                    let p2 =
                        page_to_screen(Pos2::new(*x + *width, *y + *height)) - Vec2::new(8.0, 8.0);
                    let edit_rect = Rect::from_two_pos(p1, p2);
                    let scaled_font = (*font_size * state.zoom).clamp(10.0, 42.0);
                    let egui_family = font_family.to_egui_font_family();

                    painter.rect_stroke(
                        edit_rect.expand(6.0),
                        4.0,
                        Stroke::new(2.0_f32, Color32::from_rgb(0, 114, 206)),
                        egui::StrokeKind::Outside,
                    );

                    let te_id = egui::Id::new(("canvas_inline_sticky", &edit_id));
                    let te = egui::TextEdit::multiline(text)
                        .id(te_id)
                        .font(egui::FontId::new(scaled_font, egui_family))
                        .text_color(text_color.to_color32())
                        .hint_text("Tulis catatan tempel...")
                        .frame(false)
                        .desired_width(edit_rect.width());
                    let te_resp = ui.put(edit_rect, te);
                    if state.focus_text_edit {
                        te_resp.request_focus();
                        state.focus_text_edit = false;
                    } else if te_resp.lost_focus() {
                        close_editor = true;
                    }
                }
                DrawObject::Shape {
                    shape_type,
                    x,
                    y,
                    width,
                    height,
                    stroke_width,
                    corner_radius,
                    label,
                    locked,
                    ..
                } => {
                    if *locked {
                        close_editor = true;
                    } else {
                        if label.is_none() {
                            *label = Some(crate::model::ShapeLabel {
                                text: String::new(),
                                font_size: 16.0,
                                color: crate::model::ColorData {
                                    r: 35,
                                    g: 35,
                                    b: 35,
                                    a: 255,
                                },
                                alignment: crate::model::TextAlign::Center,
                                vertical_alignment: crate::model::VerticalAlign::Middle,
                            });
                        }
                        if let Some(lbl) = label.as_mut() {
                            let (safe_w, safe_h) = calculate_shape_text_bounds(
                                *shape_type,
                                *width,
                                *height,
                                *corner_radius,
                                *stroke_width,
                            );
                            let auto_font = calculate_auto_fit_font_size(
                                lbl.font_size,
                                &lbl.text,
                                safe_w,
                                safe_h,
                                8.0,
                            );
                            let scaled_font = (auto_font * state.zoom).clamp(10.0, 72.0);

                            let center_page = Pos2::new(*x + *width / 2.0, *y + *height / 2.0);
                            let center_screen = page_to_screen(center_page);
                            let screen_w = (safe_w * state.zoom).max(60.0);
                            let screen_h = (safe_h * state.zoom).max(28.0);
                            let edit_rect = Rect::from_center_size(
                                center_screen,
                                Vec2::new(screen_w, screen_h),
                            );

                            painter.rect_filled(
                                edit_rect.expand(4.0),
                                4.0,
                                Color32::from_rgba_unmultiplied(255, 255, 255, 215),
                            );
                            painter.rect_stroke(
                                edit_rect.expand(4.0),
                                4.0,
                                Stroke::new(1.8_f32, Color32::from_rgb(0, 114, 206)),
                                egui::StrokeKind::Outside,
                            );

                            let te_id = egui::Id::new(("canvas_inline_shape_text", &edit_id));
                            let te = egui::TextEdit::multiline(&mut lbl.text)
                                .id(te_id)
                                .font(egui::FontId::proportional(scaled_font))
                                .text_color(lbl.color.to_color32())
                                .hint_text("Teks label...")
                                .frame(false)
                                .desired_width(screen_w);
                            let te_resp = ui.put(edit_rect, te);
                            if state.focus_text_edit {
                                te_resp.request_focus();
                                state.focus_text_edit = false;
                            } else if te_resp.lost_focus() {
                                close_editor = true;
                            }
                        }
                    }
                }
                _ => {
                    close_editor = true;
                }
            }
        }
        if close_editor {
            state.editing_text_id = None;
            page.objects
                .retain(|o| !matches!(o, DrawObject::Text { text, .. } if text.trim().is_empty()));
            for obj in &mut page.objects {
                if let DrawObject::Shape { label, .. } = obj
                    && let Some(lbl) = label
                    && lbl.text.trim().is_empty()
                {
                    *label = None;
                }
            }
            state.clean_redundant_top_undo(&page.id, &page.objects);
            state.spatial_grid.rebuild(&page.objects);
        }
    }
}

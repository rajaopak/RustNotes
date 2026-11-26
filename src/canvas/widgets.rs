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

                    let lang = state.language;
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(6.0, 0.0);

                        // 1. Jenis Rute Jalur
                        ui.menu_button(lang.tr("🔀 Rute", "🔀 Route"), |ui| {
                            if ui
                                .button(lang.tr("📏 Garis Lurus", "📏 Straight"))
                                .clicked()
                            {
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
                            if ui
                                .button(lang.tr("🔀 Siku (Orthogonal)", "🔀 Orthogonal"))
                                .clicked()
                            {
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
                            if ui
                                .button(lang.tr("〰 Lengkung (Curved)", "〰 Curved"))
                                .clicked()
                            {
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
                        ui.menu_button(lang.tr("➔ Ujung", "➔ Ends"), |ui| {
                            if ui.button(lang.tr("Tanpa Panah", "No Arrows")).clicked() {
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
                            if ui
                                .button(lang.tr("➔ Panah di Akhir", "➔ End Arrow"))
                                .clicked()
                            {
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
                            if ui
                                .button(lang.tr("↔ Panah di Kedua Ujung", "↔ Both Ends Arrows"))
                                .clicked()
                            {
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
                            lang.tr("🔓 Buka", "🔓 Unlock")
                        } else {
                            lang.tr("🔒 Kunci", "🔒 Lock")
                        };
                        if ui
                            .button(lock_text)
                            .on_hover_text(lang.tr(
                                "Kunci atau Buka Kunci Connector",
                                "Lock or Unlock Connector",
                            ))
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
                                .button(lang.tr("↺ Jalur Otomatis", "↺ Auto Route"))
                                .on_hover_text(lang.tr(
                                    "Hapus semua tikungan manual dan kembali ke rute otomatis",
                                    "Clear manual control points and restore automatic routing",
                                ))
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

                    let lang = state.language;
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(6.0, 0.0);

                        // 1. Tombol Cepat "Rounded Rectangle" (Task 5.5) & Jenis Bentuk (5.3)
                        ui.menu_button(lang.tr("📐 Bentuk", "📐 Shape"), |ui| {
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
                            if ui
                                .button(lang.tr("▭ Persegi Panjang", "▭ Rectangle"))
                                .clicked()
                            {
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
                            if ui.button(lang.tr("⬭ Elips", "⬭ Ellipse")).clicked() {
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
                            if ui.button(lang.tr("◇ Belah Ketupat", "◇ Diamond")).clicked() {
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
                            if ui.button(lang.tr("△ Segitiga", "△ Triangle")).clicked() {
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
                            if ui
                                .button(lang.tr("▱ Jajar Genjang", "▱ Parallelogram"))
                                .clicked()
                            {
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
                            if ui.button(lang.tr("╱ Garis", "╱ Line")).clicked() {
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
                            .button(lang.tr("▢ Bulat", "▢ Rounded"))
                            .on_hover_text(lang.tr(
                                "Beralih ke Rounded Rectangle",
                                "Switch to Rounded Rectangle",
                            ))
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
                            .button(lang.tr("⊘ Isi", "⊘ Fill"))
                            .on_hover_text(
                                lang.tr("Tanpa Isi (Transparan)", "No Fill (Transparent)"),
                            )
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
                                lang.tr("Biru Lembut", "Soft Blue"),
                            ),
                            (
                                Color32::from_rgba_unmultiplied(255, 220, 50, 55),
                                lang.tr("Kuning Lembut", "Soft Yellow"),
                            ),
                            (
                                Color32::from_rgba_unmultiplied(40, 167, 69, 45),
                                lang.tr("Hijau Lembut", "Soft Green"),
                            ),
                            (
                                Color32::from_rgba_unmultiplied(220, 53, 69, 45),
                                lang.tr("Merah Lembut", "Soft Red"),
                            ),
                            (
                                Color32::from_rgba_unmultiplied(240, 243, 246, 220),
                                lang.tr("Putih Kartu", "Card White"),
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
                            .button(lang.tr("✍ Teks", "✍ Text"))
                            .on_hover_text(lang.tr(
                                "Tulis atau Sunting Teks di dalam Bentuk",
                                "Write or Edit Text inside Shape",
                            ))
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
                            lang.tr("🔓 Buka", "🔓 Unlock")
                        } else {
                            lang.tr("🔒 Kunci", "🔒 Lock")
                        };
                        if ui
                            .button(lock_text)
                            .on_hover_text(
                                lang.tr("Kunci atau Buka Kunci Bentuk", "Lock or Unlock Shape"),
                            )
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

                    let lang = state.language;
                    if all_locked {
                        ui.horizontal(|ui| {
                            ui.label(lang.tr("🔒 Terkunci", "🔒 Locked"));
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
                    let mut push_undo_on_mutate = true;
                    let mut new_font_size = curr_font_size;
                    let mut new_family = curr_family;
                    let mut new_bold = curr_bold;
                    let mut new_italic = curr_italic;
                    let mut new_align = curr_align;
                    let mut new_text_color = curr_text_color;
                    let mut new_sticky_color = curr_sticky_color;

                    let (min_fs, max_fs) = if is_sticky {
                        (
                            crate::geometry::MIN_STICKY_FONT_SIZE,
                            crate::geometry::MAX_STICKY_FONT_SIZE,
                        )
                    } else {
                        (
                            crate::geometry::MIN_TEXT_FONT_SIZE,
                            crate::geometry::MAX_TEXT_FONT_SIZE,
                        )
                    };

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

                        // 2. Ukuran Font Stepper, DragValue Smooth & Presets
                        if ui.button("➖").clicked() && curr_font_size > min_fs {
                            new_font_size = (curr_font_size - 2.0).max(min_fs);
                            mutated = true;
                        }
                        let mut drag_fs = curr_font_size;
                        let dv_resp = ui
                            .add(
                                egui::DragValue::new(&mut drag_fs)
                                    .range(min_fs..=max_fs)
                                    .speed(0.35)
                                    .fixed_decimals(0)
                                    .suffix(" pt"),
                            )
                            .on_hover_text(lang.tr(
                                "Seret kiri/kanan atau klik untuk ubah ukuran font",
                                "Drag left/right or click to adjust font size",
                            ));
                        if dv_resp.drag_started() {
                            state.push_undo(&page.id, &page.objects);
                        }
                        if dv_resp.changed() {
                            new_font_size = drag_fs.clamp(min_fs, max_fs);
                            mutated = true;
                            if dv_resp.dragged() {
                                push_undo_on_mutate = false;
                            }
                        }
                        ui.menu_button("▾", |ui| {
                            for sz in [
                                10.0, 12.0, 14.0, 16.0, 18.0, 20.0, 24.0, 28.0, 32.0, 40.0, 48.0,
                                64.0, 96.0, 128.0,
                            ]
                            .into_iter()
                            .filter(|&s| s >= min_fs && s <= max_fs)
                            {
                                if ui.button(format!("{sz:.0} pt")).clicked() {
                                    new_font_size = sz;
                                    mutated = true;
                                    ui.close_menu();
                                }
                            }
                        });
                        if ui.button("➕").clicked() && curr_font_size < max_fs {
                            new_font_size = (curr_font_size + 2.0).min(max_fs);
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
                            (
                                crate::model::TextAlign::Left,
                                "⇤",
                                lang.tr("Rata Kiri", "Align Left"),
                            ),
                            (
                                crate::model::TextAlign::Center,
                                "≡",
                                lang.tr("Rata Tengah", "Align Center"),
                            ),
                            (
                                crate::model::TextAlign::Right,
                                "⇥",
                                lang.tr("Rata Kanan", "Align Right"),
                            ),
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
                        ui.menu_button(lang.tr("🎨 Teks", "🎨 Text"), |ui| {
                            let text_colors = [
                                (Color32::from_rgb(25, 27, 32), lang.tr("Hitam", "Black")),
                                (Color32::from_rgb(100, 105, 115), lang.tr("Abu-abu", "Gray")),
                                (
                                    Color32::from_rgb(15, 82, 186),
                                    lang.tr("Biru Navy", "Navy Blue"),
                                ),
                                (
                                    Color32::from_rgb(2, 132, 199),
                                    lang.tr("Biru Muda", "Sky Blue"),
                                ),
                                (Color32::from_rgb(22, 128, 61), lang.tr("Hijau", "Green")),
                                (Color32::from_rgb(220, 38, 38), lang.tr("Merah", "Red")),
                                (Color32::from_rgb(217, 119, 6), lang.tr("Oranye", "Orange")),
                                (Color32::from_rgb(126, 34, 206), lang.tr("Ungu", "Purple")),
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
                            ui.menu_button(lang.tr("📝 Warna Tempel", "📝 Note Color"), |ui| {
                                let sticky_colors = [
                                    (
                                        Color32::from_rgb(254, 243, 189),
                                        lang.tr("Kuning Klasik", "Classic Yellow"),
                                    ),
                                    (
                                        Color32::from_rgb(252, 231, 243),
                                        lang.tr("Pink Pastel", "Pastel Pink"),
                                    ),
                                    (
                                        Color32::from_rgb(220, 252, 231),
                                        lang.tr("Hijau Mint", "Mint Green"),
                                    ),
                                    (
                                        Color32::from_rgb(224, 242, 254),
                                        lang.tr("Biru Langit", "Sky Blue"),
                                    ),
                                    (
                                        Color32::from_rgb(243, 232, 255),
                                        lang.tr("Ungu Lavender", "Lavender"),
                                    ),
                                    (
                                        Color32::from_rgb(254, 215, 170),
                                        lang.tr("Oranye Peach", "Peach Orange"),
                                    ),
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
                                .button(lang.tr("✏ Sunting", "✏ Edit"))
                                .on_hover_text(
                                    lang.tr(
                                        "Buka penyunting teks langsung",
                                        "Open inline text editor",
                                    ),
                                )
                                .clicked()
                            {
                                state.editing_text_id = Some(target_id.to_string());
                                state.focus_text_edit = true;
                            }
                        }
                    });

                    if mutated {
                        if push_undo_on_mutate {
                            state.push_undo(&page.id, &page.objects);
                        }
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
                        state.rebuild_spatial_grid(&page.id, &page.objects);
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
                    bold,
                    italic,
                    alignment,
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
                    let scaled_font = (*font_size * state.zoom).clamp(6.0, 200.0);
                    let (egui_family, _needs_faux_bold) = ui.fonts(|fonts| {
                        let registered = fonts.families();
                        font_family.resolve_egui_font_family(*bold, |fam| registered.contains(fam))
                    });
                    let (approx_w, _) = crate::geometry::text_approx_size(text, *font_size);
                    let box_w = if text.is_empty() {
                        (scaled_font * 8.0).max(160.0)
                    } else {
                        (approx_w * state.zoom + 16.0)
                            .max((max_chars * scaled_font * 0.62 + 24.0).max(120.0))
                    };
                    let box_h =
                        ((lines_cnt + 0.6) * scaled_font * 1.35 + 8.0).max(scaled_font * 1.6);
                    let edit_rect = Rect::from_min_size(screen_tl, Vec2::new(box_w, box_h));

                    let te_id = egui::Id::new(("canvas_inline_text", &edit_id));
                    let text_c = color.to_color32();
                    let halign = alignment.to_egui_align();
                    let is_italic = *italic;
                    let font_id = egui::FontId::new(scaled_font, egui_family);
                    let mut layouter = |ui: &egui::Ui, buf: &str, wrap_width: f32| {
                        let mut job = egui::text::LayoutJob {
                            halign,
                            ..Default::default()
                        };
                        job.wrap.max_width = wrap_width;
                        job.append(
                            buf,
                            0.0,
                            egui::TextFormat {
                                font_id: font_id.clone(),
                                color: text_c,
                                italics: is_italic,
                                ..Default::default()
                            },
                        );
                        ui.fonts(|f| f.layout_job(job))
                    };
                    let te = egui::TextEdit::multiline(text)
                        .id(te_id)
                        .horizontal_align(halign)
                        .layouter(&mut layouter)
                        .frame(false)
                        .desired_width(box_w);
                    let prev_bg = ui.visuals().extreme_bg_color;
                    ui.visuals_mut().extreme_bg_color = Color32::TRANSPARENT;
                    let te_resp = ui.put(edit_rect, te);
                    ui.visuals_mut().extreme_bg_color = prev_bg;
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
                    bold,
                    italic,
                    alignment,
                    ..
                } => {
                    let p1 = page_to_screen(Pos2::new(*x, *y)) + Vec2::new(8.0, 8.0);
                    let p2 =
                        page_to_screen(Pos2::new(*x + *width, *y + *height)) - Vec2::new(8.0, 8.0);
                    let edit_rect = Rect::from_two_pos(p1, p2);
                    let scaled_font = (*font_size * state.zoom).clamp(6.0, 100.0);
                    let (egui_family, _needs_faux_bold) = ui.fonts(|fonts| {
                        let registered = fonts.families();
                        font_family.resolve_egui_font_family(*bold, |fam| registered.contains(fam))
                    });

                    painter.rect_stroke(
                        edit_rect.expand(6.0),
                        4.0,
                        Stroke::new(2.0_f32, Color32::from_rgb(0, 114, 206)),
                        egui::StrokeKind::Outside,
                    );

                    let te_id = egui::Id::new(("canvas_inline_sticky", &edit_id));
                    let text_c = text_color.to_color32();
                    let halign = alignment.to_egui_align();
                    let is_italic = *italic;
                    let font_id = egui::FontId::new(scaled_font, egui_family);
                    let mut layouter = |ui: &egui::Ui, buf: &str, wrap_width: f32| {
                        let mut job = egui::text::LayoutJob {
                            halign,
                            ..Default::default()
                        };
                        job.wrap.max_width = wrap_width;
                        job.append(
                            buf,
                            0.0,
                            egui::TextFormat {
                                font_id: font_id.clone(),
                                color: text_c,
                                italics: is_italic,
                                ..Default::default()
                            },
                        );
                        ui.fonts(|f| f.layout_job(job))
                    };
                    let te = egui::TextEdit::multiline(text)
                        .id(te_id)
                        .horizontal_align(halign)
                        .layouter(&mut layouter)
                        .hint_text(
                            state
                                .language
                                .tr("Tulis catatan tempel...", "Write sticky note..."),
                        )
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

                            painter.rect_stroke(
                                edit_rect.expand(3.0),
                                4.0,
                                Stroke::new(
                                    1.2_f32,
                                    Color32::from_rgba_unmultiplied(0, 114, 206, 150),
                                ),
                                egui::StrokeKind::Outside,
                            );

                            let te_id = egui::Id::new(("canvas_inline_shape_text", &edit_id));
                            let te = egui::TextEdit::multiline(&mut lbl.text)
                                .id(te_id)
                                .font(egui::FontId::proportional(scaled_font))
                                .text_color(lbl.color.to_color32())
                                .hint_text(state.language.tr("Teks label...", "Label text..."))
                                .frame(false)
                                .desired_width(screen_w);
                            let prev_bg = ui.visuals().extreme_bg_color;
                            ui.visuals_mut().extreme_bg_color = Color32::TRANSPARENT;
                            let te_resp = ui.put(edit_rect, te);
                            ui.visuals_mut().extreme_bg_color = prev_bg;
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
            state.rebuild_spatial_grid(&page.id, &page.objects);
        }
    }
}

/// Dialog modal untuk membuat dan mengedit rumus matematika LaTeX (Task 10)
pub fn render_equation_modal_dialog(ctx: &egui::Context, state: &mut CanvasState, page: &mut Page) {
    if !state.show_equation_dialog {
        return;
    }

    let lang = state.language;
    let mut open = true;
    let title = if state.editing_equation_id.is_some() {
        lang.tr(
            "📐 Edit LaTeX (Rumus, Tabel, Grafik, Geometri)",
            "📐 Edit LaTeX (Formula, Table, Graph, Geometry)",
        )
    } else {
        lang.tr(
            "📐 Sisipkan LaTeX (Rumus, Tabel, Grafik, Geometri)",
            "📐 Insert LaTeX (Formula, Table, Graph, Geometry)",
        )
    };

    let mut insert_action = false;
    let mut close_action = false;

    let clean_buffer = state.equation_latex_buffer.trim();
    let is_empty = clean_buffer.is_empty();
    let parse_result = if is_empty {
        Ok(Vec::new())
    } else {
        crate::geometry::math::parse_latex_math(clean_buffer)
    };

    let is_valid = !is_empty && parse_result.is_ok();

    let screen_rect = ctx.screen_rect();
    let max_w = (screen_rect.width() * 0.95).max(460.0);
    let max_h = (screen_rect.height() * 0.92).max(400.0);

    egui::Window::new(title)
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .pivot(egui::Align2::CENTER_CENTER)
        .default_pos(screen_rect.center())
        .default_size(Vec2::new(660.0, 620.0))
        .min_width(440.0)
        .min_height(380.0)
        .max_width(max_w)
        .max_height(max_h)
        .constrain_to(screen_rect)
        .show(ctx, |ui| {
            // Gunakan layout bottom_up agar bilah tombol tindakan ("Sisipkan" / "Batal")
            // selalu tertambat di bagian paling bawah (pinned footer) dan tidak pernah
            // terpotong atau terdorong ke luar layar ketika rumus sangat tinggi/panjang.
            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.add_enabled_ui(is_valid, |ui| {
                        let btn_text = if state.editing_equation_id.is_some() {
                            lang.tr("Perbarui Persamaan", "Update Equation")
                        } else {
                            lang.tr("Sisipkan ke Halaman", "Insert onto Page")
                        };
                        if ui.button(btn_text).clicked() {
                            insert_action = true;
                        }
                    });
                    if ui.button(lang.tr("Batal", "Cancel")).clicked() {
                        close_action = true;
                    }
                });
                ui.add_space(4.0);
                ui.separator();

                // Area konten di atas footer dengan ScrollArea vertikal dinamis
                ui.with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing = Vec2::new(8.0, 8.0);

                            ui.horizontal(|ui| {
                                ui.label(lang.tr("Ukuran Font:", "Font Size:"));
                                ui.add(
                                    egui::Slider::new(&mut state.equation_font_size, 12.0..=64.0)
                                        .suffix(" pt"),
                                );
                            });

                            // Quick LaTeX Snippet Buttons
                            ui.label(
                                egui::RichText::new(lang.tr(
                                    "Simbol & Rumus Cepat:",
                                    "Quick Symbols & Formulas:",
                                ))
                                .size(12.0)
                                .strong(),
                            );
                            ui.horizontal_wrapped(|ui| {
                                let snippets = [
                                    (r"\frac{a}{b}", r"\frac{a}{b}"),
                                    (r"\sqrt{x}", r"\sqrt{x}"),
                                    (r"x^{2}", r"^{2}"),
                                    (r"x_{i}", r"_{i}"),
                                    (r"\sum", r"\sum_{i=1}^{n}"),
                                    (r"\int", r"\int_{a}^{b}"),
                                    (r"\alpha", r"\alpha"),
                                    (r"\beta", r"\beta"),
                                    (r"\pi", r"\pi"),
                                    (r"\theta", r"\theta"),
                                    (r"\pm", r"\pm"),
                                    (r"\times", r"\times"),
                                    (r"\leq", r"\leq"),
                                    (r"\geq", r"\geq"),
                                    (r"\neq", r"\neq"),
                                    (r"\approx", r"\approx"),
                                    (r"\infty", r"\infty"),
                                    (r"\to", r"\to"),
                                ];
                                for (lbl, snip) in snippets {
                                    if ui.button(lbl).clicked() {
                                        state.equation_latex_buffer.push_str(snip);
                                    }
                                }
                            });

                            ui.label(
                                egui::RichText::new(lang.tr(
                                    "Template Tabel, Grafik & Geometri:",
                                    "Table, Graph & Geometry Templates:",
                                ))
                                .size(12.0)
                                .strong(),
                            );
                            ui.horizontal_wrapped(|ui| {
                                let templates: [(&str, &str); 8] = [
                                    (
                                        lang.tr("📊 Grafik Fungsi", "📊 Function Plot"),
                                        "\\begin{axis}[title={$f(x) = x^2$}, xlabel={$x$}, ylabel={$y$}, xmin=-3, xmax=3, grid=major, axis lines=middle]\n\\addplot[blue, domain=-3:3]{x^2};\n\\addplot[red, dashed, domain=-3:3]{2*x + 1};\n\\legend{$x^2$, $2x+1$}\n\\end{axis}",
                                    ),
                                    (
                                        lang.tr("📈 Grafik Data", "📈 Data Chart"),
                                        "\\begin{axis}[title={Data Pengamatan}, xlabel={Waktu}, ylabel={Nilai}, grid=major, ybar]\n\\addplot[teal] coordinates {(1, 3) (2, 7) (3, 5) (4, 9)};\n\\end{axis}",
                                    ),
                                    (
                                        lang.tr("🧊 Grafik 3D", "🧊 3D Surface"),
                                        "\\begin{tikzpicture}\n  \\begin{axis}[domain=-1:1, y domain=-1:1, xlabel={$x$}, ylabel={$y$}, zlabel={$z$}]\n    \\addplot3[surf] {x*y*exp(x+2*y-9*x^2-9*y^2)};\n  \\end{axis}\n\\end{tikzpicture}",
                                    ),
                                    (
                                        lang.tr("📐 Geometri Segitiga", "📐 Triangle Geometry"),
                                        "\\begin{tikzpicture}\n\\filldraw[fill=blue!20, draw=blue, thick] (0,0) node[below left]{$A$} -- (4,0) node[below right]{$B$} -- (1.5,2.5) node[above]{$C$} -- cycle;\n\\draw[red, dashed] (1.5,2.5) -- (1.5,0) node[below]{$H$};\n\\end{tikzpicture}",
                                    ),
                                    (
                                        lang.tr("⭕ Geometri Lingkaran", "⭕ Circle Geometry"),
                                        "\\begin{tikzpicture}\n\\draw[blue, thick] (0,0) circle (2);\n\\draw[red, ->, thick] (0,0) node[below left]{$O$} -- (1.41,1.41) node[midway, above left]{$r$};\n\\draw[green] (0.6,0) arc (0:45:0.6);\n\\end{tikzpicture}",
                                    ),
                                    (
                                        lang.tr("🕸 Graf Node-Edge", "🕸 Node-Edge Graph"),
                                        "\\begin{tikzpicture}\n\\node[circle, draw=blue, fill=blue!15] (A) at (0,0) {A};\n\\node[circle, draw=blue, fill=blue!15] (B) at (3,1.2) {B};\n\\node[circle, draw=blue, fill=blue!15] (C) at (3,-1.2) {C};\n\\draw[->, thick] (A) -- node[above]{2} (B);\n\\draw[->, thick] (A) -- node[below]{5} (C);\n\\draw[->, dashed, red] (B) -- node[right]{3} (C);\n\\end{tikzpicture}",
                                    ),
                                    (
                                        lang.tr("📋 Tabel", "📋 Table"),
                                        "\\begin{tabular}{|l|c|r|}\n\\hline\n\\textbf{Fungsi} & \\textbf{Turunan} & \\textbf{Nilai} \\\\\n\\hline\n$x^2$ & $2x$ & 4 \\\\\n$\\sin(x)$ & $\\cos(x)$ & 1 \\\\\n\\hline\n\\end{tabular}",
                                    ),
                                    (
                                        lang.tr("[M] Matriks", "[M] Matrix"),
                                        "A = \\begin{pmatrix}\n1 & 2 \\\\\n3 & 4\n\\end{pmatrix}",
                                    ),
                                ];
                                for (lbl, tpl) in templates {
                                    if ui.button(lbl).clicked() {
                                        state.equation_latex_buffer = tpl.to_string();
                                    }
                                }
                            });

                            ui.separator();

                            ui.label(lang.tr("Kode LaTeX:", "LaTeX Code:"));
                            let text_edit = egui::TextEdit::multiline(&mut state.equation_latex_buffer)
                                .desired_rows(6)
                                .desired_width(f32::INFINITY)
                                .font(egui::TextStyle::Monospace)
                                .hint_text(lang.tr(
                                    r"Contoh rumus, \begin{tabular}, \begin{axis}, atau \begin{tikzpicture}",
                                    r"Formula, \begin{tabular}, \begin{axis}, or \begin{tikzpicture}",
                                ));
                            ui.add(text_edit);

                            ui.separator();

                            // Live Preview Box dengan batas tinggi dan auto-fit + scroll internal
                            ui.label(
                                egui::RichText::new(lang.tr("Pratinjau Visual:", "Visual Preview:"))
                                    .size(12.0)
                                    .strong(),
                            );

                            match &parse_result {
                                Ok(nodes) if !is_empty => {
                                    let preview_font_size = state.equation_font_size;
                                    let layout = crate::geometry::math::MathLayout::compute(
                                        nodes,
                                        preview_font_size,
                                    );

                                    let bg_col = if ui.visuals().dark_mode {
                                        Color32::from_rgb(32, 32, 36)
                                    } else {
                                        Color32::from_rgb(248, 249, 250)
                                    };
                                    let border_col = if ui.visuals().dark_mode {
                                        Color32::from_rgb(60, 60, 65)
                                    } else {
                                        Color32::from_rgb(220, 224, 230)
                                    };
                                    let text_col = if ui.visuals().dark_mode {
                                        Color32::from_rgb(240, 240, 245)
                                    } else {
                                        Color32::from_rgb(20, 20, 25)
                                    };

                                    egui::Frame::group(ui.style())
                                        .fill(bg_col)
                                        .stroke(Stroke::new(1.0_f32, border_col))
                                        .corner_radius(4.0)
                                        .inner_margin(8.0)
                                        .show(ui, |ui| {
                                            egui::ScrollArea::both()
                                                .max_height(280.0)
                                                .auto_shrink([false, true])
                                                .show(ui, |ui| {
                                                    let avail_w = ui.available_width().max(260.0);
                                                    let preview_scale = if layout.width + 16.0 > avail_w {
                                                        (avail_w / (layout.width + 16.0)).clamp(0.48, 1.0)
                                                    } else {
                                                        1.0
                                                    };
                                                    let drawn_w = layout.width * preview_scale;
                                                    let drawn_h = layout.height * preview_scale;
                                                    let box_w = (drawn_w + 24.0).max(avail_w);
                                                    let box_h = (drawn_h + 16.0).max(60.0);
                                                    let (rect, _resp) = ui.allocate_exact_size(
                                                        Vec2::new(box_w, box_h),
                                                        egui::Sense::hover(),
                                                    );
                                                    if ui.is_rect_visible(rect) {
                                                        let painter = ui.painter_at(rect);
                                                        let render_x = rect.min.x
                                                            + (rect.width() - drawn_w).max(0.0) * 0.5;
                                                        let render_y = rect.min.y
                                                            + (rect.height() - drawn_h).max(0.0) * 0.5;
                                                        layout.render(
                                                            &painter,
                                                            Pos2::new(render_x, render_y),
                                                            preview_scale,
                                                            0.0,
                                                            text_col,
                                                        );
                                                    }
                                                });
                                        });
                                }
                                Ok(_) => {
                                    ui.label(
                                        egui::RichText::new(lang.tr(
                                            "Ketik kode LaTeX atau pilih template di atas untuk melihat pratinjau visual.",
                                            "Enter LaTeX code or pick a template above to see a visual preview.",
                                        ))
                                        .italics()
                                        .color(Color32::GRAY),
                                    );
                                }
                                Err(err) => {
                                    ui.colored_label(
                                        Color32::from_rgb(235, 87, 87),
                                        format!(
                                            "⚠️ {}: {}",
                                            lang.tr("Kesalahan sintaks", "Syntax error"),
                                            err
                                        ),
                                    );
                                    ui.label(
                                        egui::RichText::new(lang.tr(
                                            "Catatan: Mendukung rumus matematika, tabel (tabular/array/pmatrix/cases), grafik 2D & 3D (axis/addplot/addplot3), geometri & graf (tikzpicture), maupun dokumen LaTeX lengkap (\\begin{document}).",
                                            "Note: Supports math formulas, tables (tabular/array/pmatrix/cases), 2D & 3D plots (axis/addplot/addplot3), TikZ geometry & graphs, and full LaTeX documents (\\begin{document}).",
                                        ))
                                        .size(11.0)
                                        .color(Color32::GRAY),
                                    );
                                }
                            }
                        });
                });
            });
        });

    if !open || close_action {
        state.show_equation_dialog = false;
        state.editing_equation_id = None;
        state.equation_target_page_id = None;
        state.equation_latex_buffer.clear();
    } else if insert_action {
        let clean_latex = state.equation_latex_buffer.trim().to_string();
        if !clean_latex.is_empty() {
            state.push_undo(&page.id, &page.objects);
            if let Some(ref edit_id) = state.editing_equation_id {
                for obj in &mut page.objects {
                    if obj.id() == edit_id {
                        if let DrawObject::Equation {
                            latex, font_size, ..
                        } = obj
                        {
                            *latex = clean_latex.clone();
                            *font_size = state.equation_font_size;
                        }
                        break;
                    }
                }
                state.selected_ids = vec![edit_id.clone()];
            } else {
                let new_id = uuid::Uuid::new_v4().to_string();
                let insert_pos = state.equation_insert_pos;
                let new_obj = DrawObject::Equation {
                    id: new_id.clone(),
                    latex: clean_latex,
                    x: insert_pos.x,
                    y: insert_pos.y,
                    font_size: state.equation_font_size,
                    scale: 1.0,
                    color: state.text_color.into(),
                    rotation: 0.0,
                    locked: false,
                };
                page.objects.push(new_obj);
                state.selected_ids = vec![new_id];
            }
            // Alihkan ke alat Lasso agar objek LaTeX yang baru dibuat/disunting dapat langsung digeser dan di-resize
            state.current_tool = crate::canvas::types::ActiveTool::Lasso;
            state.rebuild_spatial_grid(&page.id, &page.objects);
        }
        state.show_equation_dialog = false;
        state.editing_equation_id = None;
        state.equation_target_page_id = None;
        state.equation_latex_buffer.clear();
    }
}

use super::laser::{smooth_laser_points, tessellate_laser_layer};
use super::primitives::{
    draw_non_stroke_object, draw_paper_pattern, draw_rotated_background_image,
    draw_shape_primitive, transform_page_mesh_to_screen,
};
use super::state::CanvasState;
use super::types::{ActiveTool, EraserMode, ResizeCorner};
use crate::geometry::{
    ShapeTransformBox, TransformHandleKind, calculate_auto_fit_font_size,
    calculate_proportional_scale, calculate_shape_text_bounds, calculate_single_axis_resize,
    change_shape_kind, combined_bounds, compute_transform_handles, connector_polyline,
    find_alignment_guides, find_connector_snap_site, hit_test_connector_details,
    hit_test_connector_midpoint, hit_test_connector_with_objects, hit_test_object,
    hit_test_transform_handles, insert_control_point_at, next_shape_rect, object_bounds,
    object_in_lasso, prune_collinear_control_points, prune_invalid_connector_control_points,
    recognize_handwriting_from_strokes, recognize_shape, resolve_connector_endpoints, scale_object,
    shape_connection_point, should_reject_palm_input, smooth_stroke_points, snap_point_to_grid,
    snap_value_to_grid, tessellate_stroke_ex, translate_object,
};
use crate::model::{
    ColorData, ConnectionSite, ConnectorEnd, ConnectorRoute, ConnectorStyle, DrawObject,
    MarkerKind, Page, Point2D, ShapeKind, StrokePoint,
};
use eframe::egui::{
    self, Color32, CursorIcon, Key, PointerButton, Pos2, Rect, Response, Sense, Stroke, Ui, Vec2,
};
use std::time::Instant;
use uuid::Uuid;

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

pub fn render_page_canvas(ui: &mut Ui, page: &mut Page, state: &mut CanvasState) -> Response {
    let desired_size = Vec2::new(page.width * state.zoom, page.height * state.zoom);
    let (rect, response) = ui.allocate_exact_size(desired_size, Sense::click_and_drag());
    // Fase 8.9: Metadata Aksesibilitas (AccessKit) untuk kanvas halaman
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Other, true, "Kanvas Catatan RustNotes")
    });
    let painter = ui.painter_at(rect);

    let zoom = state.zoom;
    let origin = rect.min;
    let screen_clip_rect = ui.clip_rect();

    // Transform from screen coordinates to page coordinates
    let screen_to_page = move |screen_pos: Pos2| -> Pos2 {
        let rel = screen_pos - origin;
        Pos2::new(rel.x / zoom, rel.y / zoom)
    };

    let page_to_screen =
        move |page_pos: Pos2| -> Pos2 { origin + Vec2::new(page_pos.x * zoom, page_pos.y * zoom) };

    // Pre-cache textures for background and image objects
    if let crate::model::PageBackground::Image { asset_id }
    | crate::model::PageBackground::Pdf { asset_id, .. } = page.effective_background()
    {
        state.get_or_load_texture(ui.ctx(), &asset_id);
    }
    for obj in &page.objects {
        if let DrawObject::Image { asset_id, .. } = obj {
            state.get_or_load_texture(ui.ctx(), asset_id);
        }
    }

    // 1. Draw Paper Background and Patterns
    painter.rect_filled(rect, 4.0, page.paper_color.to_color32());

    match page.effective_background() {
        crate::model::PageBackground::Template(template) => {
            draw_paper_pattern(&painter, rect, template, state.zoom);
        }
        crate::model::PageBackground::Image { asset_id }
        | crate::model::PageBackground::Pdf { asset_id, .. } => {
            if let Some(tex) = state.get_texture(&asset_id) {
                draw_rotated_background_image(&painter, tex.id(), rect, page.rotation);
            }
        }
    }

    // 2. Draw existing page objects with Viewport Culling & Page-Space Mesh Cache
    for obj in &page.objects {
        // Culling: skip objects entirely outside the visible viewport
        let obj_b = object_bounds(obj);
        let screen_b = Rect::from_min_max(page_to_screen(obj_b.min), page_to_screen(obj_b.max));
        if !screen_clip_rect.intersects(screen_b) {
            continue;
        }

        match obj {
            DrawObject::Stroke {
                id,
                points,
                color,
                width,
                is_highlighter,
            } => {
                // Simpan mesh di dalam cache menggunakan koordinat lokal kertas (`|p| p`, skala 1.0),
                // lalu transformasikan ke posisi layar (`origin` + `state.zoom`) setiap frame agar
                // goresan tidak pernah bergeser atau hilang ketika halaman di-scroll maupun di-zoom.
                let key = (id.clone(), 0);
                let page_mesh = if let Some(cached) = state.mesh_cache.get(&key) {
                    cached.clone()
                } else {
                    let smoothed = smooth_stroke_points(points, state.smoothing_steps);
                    let stroke_c = if *is_highlighter {
                        Color32::from_rgba_unmultiplied(color.r, color.g, color.b, 90)
                    } else {
                        color.to_color32()
                    };
                    let mesh =
                        tessellate_stroke_ex(&smoothed, *width, stroke_c, *is_highlighter, |p| p);
                    if state.mesh_cache.len() > 1500 {
                        state.mesh_cache.clear();
                    }
                    state.mesh_cache.insert(key, mesh.clone());
                    mesh
                };
                let screen_mesh = transform_page_mesh_to_screen(&page_mesh, origin, state.zoom);
                painter.add(screen_mesh);
            }
            _ => {
                // Jangan gambar teks statis jika objek sedang disunting secara inline oleh TextEdit
                if state.editing_text_id.as_deref() == Some(obj.id())
                    && matches!(obj, DrawObject::Text { .. })
                {
                    continue;
                }
                draw_non_stroke_object(
                    &painter,
                    obj,
                    &page.objects,
                    page_to_screen,
                    state.zoom,
                    |id| state.get_texture(id).map(|t| t.id()),
                );
            }
        }
    }

    // 3. User interaction logic
    let pointer_pos = response.interact_pointer_pos();

    // Laser decay update (idle-based persistence matching user reference)
    let now = Instant::now();
    let idle_threshold = 1.0_f32; // tahan 1.0 detik setelah berhenti menggambar
    let fade_duration = 0.7_f32; // fade out halus selama 0.7 detik

    let elapsed = now.duration_since(state.laser_last_active).as_secs_f32();
    let laser_alpha = if state.laser_is_drawing || elapsed < idle_threshold {
        1.0_f32 // jika sedang menggambar atau jeda waktu singkat: TETAP 100% PERSISTEN
    } else {
        let fade_progress = (elapsed - idle_threshold) / fade_duration;
        (1.0 - fade_progress).clamp(0.0, 1.0)
    };

    if laser_alpha <= 0.0 && !state.laser_is_drawing {
        state.laser_strokes.clear();
        state.current_laser_stroke.clear();
    } else if !state.laser_strokes.is_empty() || !state.current_laser_stroke.is_empty() {
        ui.ctx().request_repaint();
    }

    // Active Selection Bounds
    let (
        selection_rect,
        selected_single_line,
        selected_single_shape,
        selected_single_connector,
        selected_single_shape_quick_dots,
        is_single_selected,
        is_selection_fully_locked,
    ) = {
        let selected: Vec<&DrawObject> = page
            .objects
            .iter()
            .filter(|o| state.selected_ids.contains(&o.id().to_string()))
            .collect();
        let rect = combined_bounds(&selected);
        let single_line = if selected.len() == 1 {
            if let DrawObject::Shape {
                shape_type: ShapeKind::Line,
                x,
                y,
                width,
                height,
                locked,
                ..
            } = selected[0]
            {
                if *locked {
                    None
                } else {
                    Some((Pos2::new(*x, *y), Pos2::new(*x + *width, *y + *height)))
                }
            } else {
                None
            }
        } else {
            None
        };
        let single_shape = if selected.len() == 1 {
            if let DrawObject::Shape {
                x,
                y,
                width,
                height,
                rotation,
                ..
            } = selected[0]
            {
                Some((*x, *y, *width, *height, *rotation))
            } else {
                None
            }
        } else {
            None
        };
        let single_connector = if selected.len() == 1 {
            if let DrawObject::Connector { id, .. } = selected[0] {
                Some((id.clone(), connector_polyline(selected[0], &page.objects)))
            } else {
                None
            }
        } else {
            None
        };
        let is_selection_fully_locked =
            !selected.is_empty() && selected.iter().all(|o| o.is_locked());
        let quick_dots = if selected.len() == 1
            && let DrawObject::Shape { id, locked, .. } = selected[0]
            && !*locked
        {
            let shape = selected[0];
            let sites = [
                ConnectionSite::Top,
                ConnectionSite::Right,
                ConnectionSite::Bottom,
                ConnectionSite::Left,
            ];
            let dots: Vec<(ConnectionSite, Pos2, Point2D)> = sites
                .iter()
                .map(|&site| {
                    let (pt, dir) = shape_connection_point(shape, site, None);
                    let dot_pt = pt + dir.to_vector() * 12.0;
                    (site, dot_pt.to_pos2(), pt)
                })
                .collect();
            Some((id.clone(), dots))
        } else {
            None
        };
        (
            rect,
            single_line,
            single_shape,
            single_connector,
            quick_dots,
            selected.len() == 1,
            is_selection_fully_locked,
        )
    };

    if response.drag_started_by(PointerButton::Primary)
        && let Some(pos) = pointer_pos
    {
        let page_p = screen_to_page(pos);

        // Check if clicking resize/rotation handles on selection
        if let Some((line_start, line_end)) = selected_single_line {
            let handle_dist = 14.0 / state.zoom;
            let dir = line_end - line_start;
            let dir_norm = if dir.length() > 1e-3 {
                dir / dir.length()
            } else {
                Vec2::new(1.0, 0.0)
            };
            let line_rot_handle = line_end + dir_norm * (22.0 / state.zoom);

            // Handle rotasi (⟳) di ujung titik akhir garis (berputar terhadap titik awal)
            if (page_p - line_rot_handle).length() <= handle_dist {
                state.resize_corner = Some(ResizeCorner::Rotation);
                state.drag_start_pos = Some(page_p);
                state.push_undo(&page.id, &page.objects);
                return response;
            }
            // Handle titik akhir garis (dapat berotasi & diatur terhadap titik awal sebagai pusat)
            if (page_p - line_end).length() <= handle_dist {
                state.resize_corner = Some(ResizeCorner::LineEnd);
                state.drag_start_pos = Some(page_p);
                state.push_undo(&page.id, &page.objects);
                return response;
            }
            // Handle titik awal garis (pusat rotasi)
            if (page_p - line_start).length() <= handle_dist {
                state.resize_corner = Some(ResizeCorner::LineStart);
                state.drag_start_pos = Some(page_p);
                state.push_undo(&page.id, &page.objects);
                return response;
            }
            // Klik pada badan garis untuk menggeser garis (hanya saat menggunakan alat Lasso)
            if state.current_tool == ActiveTool::Lasso
                && crate::geometry::point_to_segment_dist(page_p, line_start, line_end)
                    <= handle_dist
            {
                state.is_dragging_selection = true;
                state.drag_start_pos = Some(page_p);
                state.push_undo(&page.id, &page.objects);
                return response;
            }
        } else if let Some((source_id, dots)) = &selected_single_shape_quick_dots {
            let dot_hit_dist = 14.0 / state.zoom;
            if let Some(&(site, _, _)) = dots
                .iter()
                .find(|(_, dpos, _)| (page_p - *dpos).length() <= dot_hit_dist)
            {
                state.resize_corner = Some(ResizeCorner::QuickDiagramDot(site));
                state.drag_start_pos = Some(page_p);
                state.draft_connector_start = Some(ConnectorEnd::Attached {
                    shape_id: source_id.clone(),
                    site,
                });
                state.draft_connector_current = Some(Point2D::new(page_p.x, page_p.y));
                return response;
            }
        }

        if let Some((conn_id, Some(poly))) = &selected_single_connector {
            let is_conn_locked = page
                .objects
                .iter()
                .any(|o| o.id() == conn_id && o.is_locked());
            if !is_conn_locked {
                let handle_dist = 14.0 / state.zoom;
                // 1. Cek midpoint handles untuk menambah tikungan baru (Task 8.2)
                let start_pt = poly[0];
                let end_pt = poly[poly.len() - 1];
                if let Some((_seg_idx, mid_pt)) =
                    hit_test_connector_midpoint(Point2D::new(page_p.x, page_p.y), poly, handle_dist)
                {
                    state.push_undo(&page.id, &page.objects);
                    let mut new_idx = 0;
                    for obj in &mut page.objects {
                        if obj.id() == conn_id
                            && let DrawObject::Connector { control_points, .. } = obj
                        {
                            new_idx =
                                insert_control_point_at(control_points, start_pt, end_pt, mid_pt);
                        }
                    }
                    state.resize_corner = Some(ResizeCorner::ConnectorControl(new_idx));
                    state.drag_start_pos = Some(page_p);
                    return response;
                } else if let Some(target) =
                    hit_test_connector_details(Point2D::new(page_p.x, page_p.y), poly, handle_dist)
                {
                    let corner = match target {
                        crate::geometry::ConnectorHitTarget::StartHandle => {
                            ResizeCorner::ConnectorStart
                        }
                        crate::geometry::ConnectorHitTarget::EndHandle => {
                            ResizeCorner::ConnectorEnd
                        }
                        crate::geometry::ConnectorHitTarget::ControlPoint(idx) => {
                            let cp_idx = if idx > 0 { idx - 1 } else { 0 };
                            ResizeCorner::ConnectorControl(cp_idx)
                        }
                        crate::geometry::ConnectorHitTarget::LineSegment(_seg_idx) => {
                            state.push_undo(&page.id, &page.objects);
                            let mut new_idx = 0;
                            for obj in &mut page.objects {
                                if obj.id() == conn_id
                                    && let DrawObject::Connector { control_points, .. } = obj
                                {
                                    new_idx = insert_control_point_at(
                                        control_points,
                                        start_pt,
                                        end_pt,
                                        Point2D::new(page_p.x, page_p.y),
                                    );
                                }
                            }
                            ResizeCorner::ConnectorControl(new_idx)
                        }
                    };
                    state.resize_corner = Some(corner);
                    state.drag_start_pos = Some(page_p);
                    state.push_undo(&page.id, &page.objects);
                    return response;
                }
            }
        } else if matches!(state.current_tool, ActiveTool::Lasso | ActiveTool::Shape(_))
            && let Some(b) = selection_rect
            && !is_selection_fully_locked
        {
            let handle_dist = 14.0 / state.zoom;
            let (bx, by, bw, bh, brot) = if let Some(s) = selected_single_shape {
                s
            } else {
                (b.min.x, b.min.y, b.width(), b.height(), 0.0)
            };

            let transform_handles =
                compute_transform_handles(bx, by, bw, bh, brot, 20.0 / state.zoom);
            if let Some(h_kind) = hit_test_transform_handles(
                &transform_handles,
                Point2D::new(page_p.x, page_p.y),
                handle_dist,
            ) {
                let corner = match h_kind {
                    TransformHandleKind::NorthWest => ResizeCorner::NorthWest,
                    TransformHandleKind::North => ResizeCorner::North,
                    TransformHandleKind::NorthEast => ResizeCorner::NorthEast,
                    TransformHandleKind::East => ResizeCorner::East,
                    TransformHandleKind::SouthEast => ResizeCorner::SouthEast,
                    TransformHandleKind::South => ResizeCorner::South,
                    TransformHandleKind::SouthWest => ResizeCorner::SouthWest,
                    TransformHandleKind::West => ResizeCorner::West,
                    TransformHandleKind::Rotation => ResizeCorner::Rotation,
                };
                state.resize_corner = Some(corner);
                state.drag_start_pos = Some(page_p);
                state.push_undo(&page.id, &page.objects);
                return response;
            }

            // Check if clicking inside selection to move
            if b.contains(page_p) {
                state.is_dragging_selection = true;
                state.drag_start_pos = Some(page_p);
                state.push_undo(&page.id, &page.objects);
                return response;
            }
        }

        match state.current_tool {
            ActiveTool::Pen | ActiveTool::Pencil | ActiveTool::Highlighter => {
                // Fase 8.5: Palm Rejection — tolak awal goresan jika multi-touch terdeteksi
                let multi_touch = ui.input(|i| i.multi_touch().is_some());
                if !(state.palm_rejection_enabled && multi_touch) {
                    state.active_stroke_page_id = Some(page.id.clone());
                    state.draft_stroke = Some(vec![StrokePoint::from(page_p)]);
                    state.last_stroke_move_time = Some(Instant::now());
                    state.recognized_draft_shape = None;
                    state.selected_ids.clear();
                }
            }
            ActiveTool::Shape(_) => {
                state.active_stroke_page_id = Some(page.id.clone());
                let mut start_p = page_p;
                if state.snap_to_grid {
                    let (snapped, _) = snap_point_to_grid(
                        Point2D::new(page_p.x, page_p.y),
                        state.grid_spacing,
                        8.0 / state.zoom,
                    );
                    start_p = snapped.to_pos2();
                }
                state.draft_shape_start = Some(start_p);
                state.draft_shape_current = Some(start_p);
                state.selected_ids.clear();
            }
            ActiveTool::Connector => {
                state.active_stroke_page_id = Some(page.id.clone());
                let snap = find_connector_snap_site(
                    &page.objects,
                    Point2D::new(page_p.x, page_p.y),
                    24.0 / state.zoom,
                    None,
                );
                let start_end = if let Some((shape_id, site, _pt)) = snap {
                    ConnectorEnd::Attached { shape_id, site }
                } else {
                    ConnectorEnd::Free {
                        x: page_p.x,
                        y: page_p.y,
                    }
                };
                state.draft_connector_start = Some(start_end);
                state.draft_connector_current = Some(Point2D::new(page_p.x, page_p.y));
                state.connector_snap_candidate = None;
                state.selected_ids.clear();
            }
            ActiveTool::Eraser => {
                state.eraser_undo_pushed = false;
                state.last_whole_erase_pos = None;
                let erase_radius = match state.eraser_mode {
                    EraserMode::WholeStroke => (3.8 / state.zoom).clamp(1.8, 4.5),
                    EraserMode::Partial => (10.0 / state.zoom).clamp(4.0, 16.0),
                };
                state.erase_at(page, page_p, erase_radius);
            }
            ActiveTool::Lasso => {
                state.active_stroke_page_id = Some(page.id.clone());
                state.selected_ids.clear();
                state.lasso_points = vec![page_p];
            }
            ActiveTool::Laser => {
                state.laser_is_drawing = true;
                state.laser_last_active = Instant::now();
                state.current_laser_stroke = vec![pos];
                ui.ctx().request_repaint();
            }
            ActiveTool::Text => {
                // Jika klik/drag pada objek Teks atau StickyNote yang sudah ada, sunting objek tersebut
                let hit_id = page
                    .objects
                    .iter()
                    .rev()
                    .find(|obj| {
                        matches!(obj, DrawObject::Text { .. } | DrawObject::StickyNote { .. })
                            && hit_test_object(obj, page_p, 8.0 / state.zoom)
                    })
                    .map(|o| o.id().to_string());
                if let Some(id) = hit_id {
                    state.selected_ids = vec![id.clone()];
                    state.editing_text_id = Some(id);
                    state.focus_text_edit = true;
                } else {
                    state.push_undo(&page.id, &page.objects);
                    let id = Uuid::new_v4().to_string();
                    let new_text = DrawObject::Text {
                        id: id.clone(),
                        x: page_p.x,
                        y: page_p.y,
                        text: String::new(),
                        font_size: (state.stroke_width * 4.0 + 14.0).clamp(12.0, 72.0),
                        color: state.stroke_color.into(),
                        rotation: 0.0,
                    };
                    page.objects.push(new_text);
                    state.spatial_grid.rebuild(&page.objects);
                    state.selected_ids = vec![id.clone()];
                    state.editing_text_id = Some(id);
                    state.focus_text_edit = true;
                }
            }
            ActiveTool::StickyNote => {
                state.push_undo(&page.id, &page.objects);
                let id = Uuid::new_v4().to_string();
                let new_note = DrawObject::StickyNote {
                    id: id.clone(),
                    x: page_p.x,
                    y: page_p.y,
                    width: 180.0,
                    height: 140.0,
                    text: String::new(),
                    color: Color32::from_rgb(254, 243, 189).into(),
                };
                page.objects.push(new_note);
                state.spatial_grid.rebuild(&page.objects);
                state.selected_ids = vec![id.clone()];
                state.editing_text_id = Some(id);
                state.focus_text_edit = true;
                state.current_tool = ActiveTool::Lasso;
            }
            ActiveTool::Hand => {}
        }
    }

    if response.dragged_by(PointerButton::Primary)
        && let Some(pos) = pointer_pos
    {
        let page_p = screen_to_page(pos);

        // Handle moving selection
        if state.is_dragging_selection {
            if let Some(start) = state.drag_start_pos {
                let mut dx = page_p.x - start.x;
                let mut dy = page_p.y - start.y;

                // Shift-drag: Constrain ke gerakan ortogonal murni (Horizontal atau Vertikal)
                let shift_held = ui.input(|i| i.modifiers.shift);
                if shift_held {
                    if dx.abs() > dy.abs() {
                        dy = 0.0;
                    } else {
                        dx = 0.0;
                    }
                }

                state.active_alignment_guides.clear();

                // Evaluasi Smart Alignment Guides & Grid Snapping
                let selected_objects: Vec<&DrawObject> = page
                    .objects
                    .iter()
                    .filter(|o| state.selected_ids.contains(&o.id().to_string()) && !o.is_locked())
                    .collect();

                if let Some(moving_bounds) = combined_bounds(&selected_objects) {
                    let tentative_rect = moving_bounds.translate(Vec2::new(dx, dy));

                    if state.smart_guides {
                        let target_rects: Vec<Rect> = page
                            .objects
                            .iter()
                            .filter(|o| !state.selected_ids.contains(&o.id().to_string()))
                            .map(object_bounds)
                            .collect();

                        let (snap_adjust, guides) =
                            find_alignment_guides(tentative_rect, &target_rects, 8.0 / state.zoom);
                        dx += snap_adjust.x;
                        dy += snap_adjust.y;
                        state.active_alignment_guides = guides;
                    } else if state.snap_to_grid {
                        let (snap_x, snapped_x) = snap_value_to_grid(
                            tentative_rect.left(),
                            state.grid_spacing,
                            8.0 / state.zoom,
                        );
                        if snapped_x {
                            dx += snap_x - tentative_rect.left();
                        }
                        let (snap_y, snapped_y) = snap_value_to_grid(
                            tentative_rect.top(),
                            state.grid_spacing,
                            8.0 / state.zoom,
                        );
                        if snapped_y {
                            dy += snap_y - tentative_rect.top();
                        }
                    }
                }

                for obj in &mut page.objects {
                    if state.selected_ids.contains(&obj.id().to_string()) && !obj.is_locked() {
                        translate_object(obj, dx, dy);
                        let id = obj.id().to_string();
                        state.mesh_cache.retain(|(k, _), _| k != &id);
                    }
                }
                state.drag_start_pos = Some(page_p);
            }
        } else if let Some(corner) = state.resize_corner {
            if let Some(start) = state.drag_start_pos
                && let Some(b) = selection_rect
            {
                if corner == ResizeCorner::LineEnd {
                    for obj in &mut page.objects {
                        if state.selected_ids.contains(&obj.id().to_string())
                            && let DrawObject::Shape {
                                shape_type: ShapeKind::Line,
                                x,
                                y,
                                width,
                                height,
                                ..
                            } = obj
                        {
                            // Titik awal (x, y) tetap sebagai pusat, titik akhir berotasi mengikuti kursor
                            *width = page_p.x - *x;
                            *height = page_p.y - *y;
                        }
                    }
                    state.drag_start_pos = Some(page_p);
                } else if corner == ResizeCorner::LineStart {
                    for obj in &mut page.objects {
                        if state.selected_ids.contains(&obj.id().to_string())
                            && let DrawObject::Shape {
                                shape_type: ShapeKind::Line,
                                x,
                                y,
                                width,
                                height,
                                ..
                            } = obj
                        {
                            let end_x = *x + *width;
                            let end_y = *y + *height;
                            *x = page_p.x;
                            *y = page_p.y;
                            *width = end_x - *x;
                            *height = end_y - *y;
                        }
                    }
                    state.drag_start_pos = Some(page_p);
                } else if corner == ResizeCorner::Rotation {
                    let pivot = if let Some((line_start, _)) = selected_single_line {
                        line_start
                    } else {
                        b.center()
                    };
                    let v_start = start - pivot;
                    let v_curr = page_p - pivot;
                    let mut angle = v_curr.y.atan2(v_curr.x) - v_start.y.atan2(v_start.x);
                    if ui.input(|i| i.modifiers.shift) {
                        let step = std::f32::consts::TAU / 24.0;
                        angle = (angle / step).round() * step;
                    }
                    if angle.abs() > 0.001 {
                        for obj in &mut page.objects {
                            if state.selected_ids.contains(&obj.id().to_string()) {
                                crate::geometry::rotate_object(obj, pivot, angle);
                                let id = obj.id().to_string();
                                state.mesh_cache.retain(|(k, _), _| k != &id);
                            }
                        }
                        state.drag_start_pos = Some(page_p);
                    }
                } else if matches!(
                    corner,
                    ResizeCorner::ConnectorStart
                        | ResizeCorner::ConnectorEnd
                        | ResizeCorner::ConnectorControl(_)
                ) {
                    let shapes_snapshot: Vec<DrawObject> = page
                        .objects
                        .iter()
                        .filter(|o| matches!(o, DrawObject::Shape { .. }))
                        .cloned()
                        .collect();
                    let snap = find_connector_snap_site(
                        &shapes_snapshot,
                        Point2D::new(page_p.x, page_p.y),
                        24.0 / state.zoom,
                        None,
                    );
                    state.connector_snap_candidate = snap.clone();
                    for obj in &mut page.objects {
                        if state.selected_ids.contains(&obj.id().to_string())
                            && let DrawObject::Connector {
                                start,
                                end,
                                route,
                                control_points,
                                ..
                            } = obj
                        {
                            match corner {
                                ResizeCorner::ConnectorStart => {
                                    if let Some((shape_id, site, _)) = &snap {
                                        *start = ConnectorEnd::Attached {
                                            shape_id: shape_id.clone(),
                                            site: *site,
                                        };
                                    } else {
                                        *start = ConnectorEnd::Free {
                                            x: page_p.x,
                                            y: page_p.y,
                                        };
                                    }
                                }
                                ResizeCorner::ConnectorEnd => {
                                    if let Some((shape_id, site, _)) = &snap {
                                        *end = ConnectorEnd::Attached {
                                            shape_id: shape_id.clone(),
                                            site: *site,
                                        };
                                    } else {
                                        *end = ConnectorEnd::Free {
                                            x: page_p.x,
                                            y: page_p.y,
                                        };
                                    }
                                }
                                ResizeCorner::ConnectorControl(idx) => {
                                    let mut target_pt = Point2D::new(page_p.x, page_p.y);
                                    if *route == ConnectorRoute::Orthogonal {
                                        let prev = if idx == 0 {
                                            resolve_connector_endpoints(
                                                start,
                                                end,
                                                &shapes_snapshot,
                                            )
                                            .0
                                            .0
                                        } else if idx - 1 < control_points.len() {
                                            control_points[idx - 1]
                                        } else {
                                            target_pt
                                        };
                                        let next = if idx + 1 < control_points.len() {
                                            control_points[idx + 1]
                                        } else {
                                            resolve_connector_endpoints(
                                                start,
                                                end,
                                                &shapes_snapshot,
                                            )
                                            .1
                                            .0
                                        };
                                        if (target_pt.x - prev.x).abs() < 8.0 {
                                            target_pt.x = prev.x;
                                        } else if (target_pt.x - next.x).abs() < 8.0 {
                                            target_pt.x = next.x;
                                        }
                                        if (target_pt.y - prev.y).abs() < 8.0 {
                                            target_pt.y = prev.y;
                                        } else if (target_pt.y - next.y).abs() < 8.0 {
                                            target_pt.y = next.y;
                                        }
                                    }
                                    if idx < control_points.len() {
                                        control_points[idx] = target_pt;
                                    } else {
                                        control_points.push(target_pt);
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    state.drag_start_pos = Some(page_p);
                    ui.ctx().request_repaint();
                } else if matches!(corner, ResizeCorner::QuickDiagramDot(_)) {
                    state.draft_connector_current = Some(Point2D::new(page_p.x, page_p.y));
                    let shapes_snapshot: Vec<DrawObject> = page
                        .objects
                        .iter()
                        .filter(|o| matches!(o, DrawObject::Shape { .. }))
                        .cloned()
                        .collect();
                    let snap = find_connector_snap_site(
                        &shapes_snapshot,
                        Point2D::new(page_p.x, page_p.y),
                        24.0 / state.zoom,
                        state.draft_connector_start.as_ref().and_then(|s| match s {
                            ConnectorEnd::Attached { shape_id, .. } => Some(shape_id.as_str()),
                            _ => None,
                        }),
                    );
                    state.connector_snap_candidate = snap;
                    ui.ctx().request_repaint();
                } else if is_single_selected
                    && let Some(obj) = page
                        .objects
                        .iter_mut()
                        .find(|o| state.selected_ids.contains(&o.id().to_string()))
                    && matches!(obj, DrawObject::Shape { .. })
                {
                    if let DrawObject::Shape {
                        x,
                        y,
                        width,
                        height,
                        rotation,
                        locked,
                        ..
                    } = obj
                        && !*locked
                    {
                        let orig_box = ShapeTransformBox::new(*x, *y, *width, *height, *rotation);
                        match corner {
                            ResizeCorner::NorthWest
                            | ResizeCorner::NorthEast
                            | ResizeCorner::SouthEast
                            | ResizeCorner::SouthWest => {
                                let h_kind = match corner {
                                    ResizeCorner::NorthWest => TransformHandleKind::NorthWest,
                                    ResizeCorner::NorthEast => TransformHandleKind::NorthEast,
                                    ResizeCorner::SouthEast => TransformHandleKind::SouthEast,
                                    ResizeCorner::SouthWest => TransformHandleKind::SouthWest,
                                    _ => TransformHandleKind::SouthEast,
                                };
                                let (nx, ny, nw, nh) = calculate_proportional_scale(
                                    h_kind,
                                    Point2D::new(page_p.x, page_p.y),
                                    orig_box,
                                    4.0,
                                );
                                *x = nx;
                                *y = ny;
                                *width = nw;
                                *height = nh;
                            }
                            ResizeCorner::North
                            | ResizeCorner::South
                            | ResizeCorner::East
                            | ResizeCorner::West => {
                                let h_kind = match corner {
                                    ResizeCorner::North => TransformHandleKind::North,
                                    ResizeCorner::South => TransformHandleKind::South,
                                    ResizeCorner::East => TransformHandleKind::East,
                                    ResizeCorner::West => TransformHandleKind::West,
                                    _ => TransformHandleKind::East,
                                };
                                let (nx, ny, nw, nh) = calculate_single_axis_resize(
                                    h_kind,
                                    Point2D::new(page_p.x, page_p.y),
                                    orig_box,
                                    4.0,
                                );
                                *x = nx;
                                *y = ny;
                                *width = nw;
                                *height = nh;
                            }
                            _ => {}
                        }
                        let id = obj.id().to_string();
                        state.mesh_cache.retain(|(k, _), _| k != &id);
                    }
                    state.drag_start_pos = Some(page_p);
                } else {
                    let is_corner_handle = matches!(
                        corner,
                        ResizeCorner::NorthWest
                            | ResizeCorner::NorthEast
                            | ResizeCorner::SouthEast
                            | ResizeCorner::SouthWest
                    );

                    let (origin, sx, sy) = match corner {
                        ResizeCorner::SouthEast => (b.left_top(), true, true),
                        ResizeCorner::NorthWest => (b.right_bottom(), true, true),
                        ResizeCorner::NorthEast => (b.left_bottom(), true, true),
                        ResizeCorner::SouthWest => (b.right_top(), true, true),
                        ResizeCorner::East => (Pos2::new(b.left(), b.center().y), true, false),
                        ResizeCorner::West => (Pos2::new(b.right(), b.center().y), true, false),
                        ResizeCorner::South => (Pos2::new(b.center().x, b.top()), false, true),
                        ResizeCorner::North => (Pos2::new(b.center().x, b.bottom()), false, true),
                        _ => (b.center(), true, true),
                    };

                    let orig_w = b.width().max(1.0);
                    let orig_h = b.height().max(1.0);
                    let mut scale_x = if sx {
                        ((page_p.x - origin.x).abs() / orig_w).max(0.05)
                    } else {
                        1.0
                    };
                    let mut scale_y = if sy {
                        ((page_p.y - origin.y).abs() / orig_h).max(0.05)
                    } else {
                        1.0
                    };

                    if is_corner_handle || ui.input(|i| i.modifiers.shift) {
                        let s = if (scale_x - 1.0).abs() > (scale_y - 1.0).abs() {
                            scale_x
                        } else {
                            scale_y
                        };
                        scale_x = s;
                        scale_y = s;
                    }

                    for obj in &mut page.objects {
                        if state.selected_ids.contains(&obj.id().to_string()) {
                            scale_object(obj, origin, scale_x, scale_y);
                            let id = obj.id().to_string();
                            state.mesh_cache.retain(|(k, _), _| k != &id);
                        }
                    }
                    state.drag_start_pos = Some(page_p);
                }
            }
        } else {
            match state.current_tool {
                ActiveTool::Pen | ActiveTool::Pencil | ActiveTool::Highlighter => {
                    if state.active_stroke_page_id.as_deref() == Some(&page.id) {
                        if let Some(recognized) = &mut state.recognized_draft_shape {
                            let shift_held = ui.input(|i| i.modifiers.shift);
                            match recognized.kind {
                                ShapeKind::Line => {
                                    // Ujung bebas mengikuti kursor dengan 15-degree angle snapping
                                    let mut dx = page_p.x - recognized.x;
                                    let mut dy = page_p.y - recognized.y;
                                    let angle = dy.atan2(dx);
                                    let snap_interval = 15.0_f32.to_radians();
                                    let snapped_angle =
                                        (angle / snap_interval).round() * snap_interval;
                                    let angle_diff = (angle - snapped_angle).abs();
                                    if shift_held || angle_diff < 5.0_f32.to_radians() {
                                        let len = (dx * dx + dy * dy).sqrt();
                                        dx = len * snapped_angle.cos();
                                        dy = len * snapped_angle.sin();
                                    }
                                    recognized.width = dx;
                                    recognized.height = dy;
                                    ui.ctx().request_repaint();
                                }
                                _ => {
                                    // Bentuk 2D (Ellipse, Rectangle, Parallelogram, Triangle, ScaleneTriangle):
                                    // Pertahankan posisi dan dimensi hasil pengenalan geometri asli yang presisi.
                                    // Jangan menimpa dimensi dengan selisih kursor (page_p - min_x) yang menyebabkan
                                    // lingkaran atau bentuk besar menciut menjadi sangat kecil.
                                }
                            }
                        } else if let Some(pts) = &mut state.draft_stroke {
                            let dt_secs = state
                                .last_stroke_move_time
                                .map(|t| now.saturating_duration_since(t).as_secs_f32())
                                .unwrap_or(0.016);
                            let multi_touch = ui.input(|i| i.multi_touch().is_some());
                            // Fase 8.5: Palm Rejection — abaikan titik yang terindikasi sentuhan telapak tangan
                            let rejected = state.palm_rejection_enabled
                                && should_reject_palm_input(pts, page_p, dt_secs, multi_touch);
                            let min_dist = 2.0 / state.zoom;
                            if !rejected
                                && let Some(last) = pts.last()
                                && (page_p - last.to_pos2()).length() >= min_dist
                            {
                                pts.push(StrokePoint::from(page_p));
                                state.last_stroke_move_time = Some(Instant::now());
                            }
                        }
                    }
                }
                ActiveTool::Shape(_) => {
                    if state.active_stroke_page_id.as_deref() == Some(&page.id) {
                        let mut final_p = page_p;
                        if state.snap_to_grid {
                            let (snapped, _) = snap_point_to_grid(
                                Point2D::new(page_p.x, page_p.y),
                                state.grid_spacing,
                                8.0 / state.zoom,
                            );
                            final_p = snapped.to_pos2();
                        }
                        state.draft_shape_current = Some(final_p);
                    }
                }
                ActiveTool::Connector => {
                    if state.active_stroke_page_id.as_deref() == Some(&page.id) {
                        let exclude = match &state.draft_connector_start {
                            Some(ConnectorEnd::Attached { shape_id, .. }) => {
                                Some(shape_id.as_str())
                            }
                            _ => None,
                        };
                        state.connector_snap_candidate = find_connector_snap_site(
                            &page.objects,
                            Point2D::new(page_p.x, page_p.y),
                            24.0 / state.zoom,
                            exclude,
                        );
                        let cur_pt = state
                            .connector_snap_candidate
                            .as_ref()
                            .map(|(_, _, p)| *p)
                            .unwrap_or(Point2D::new(page_p.x, page_p.y));
                        state.draft_connector_current = Some(cur_pt);
                        ui.ctx().request_repaint();
                    }
                }
                ActiveTool::Eraser => {
                    let erase_radius = match state.eraser_mode {
                        EraserMode::WholeStroke => (3.8 / state.zoom).clamp(1.8, 4.5),
                        EraserMode::Partial => (10.0 / state.zoom).clamp(4.0, 16.0),
                    };
                    if state.erase_at(page, page_p, erase_radius) {
                        ui.ctx().request_repaint();
                    }
                }
                ActiveTool::Lasso => {
                    if state.active_stroke_page_id.as_deref() == Some(&page.id) {
                        state.lasso_points.push(page_p);
                    }
                }
                ActiveTool::Laser => {
                    state.laser_is_drawing = true;
                    state.laser_last_active = Instant::now();
                    let should_add = state
                        .current_laser_stroke
                        .last()
                        .is_none_or(|last_p| last_p.distance(pos) > 2.0);
                    if should_add {
                        state.current_laser_stroke.push(pos);
                    }
                    ui.ctx().request_repaint();
                }
                ActiveTool::Hand => {
                    state.pan_offset += response.drag_delta();
                }
                _ => {}
            }
        }
    }

    // Fase 8.1 & G4: Deteksi tahan pena (~500 ms) di akhir goresan untuk Shape Recognition
    if state.auto_shape_enabled
        && state.active_stroke_page_id.as_deref() == Some(&page.id)
        && matches!(
            state.current_tool,
            ActiveTool::Pen | ActiveTool::Pencil | ActiveTool::Highlighter
        )
        && state.recognized_draft_shape.is_none()
        && let Some(pts) = &state.draft_stroke
        && let Some(last_move) = state.last_stroke_move_time
    {
        let elapsed = now.saturating_duration_since(last_move);
        if elapsed >= std::time::Duration::from_millis(500) {
            if let Some(recognized) = recognize_shape(pts) {
                state.recognized_draft_shape = Some(recognized.clone());
                state.snap_initial_shape = Some(recognized);
                state.snap_flash_time = Some(Instant::now());
                ui.ctx().request_repaint();
            } else {
                // Jangan cek berulang kali jika goresan bukan bentuk geometri hingga bergerak lagi
                state.last_stroke_move_time = None;
            }
        } else {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(50));
        }
    }

    if response.drag_stopped() {
        state.active_alignment_guides.clear();
        state.eraser_undo_pushed = false;
        state.last_whole_erase_pos = None;
        if state.laser_is_drawing {
            state.laser_is_drawing = false;
            if !state.current_laser_stroke.is_empty() {
                let stroke = std::mem::take(&mut state.current_laser_stroke);
                state.laser_strokes.push(stroke);
            }
            state.laser_last_active = Instant::now();
        }
        if state.is_dragging_selection {
            state.is_dragging_selection = false;
            state.drag_start_pos = None;
            // Task 8.5: Periksa bila ada shape bergerak yang menimpa control points pada connector
            let shapes_snapshot: Vec<DrawObject> = page
                .objects
                .iter()
                .filter(|o| matches!(o, DrawObject::Shape { .. }))
                .cloned()
                .collect();
            for obj in &mut page.objects {
                prune_invalid_connector_control_points(obj, &shapes_snapshot);
            }
            state.spatial_grid.rebuild(&page.objects);
        } else if let Some(corner) = state.resize_corner.take() {
            state.drag_start_pos = None;
            // Task 8.3: Jika selesai drag control point, bersihkan jika hampir segaris
            if matches!(corner, ResizeCorner::ConnectorControl(_)) {
                let shapes_snapshot: Vec<DrawObject> = page
                    .objects
                    .iter()
                    .filter(|o| matches!(o, DrawObject::Shape { .. }))
                    .cloned()
                    .collect();
                for obj in &mut page.objects {
                    if state.selected_ids.contains(&obj.id().to_string())
                        && let DrawObject::Connector {
                            start,
                            end,
                            control_points,
                            ..
                        } = obj
                    {
                        let ((start_pt, _), (end_pt, _)) =
                            resolve_connector_endpoints(start, end, &shapes_snapshot);
                        prune_collinear_control_points(control_points, start_pt, end_pt, 6.0);
                    }
                }
            }

            // Task 9.2 & 9.3: Quick Diagramming (Ketuk titik atau seret konektor)
            if let ResizeCorner::QuickDiagramDot(site) = corner {
                let stop_page_p = pointer_pos
                    .map(screen_to_page)
                    .unwrap_or(Pos2::new(0.0, 0.0));
                let drag_dist = state
                    .drag_start_pos
                    .map_or(0.0, |p| (stop_page_p - p).length());
                if let Some(ConnectorEnd::Attached {
                    shape_id: source_id,
                    ..
                }) = state.draft_connector_start.take()
                {
                    if drag_dist <= 6.0 {
                        // Task 9.2: Ketuk / Klik titik -> buat bentuk baru terhubung searah titik
                        if let Some(source_obj) =
                            page.objects.iter().find(|o| o.id() == source_id).cloned()
                            && let DrawObject::Shape {
                                x: sx,
                                y: sy,
                                width: sw,
                                height: sh,
                                shape_type,
                                color,
                                stroke_width,
                                fill,
                                corner_radius,
                                ..
                            } = source_obj
                        {
                            state.push_undo(&page.id, &page.objects);
                            let (nx, ny, nw, nh, opp_site) = next_shape_rect(
                                (sx, sy, sw, sh),
                                site,
                                60.0,
                                &page.objects,
                                (page.width, page.height),
                            );
                            let new_shape_id = Uuid::new_v4().to_string();
                            let new_shape = DrawObject::Shape {
                                id: new_shape_id.clone(),
                                x: nx,
                                y: ny,
                                width: nw,
                                height: nh,
                                color: color.clone(),
                                stroke_width,
                                fill,
                                shape_type,
                                rotation: 0.0,
                                corner_radius,
                                curve_points: Vec::new(),
                                label: Some(crate::model::ShapeLabel {
                                    text: String::new(),
                                    font_size: 16.0,
                                    color,
                                    alignment: crate::model::TextAlign::Center,
                                    vertical_alignment: crate::model::VerticalAlign::Middle,
                                }),
                                locked: false,
                            };
                            let new_conn = DrawObject::Connector {
                                id: Uuid::new_v4().to_string(),
                                start: ConnectorEnd::Attached {
                                    shape_id: source_id,
                                    site,
                                },
                                end: ConnectorEnd::Attached {
                                    shape_id: new_shape_id.clone(),
                                    site: opp_site,
                                },
                                route: ConnectorRoute::Orthogonal,
                                control_points: Vec::new(),
                                style: ConnectorStyle {
                                    stroke_color: ColorData {
                                        r: 40,
                                        g: 45,
                                        b: 55,
                                        a: 255,
                                    },
                                    stroke_width: 2.0,
                                    start_marker: MarkerKind::None,
                                    end_marker: MarkerKind::Arrow,
                                },
                                locked: false,
                            };
                            page.objects.push(new_shape);
                            page.objects.push(new_conn);
                            state.selected_ids = vec![new_shape_id.clone()];
                            state.editing_text_id = Some(new_shape_id);
                            state.focus_text_edit = true;
                        }
                    } else if let Some((target_id, target_site, _)) =
                        state.connector_snap_candidate.take()
                    {
                        // Task 9.3: Lepas pada bentuk lain -> sambungkan konektor
                        state.push_undo(&page.id, &page.objects);
                        let new_conn = DrawObject::Connector {
                            id: Uuid::new_v4().to_string(),
                            start: ConnectorEnd::Attached {
                                shape_id: source_id,
                                site,
                            },
                            end: ConnectorEnd::Attached {
                                shape_id: target_id,
                                site: target_site,
                            },
                            route: ConnectorRoute::Orthogonal,
                            control_points: Vec::new(),
                            style: ConnectorStyle {
                                stroke_color: ColorData {
                                    r: 40,
                                    g: 45,
                                    b: 55,
                                    a: 255,
                                },
                                stroke_width: 2.0,
                                start_marker: MarkerKind::None,
                                end_marker: MarkerKind::Arrow,
                            },
                            locked: false,
                        };
                        page.objects.push(new_conn);
                    } else {
                        // Task 9.3: Lepas di ruang kosong -> buat bentuk baru di sana
                        if let Some(source_obj) =
                            page.objects.iter().find(|o| o.id() == source_id).cloned()
                            && let DrawObject::Shape {
                                width: sw,
                                height: sh,
                                shape_type,
                                color,
                                stroke_width,
                                fill,
                                corner_radius,
                                ..
                            } = source_obj
                        {
                            state.push_undo(&page.id, &page.objects);
                            let margin = 16.0_f32;
                            let nw = sw.abs();
                            let nh = sh.abs();
                            let nx = (stop_page_p.x - nw * 0.5)
                                .clamp(margin, (page.width - nw - margin).max(margin));
                            let ny = (stop_page_p.y - nh * 0.5)
                                .clamp(margin, (page.height - nh - margin).max(margin));

                            let new_shape_id = Uuid::new_v4().to_string();
                            let new_shape = DrawObject::Shape {
                                id: new_shape_id.clone(),
                                x: nx,
                                y: ny,
                                width: nw,
                                height: nh,
                                color: color.clone(),
                                stroke_width,
                                fill,
                                shape_type,
                                rotation: 0.0,
                                corner_radius,
                                curve_points: Vec::new(),
                                label: Some(crate::model::ShapeLabel {
                                    text: String::new(),
                                    font_size: 16.0,
                                    color,
                                    alignment: crate::model::TextAlign::Center,
                                    vertical_alignment: crate::model::VerticalAlign::Middle,
                                }),
                                locked: false,
                            };
                            let new_conn = DrawObject::Connector {
                                id: Uuid::new_v4().to_string(),
                                start: ConnectorEnd::Attached {
                                    shape_id: source_id,
                                    site,
                                },
                                end: ConnectorEnd::Attached {
                                    shape_id: new_shape_id.clone(),
                                    site: ConnectionSite::Auto,
                                },
                                route: ConnectorRoute::Orthogonal,
                                control_points: Vec::new(),
                                style: ConnectorStyle {
                                    stroke_color: ColorData {
                                        r: 40,
                                        g: 45,
                                        b: 55,
                                        a: 255,
                                    },
                                    stroke_width: 2.0,
                                    start_marker: MarkerKind::None,
                                    end_marker: MarkerKind::Arrow,
                                },
                                locked: false,
                            };
                            page.objects.push(new_shape);
                            page.objects.push(new_conn);
                            state.selected_ids = vec![new_shape_id.clone()];
                            state.editing_text_id = Some(new_shape_id);
                            state.focus_text_edit = true;
                        }
                    }
                }
                state.draft_connector_start = None;
                state.draft_connector_current = None;
                state.connector_snap_candidate = None;
            }
            state.spatial_grid.rebuild(&page.objects);
        } else if state.active_stroke_page_id.as_deref() == Some(&page.id) {
            state.active_stroke_page_id = None;
            state.last_stroke_move_time = None;
            let recognized_shape = state.recognized_draft_shape.take();
            if let Some(mut pts) = state.draft_stroke.take() {
                // Ensure exact final point
                if let Some(pos) = pointer_pos {
                    pts.push(StrokePoint::from(screen_to_page(pos)));
                }

                if pts.len() > 1 {
                    state.push_undo(&page.id, &page.objects);
                    let is_highlighter = state.current_tool == ActiveTool::Highlighter;
                    let stroke_width = if is_highlighter {
                        state.stroke_width * 6.0
                    } else if state.current_tool == ActiveTool::Pencil {
                        (state.stroke_width - 0.5).max(1.0)
                    } else {
                        state.stroke_width
                    };
                    let color = if is_highlighter {
                        Color32::from_rgba_unmultiplied(
                            state.stroke_color.r(),
                            state.stroke_color.g(),
                            state.stroke_color.b(),
                            110,
                        )
                    } else {
                        state.stroke_color
                    };
                    // Fase 8.1 & G4: Shape Recognition & Determinisme Undo
                    if let Some(recognized) = recognized_shape {
                        let new_id = Uuid::new_v4().to_string();
                        let is_line = recognized.kind == ShapeKind::Line;
                        let corner_radius = if recognized.kind == ShapeKind::Rectangle {
                            (recognized.width.abs().min(recognized.height.abs()) * 0.12)
                                .clamp(4.0, 14.0)
                        } else {
                            0.0
                        };

                        // Fase G4.3: Simpan stroke asli ke snapshot pertama, lalu ganti dengan shape terkoreksi
                        // Undo #1 -> kembali ke goresan asli pengguna
                        // Undo #2 -> hapus goresan (kertas kosong)
                        state.push_undo(&page.id, &page.objects);
                        let orig_stroke = DrawObject::Stroke {
                            id: Uuid::new_v4().to_string(),
                            points: pts.clone(),
                            color: color.into(),
                            width: stroke_width,
                            is_highlighter,
                        };
                        page.objects.push(orig_stroke);

                        state.push_undo(&page.id, &page.objects);
                        page.objects.pop();

                        page.objects.push(DrawObject::Shape {
                            id: new_id.clone(),
                            shape_type: recognized.kind,
                            x: recognized.x,
                            y: recognized.y,
                            width: recognized.width,
                            height: recognized.height,
                            color: color.into(),
                            stroke_width,
                            fill: None,
                            rotation: 0.0,
                            corner_radius,
                            label: None,
                            locked: false,
                            curve_points: Vec::new(),
                        });
                        if is_line {
                            state.selected_ids = vec![new_id];
                        }
                        state.spatial_grid.rebuild(&page.objects);
                    } else {
                        page.objects.push(DrawObject::Stroke {
                            id: Uuid::new_v4().to_string(),
                            points: pts,
                            color: color.into(),
                            width: stroke_width,
                            is_highlighter,
                        });
                        state.spatial_grid.rebuild(&page.objects);
                        // Fase 8.6: Perbarui indeks OCR tulisan tangan otomatis
                        page.ocr_text = recognize_handwriting_from_strokes(&page.objects);
                    }
                }
            } else if let (Some(start), Some(curr)) = (
                state.draft_shape_start.take(),
                state.draft_shape_current.take(),
            ) && (curr - start).length() > 4.0
            {
                state.push_undo(&page.id, &page.objects);
                if let ActiveTool::Shape(kind) = state.current_tool {
                    let new_id = Uuid::new_v4().to_string();
                    let w = curr.x - start.x;
                    let h = curr.y - start.y;
                    let corner_radius = if kind == ShapeKind::Rectangle {
                        (w.abs().min(h.abs()) * 0.12).clamp(4.0, 14.0)
                    } else {
                        0.0
                    };
                    page.objects.push(DrawObject::Shape {
                        id: new_id.clone(),
                        shape_type: kind,
                        x: start.x,
                        y: start.y,
                        width: w,
                        height: h,
                        color: state.stroke_color.into(),
                        stroke_width: state.stroke_width,
                        fill: None,
                        rotation: 0.0,
                        corner_radius,
                        label: None,
                        locked: false,
                        curve_points: Vec::new(),
                    });
                    if kind == ShapeKind::Line {
                        state.selected_ids = vec![new_id];
                    }
                    state.spatial_grid.rebuild(&page.objects);
                }
            } else if let Some(start_end) = state.draft_connector_start.take() {
                let stop_page_p = pointer_pos
                    .map(screen_to_page)
                    .unwrap_or(Pos2::new(0.0, 0.0));
                let curr_pt = state
                    .draft_connector_current
                    .take()
                    .unwrap_or_else(|| Point2D::new(stop_page_p.x, stop_page_p.y));
                let end_end =
                    if let Some((snap_id, snap_site, _)) = state.connector_snap_candidate.take() {
                        ConnectorEnd::Attached {
                            shape_id: snap_id,
                            site: snap_site,
                        }
                    } else {
                        ConnectorEnd::Free {
                            x: curr_pt.x,
                            y: curr_pt.y,
                        }
                    };

                let start_pos = match &start_end {
                    ConnectorEnd::Attached { shape_id, site } => {
                        if let Some(s) = page.objects.iter().find(|o| o.id() == shape_id) {
                            shape_connection_point(s, *site, None).0
                        } else {
                            Point2D::new(stop_page_p.x, stop_page_p.y)
                        }
                    }
                    ConnectorEnd::Free { x, y } => Point2D::new(*x, *y),
                };

                let end_pos = match &end_end {
                    ConnectorEnd::Attached { shape_id, site } => {
                        if let Some(s) = page.objects.iter().find(|o| o.id() == shape_id) {
                            shape_connection_point(s, *site, Some(start_pos)).0
                        } else {
                            curr_pt
                        }
                    }
                    ConnectorEnd::Free { x, y } => Point2D::new(*x, *y),
                };

                if start_pos.dist(end_pos) > 6.0 {
                    state.push_undo(&page.id, &page.objects);
                    let new_id = Uuid::new_v4().to_string();
                    let new_conn = DrawObject::Connector {
                        id: new_id.clone(),
                        start: start_end,
                        end: end_end,
                        route: ConnectorRoute::Orthogonal,
                        control_points: Vec::new(),
                        style: ConnectorStyle {
                            stroke_color: state.stroke_color.into(),
                            stroke_width: state.stroke_width.clamp(1.5, 6.0),
                            start_marker: MarkerKind::None,
                            end_marker: MarkerKind::Arrow,
                        },
                        locked: false,
                    };
                    page.objects.push(new_conn);
                    state.selected_ids = vec![new_id];
                    state.spatial_grid.rebuild(&page.objects);
                }
            } else if !state.lasso_points.is_empty() {
                if state.lasso_points.len() > 2 {
                    let mut selected = Vec::new();
                    for obj in &page.objects {
                        if object_in_lasso(obj, &state.lasso_points) {
                            selected.push(obj.id().to_string());
                        }
                    }
                    state.selected_ids = selected;
                }
                state.lasso_points.clear();
            }
        }
    }

    // 4. Render Active Draft Stroke / Shape with live smooth tessellation
    if state.active_stroke_page_id.as_deref() == Some(&page.id) {
        if let Some(recognized) = &state.recognized_draft_shape {
            let stroke = Stroke::new(state.stroke_width * state.zoom, state.stroke_color);
            let corner_radius = if recognized.kind == ShapeKind::Rectangle {
                (recognized.width.abs().min(recognized.height.abs()) * 0.12).clamp(4.0, 14.0)
            } else {
                0.0
            };
            draw_shape_primitive(
                &painter,
                recognized.kind,
                recognized.x,
                recognized.y,
                recognized.width,
                recognized.height,
                0.0,
                corner_radius,
                &[],
                Color32::TRANSPARENT,
                stroke,
                page_to_screen,
            );

            // Fase G4.1: Visual feedback snap flash halus saat baru saja snap (< 220 ms)
            if let Some(flash_t) = state.snap_flash_time {
                let flash_elapsed = now.saturating_duration_since(flash_t).as_millis();
                if flash_elapsed < 220 {
                    let flash_alpha = ((220 - flash_elapsed) as f32 / 220.0 * 180.0) as u8;
                    let flash_col = Color32::from_rgba_unmultiplied(0, 180, 255, flash_alpha);
                    let flash_stroke =
                        Stroke::new((state.stroke_width + 2.5) * state.zoom, flash_col);
                    draw_shape_primitive(
                        &painter,
                        recognized.kind,
                        recognized.x,
                        recognized.y,
                        recognized.width,
                        recognized.height,
                        0.0,
                        corner_radius,
                        &[],
                        Color32::TRANSPARENT,
                        flash_stroke,
                        page_to_screen,
                    );
                    ui.ctx().request_repaint();
                }
            }

            if recognized.kind == ShapeKind::Line {
                let s_start = page_to_screen(Pos2::new(recognized.x, recognized.y));
                let s_end = page_to_screen(Pos2::new(
                    recognized.x + recognized.width,
                    recognized.y + recognized.height,
                ));
                let handle_stroke = Stroke::new(2.0_f32, Color32::from_rgb(0, 114, 206));
                painter.circle(s_start, 6.5, Color32::WHITE, handle_stroke);
                painter.circle_filled(s_start, 3.0, Color32::from_rgb(0, 114, 206));
                painter.circle(s_end, 6.0, Color32::WHITE, handle_stroke);
            }
        } else if let Some(pts) = &state.draft_stroke {
            let is_highlighter = state.current_tool == ActiveTool::Highlighter;
            let stroke_width = if is_highlighter {
                state.stroke_width * 6.0 * state.zoom
            } else {
                state.stroke_width * state.zoom
            };
            let color = if is_highlighter {
                Color32::from_rgba_unmultiplied(
                    state.stroke_color.r(),
                    state.stroke_color.g(),
                    state.stroke_color.b(),
                    110,
                )
            } else {
                state.stroke_color
            };
            let smoothed = smooth_stroke_points(pts, state.smoothing_steps);
            let mesh = tessellate_stroke_ex(
                &smoothed,
                stroke_width,
                color,
                is_highlighter,
                page_to_screen,
            );
            painter.add(mesh);
        }

        if let (Some(start), Some(curr)) = (state.draft_shape_start, state.draft_shape_current) {
            let stroke = Stroke::new(state.stroke_width * state.zoom, state.stroke_color);
            if let ActiveTool::Shape(kind) = state.current_tool {
                let w = curr.x - start.x;
                let h = curr.y - start.y;
                let corner_radius = if kind == ShapeKind::Rectangle {
                    (w.abs().min(h.abs()) * 0.12).clamp(4.0, 14.0)
                } else {
                    0.0
                };
                draw_shape_primitive(
                    &painter,
                    kind,
                    start.x,
                    start.y,
                    w,
                    h,
                    0.0,
                    corner_radius,
                    &[],
                    Color32::TRANSPARENT,
                    stroke,
                    page_to_screen,
                );
                if kind == ShapeKind::Line {
                    let s_start = page_to_screen(start);
                    let s_end = page_to_screen(curr);
                    let handle_stroke = Stroke::new(2.0_f32, Color32::from_rgb(0, 114, 206));
                    painter.circle(s_start, 6.5, Color32::WHITE, handle_stroke);
                    painter.circle_filled(s_start, 3.0, Color32::from_rgb(0, 114, 206));
                    painter.circle(s_end, 6.0, Color32::WHITE, handle_stroke);
                }
            }
        }

        // Preview Draft Connector saat ditarik (Task 7.1)
        if let (Some(start_end), Some(curr_pt)) =
            (&state.draft_connector_start, &state.draft_connector_current)
        {
            let start_pos = match start_end {
                ConnectorEnd::Attached { shape_id, site } => {
                    if let Some(s) = page.objects.iter().find(|o| o.id() == shape_id) {
                        shape_connection_point(s, *site, None).0
                    } else {
                        *curr_pt
                    }
                }
                ConnectorEnd::Free { x, y } => Point2D::new(*x, *y),
            };
            let path = crate::geometry::route_orthogonal(
                start_pos,
                crate::geometry::Direction::Right,
                *curr_pt,
                crate::geometry::Direction::Left,
                16.0,
            );
            if path.len() >= 2 {
                let stroke = Stroke::new(
                    (state.stroke_width * state.zoom).max(1.5),
                    Color32::from_rgb(0, 114, 206),
                );
                for i in 0..path.len() - 1 {
                    let p1 = page_to_screen(path[i].to_pos2());
                    let p2 = page_to_screen(path[i + 1].to_pos2());
                    painter.line_segment([p1, p2], stroke);
                }
                let last = path[path.len() - 1];
                let prev = path[path.len() - 2];
                let dir = last - prev;
                let arrow = crate::geometry::arrowhead_geometry(last, dir, 12.0, 8.0);
                let p_tip = page_to_screen(arrow[0].to_pos2());
                let p_l = page_to_screen(arrow[1].to_pos2());
                let p_r = page_to_screen(arrow[2].to_pos2());
                painter.add(egui::Shape::convex_polygon(
                    vec![p_tip, p_l, p_r],
                    Color32::from_rgb(0, 114, 206),
                    Stroke::NONE,
                ));
            }
        }
    }

    // Visual feedback titik sambung pada bentuk saat alat Connector aktif atau saat re-anchor (Task 7.1, 7.3)
    if state.current_tool == ActiveTool::Connector
        || state.draft_connector_start.is_some()
        || matches!(
            state.resize_corner,
            Some(ResizeCorner::ConnectorStart | ResizeCorner::ConnectorEnd)
        )
    {
        for obj in &page.objects {
            if let DrawObject::Shape { locked, .. } = obj
                && !*locked
            {
                for &site in &[
                    ConnectionSite::Top,
                    ConnectionSite::Right,
                    ConnectionSite::Bottom,
                    ConnectionSite::Left,
                ] {
                    let (anchor, _) = shape_connection_point(obj, site, None);
                    let s_pt = page_to_screen(anchor.to_pos2());
                    let is_candidate = state
                        .connector_snap_candidate
                        .as_ref()
                        .is_some_and(|(_, _, pt)| pt.dist_sq(anchor) < 1.0);

                    if is_candidate {
                        painter.circle_filled(s_pt, 7.0, Color32::from_rgb(40, 167, 69));
                        painter.circle_stroke(s_pt, 9.0, Stroke::new(2.0_f32, Color32::WHITE));
                    } else {
                        painter.circle_filled(
                            s_pt,
                            4.5,
                            Color32::from_rgba_unmultiplied(0, 114, 206, 160),
                        );
                        painter.circle_stroke(s_pt, 4.5, Stroke::new(1.2_f32, Color32::WHITE));
                    }
                }
            }
        }
    }

    // 5. Render Active Lasso Loop (hanya pada halaman tempat Lasso sedang ditarik)
    if state.active_stroke_page_id.as_deref() == Some(&page.id) && state.lasso_points.len() > 1 {
        let stroke = Stroke::new(1.5_f32, Color32::from_rgb(0, 114, 206));
        for i in 1..state.lasso_points.len() {
            let p1 = page_to_screen(state.lasso_points[i - 1]);
            let p2 = page_to_screen(state.lasso_points[i]);
            painter.line_segment([p1, p2], stroke);
        }
    }

    // 6. Render Selection Bounding Box & Handles (recompute after drag mutations)
    let (
        updated_selection_rect,
        updated_single_line,
        updated_single_shape,
        updated_single_connector,
        any_locked,
        all_locked,
        has_shapes_selected,
        updated_single_shape_quick_dots,
    ) = {
        let selected: Vec<&DrawObject> = page
            .objects
            .iter()
            .filter(|o| state.selected_ids.contains(&o.id().to_string()))
            .collect();
        let rect = combined_bounds(&selected);
        let single_line = if selected.len() == 1 {
            if let DrawObject::Shape {
                shape_type: ShapeKind::Line,
                x,
                y,
                width,
                height,
                ..
            } = selected[0]
            {
                Some((Pos2::new(*x, *y), Pos2::new(*x + *width, *y + *height)))
            } else {
                None
            }
        } else {
            None
        };
        let single_shape = if selected.len() == 1 {
            if let DrawObject::Shape {
                x,
                y,
                width,
                height,
                rotation,
                ..
            } = selected[0]
            {
                Some((*x, *y, *width, *height, *rotation))
            } else {
                None
            }
        } else {
            None
        };
        let single_connector = if selected.len() == 1 {
            if let DrawObject::Connector { id, .. } = selected[0] {
                Some((id.clone(), connector_polyline(selected[0], &page.objects)))
            } else {
                None
            }
        } else {
            None
        };
        let any_l = selected.iter().any(|o| o.is_locked());
        let all_l = !selected.is_empty() && selected.iter().all(|o| o.is_locked());
        let has_shapes = selected
            .iter()
            .any(|o| matches!(o, DrawObject::Shape { .. }));
        let quick_dots = if selected.len() == 1
            && let DrawObject::Shape { id, locked, .. } = selected[0]
            && !*locked
        {
            let shape = selected[0];
            let sites = [
                ConnectionSite::Top,
                ConnectionSite::Right,
                ConnectionSite::Bottom,
                ConnectionSite::Left,
            ];
            let dots: Vec<(ConnectionSite, Pos2, Point2D)> = sites
                .iter()
                .map(|&site| {
                    let (pt, dir) = shape_connection_point(shape, site, None);
                    let dot_pt = pt + dir.to_vector() * 12.0;
                    (site, dot_pt.to_pos2(), pt)
                })
                .collect();
            Some((id.clone(), dots))
        } else {
            None
        };
        (
            rect,
            single_line,
            single_shape,
            single_connector,
            any_l,
            all_l,
            has_shapes,
            quick_dots,
        )
    };

    if let Some((conn_id, Some(poly))) = &updated_single_connector {
        if poly.len() >= 2 {
            let is_conn_locked = page
                .objects
                .iter()
                .any(|o| o.id() == conn_id && o.is_locked());
            let handle_stroke = if is_conn_locked {
                Stroke::new(1.8_f32, Color32::from_rgb(140, 150, 165))
            } else {
                Stroke::new(2.0_f32, Color32::from_rgb(0, 114, 206))
            };

            // Garis highlight seleksi
            let highlight_col = if is_conn_locked {
                Color32::from_rgba_unmultiplied(140, 150, 165, 50)
            } else {
                Color32::from_rgba_unmultiplied(0, 114, 206, 50)
            };
            for i in 0..poly.len() - 1 {
                let p1 = page_to_screen(poly[i].to_pos2());
                let p2 = page_to_screen(poly[i + 1].to_pos2());
                painter.line_segment([p1, p2], Stroke::new(6.0_f32, highlight_col));
            }

            if is_conn_locked {
                // Task 10.1: Visual lock badge 🔒 di titik tengah connector
                let mid_idx = poly.len() / 2;
                let lock_pos = page_to_screen(poly[mid_idx].to_pos2());
                painter.circle(
                    lock_pos,
                    11.0,
                    Color32::WHITE,
                    Stroke::new(1.8_f32, Color32::from_rgb(140, 150, 165)),
                );
                painter.text(
                    lock_pos,
                    egui::Align2::CENTER_CENTER,
                    "🔒",
                    egui::FontId::proportional(12.0),
                    Color32::from_rgb(90, 100, 115),
                );
                if let Some(pos) = pointer_pos
                    && (pos - lock_pos).length() <= 14.0
                {
                    egui::show_tooltip(
                        ui.ctx(),
                        ui.layer_id(),
                        egui::Id::new("locked_conn_badge_tooltip"),
                        |ui| {
                            ui.label("🔒 Terkunci: Connector tidak dapat digeser atau diubah titik jalurnya. Tekan Ctrl+L atau gunakan toolbar untuk membuka kunci.");
                        },
                    );
                }
            } else {
                // Start Handle di poly[0]
                let s_start = page_to_screen(poly[0].to_pos2());
                painter.circle(s_start, 6.5, Color32::WHITE, handle_stroke);
                painter.circle_filled(s_start, 3.0, Color32::from_rgb(0, 114, 206));

                // End Handle di poly[last]
                let s_end = page_to_screen(poly[poly.len() - 1].to_pos2());
                painter.circle(s_end, 6.5, Color32::WHITE, handle_stroke);
                painter.circle_filled(s_end, 3.0, Color32::from_rgb(0, 114, 206));

                // Waypoint handles di titik tikungan (Task 8.1, 8.3)
                for pt in poly.iter().take(poly.len() - 1).skip(1) {
                    let s_cp = page_to_screen(pt.to_pos2());
                    painter.circle(s_cp, 5.0, Color32::WHITE, handle_stroke);
                    painter.circle_filled(s_cp, 2.5, Color32::from_rgb(0, 114, 206));
                }

                // Midpoint ghost handles untuk menambah tikungan baru (Task 8.1, 8.2)
                let mid_stroke =
                    Stroke::new(1.2_f32, Color32::from_rgba_unmultiplied(0, 114, 206, 180));
                for i in 0..poly.len() - 1 {
                    let mid = (poly[i] + poly[i + 1]) * 0.5;
                    let s_mid = page_to_screen(mid.to_pos2());
                    painter.circle(s_mid, 4.0, Color32::WHITE, mid_stroke);
                    painter.circle_filled(s_mid, 1.5, Color32::from_rgb(0, 114, 206));
                }

                // Task 8.1 & 8.3: Tooltip & kursor pada handle connector saat hover
                if let Some(pos) = pointer_pos
                    && state.resize_corner.is_none()
                    && !state.is_dragging_selection
                {
                    let page_p = screen_to_page(pos);
                    let handle_dist = 14.0 / state.zoom;
                    if let Some((_seg_idx, _mid_pt)) = hit_test_connector_midpoint(
                        Point2D::new(page_p.x, page_p.y),
                        poly,
                        handle_dist,
                    ) {
                        ui.ctx().set_cursor_icon(CursorIcon::Crosshair);
                        egui::show_tooltip(
                            ui.ctx(),
                            ui.layer_id(),
                            egui::Id::new("connector_mid_tooltip"),
                            |ui| {
                                ui.label(
                                    "Tambah Tikungan: Seret untuk menambah titik kontrol baru.",
                                );
                            },
                        );
                    } else if let Some(target) = hit_test_connector_details(
                        Point2D::new(page_p.x, page_p.y),
                        poly,
                        handle_dist,
                    ) {
                        match target {
                            crate::geometry::ConnectorHitTarget::StartHandle
                            | crate::geometry::ConnectorHitTarget::EndHandle => {
                                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                                egui::show_tooltip(
                                    ui.ctx(),
                                    ui.layer_id(),
                                    egui::Id::new("connector_end_tooltip"),
                                    |ui| {
                                        ui.label("Ujung Connector: Seret untuk menyambung atau melepas titik sambung.");
                                    },
                                );
                            }
                            crate::geometry::ConnectorHitTarget::ControlPoint(_) => {
                                ui.ctx().set_cursor_icon(CursorIcon::Grab);
                                egui::show_tooltip(
                                    ui.ctx(),
                                    ui.layer_id(),
                                    egui::Id::new("connector_cp_tooltip"),
                                    |ui| {
                                        ui.label("Titik Kontrol: Seret untuk memindahkan jalur. Klik ganda untuk menghapus.");
                                    },
                                );
                            }
                            crate::geometry::ConnectorHitTarget::LineSegment(_) => {
                                ui.ctx().set_cursor_icon(CursorIcon::Crosshair);
                            }
                        }
                    }
                }
            }

            // Floating Toolbar untuk Connector Terpilih (Task 7.6)
            let s_mid = page_to_screen(poly[poly.len() / 2].to_pos2());
            let panel_pos = Pos2::new(
                s_mid.x.clamp(rect.left() + 250.0, rect.right() - 250.0),
                (s_mid.y - 28.0).max(rect.top() + 8.0),
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
                                    if o.id() == conn_id && let DrawObject::Connector { style, .. } = o {
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
                                    let (c_rect, resp) = ui.allocate_exact_size(
                                        Vec2::splat(18.0),
                                        egui::Sense::click(),
                                    );
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
                                let cur_w = page.objects.iter().find_map(|o| {
                                    if o.id() == conn_id && let DrawObject::Connector { style, .. } = o {
                                        Some(style.stroke_width)
                                    } else {
                                        None
                                    }
                                }).unwrap_or(2.5);

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
                                let lock_text = if is_locked { "🔓 Buka" } else { "🔒 Kunci" };
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
                                        .on_hover_text("Hapus semua tikungan manual dan kembali ke rute otomatis")
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
    } else if let Some((line_start, line_end)) = updated_single_line {
        let s_start = page_to_screen(line_start);
        let s_end = page_to_screen(line_end);
        let handle_stroke = Stroke::new(2.0_f32, Color32::from_rgb(0, 114, 206));

        // Garis bantu seleksi antara titik awal (pusat) dan titik akhir
        painter.line_segment(
            [s_start, s_end],
            Stroke::new(1.5_f32, Color32::from_rgba_unmultiplied(0, 114, 206, 160)),
        );

        // Titik awal garis sebagai Pusat Rotasi (Pivot Anchor: lingkaran ganda)
        painter.circle(s_start, 7.0, Color32::WHITE, handle_stroke);
        painter.circle_filled(s_start, 3.5, Color32::from_rgb(0, 114, 206));

        // Titik akhir garis yang dapat dirotasi
        painter.circle(s_end, 6.5, Color32::WHITE, handle_stroke);

        // Tangkai & tombol rotasi (⟳) di ujung titik akhir garis
        let dir = s_end - s_start;
        let dir_norm = if dir.length() > 1e-3 {
            dir / dir.length()
        } else {
            Vec2::new(1.0, 0.0)
        };
        let rot_pos = s_end + dir_norm * 22.0;
        painter.line_segment([s_end + dir_norm * 6.5, rot_pos], handle_stroke);
        painter.circle(rot_pos, 7.5, Color32::WHITE, handle_stroke);
        painter.text(
            rot_pos,
            egui::Align2::CENTER_CENTER,
            "⟳",
            egui::FontId::proportional(11.0),
            Color32::from_rgb(0, 114, 206),
        );
    } else if let Some(b) = updated_selection_rect {
        let (bx, by, bw, bh, brot) = if let Some(s) = updated_single_shape {
            s
        } else {
            (b.min.x, b.min.y, b.width(), b.height(), 0.0)
        };

        let transform_handles = compute_transform_handles(bx, by, bw, bh, brot, 20.0 / state.zoom);

        let mut handle_screen_map = std::collections::HashMap::new();
        for h in &transform_handles {
            handle_screen_map.insert(h.kind, page_to_screen(h.pos.to_pos2()));
        }

        let s_nw = handle_screen_map[&TransformHandleKind::NorthWest];
        let s_ne = handle_screen_map[&TransformHandleKind::NorthEast];
        let s_se = handle_screen_map[&TransformHandleKind::SouthEast];
        let s_sw = handle_screen_map[&TransformHandleKind::SouthWest];
        let s_n = handle_screen_map[&TransformHandleKind::North];
        let s_e = handle_screen_map[&TransformHandleKind::East];
        let s_s = handle_screen_map[&TransformHandleKind::South];
        let s_w = handle_screen_map[&TransformHandleKind::West];
        let s_rot = handle_screen_map[&TransformHandleKind::Rotation];

        // Garis batas seleksi berotasi
        let box_stroke = if all_locked {
            Stroke::new(1.5_f32, Color32::from_rgb(140, 150, 165))
        } else {
            Stroke::new(1.5_f32, Color32::from_rgb(0, 114, 206))
        };
        painter.line_segment([s_nw, s_ne], box_stroke);
        painter.line_segment([s_ne, s_se], box_stroke);
        painter.line_segment([s_se, s_sw], box_stroke);
        painter.line_segment([s_sw, s_nw], box_stroke);

        if all_locked {
            // Task 10.1: Visual lock badge 🔒 di sudut kanan-atas bounding box
            let lock_badge_pos = s_ne + Vec2::new(10.0, -10.0);
            painter.circle(
                lock_badge_pos,
                11.0,
                Color32::WHITE,
                Stroke::new(1.8_f32, Color32::from_rgb(140, 150, 165)),
            );
            painter.text(
                lock_badge_pos,
                egui::Align2::CENTER_CENTER,
                "🔒",
                egui::FontId::proportional(12.0),
                Color32::from_rgb(90, 100, 115),
            );
            if let Some(pos) = pointer_pos
                && (pos - lock_badge_pos).length() <= 14.0
            {
                egui::show_tooltip(
                    ui.ctx(),
                    ui.layer_id(),
                    egui::Id::new("locked_shape_badge_tooltip"),
                    |ui| {
                        ui.label("🔒 Terkunci: Bentuk tidak dapat digeser atau diubah ukurannya. Tekan Ctrl+L atau gunakan toolbar untuk membuka kunci.");
                    },
                );
            }
        } else {
            // 4 Corner handles: Lingkaran putih di tiap sudut untuk scale proporsional (A1)
            let corner_stroke = Stroke::new(2.0_f32, Color32::from_rgb(0, 114, 206));
            for pos in [s_nw, s_ne, s_se, s_sw] {
                painter.circle(pos, 6.0, Color32::WHITE, corner_stroke);
            }

            // 4 Edge handles: Persegi biru di tengah sisi untuk resize satu sumbu (A1)
            for pos in [s_n, s_e, s_s, s_w] {
                let edge_rect = Rect::from_center_size(pos, Vec2::splat(9.0));
                painter.rect_filled(edge_rect, 1.5, Color32::from_rgb(0, 114, 206));
                painter.rect_stroke(
                    edge_rect,
                    1.5,
                    Stroke::new(1.0_f32, Color32::WHITE),
                    egui::StrokeKind::Outside,
                );
            }

            // 1 Rotation handle: Lingkaran putih menonjol di atas dengan ⟳
            painter.line_segment([s_n, s_rot], corner_stroke);
            painter.circle(s_rot, 7.5, Color32::WHITE, corner_stroke);
            painter.text(
                s_rot,
                egui::Align2::CENTER_CENTER,
                "⟳",
                egui::FontId::proportional(11.0),
                Color32::from_rgb(0, 114, 206),
            );
        }

        // Kursor dinamis saat hover di atas handle seleksi
        if let Some(pos) = pointer_pos {
            let page_p = screen_to_page(pos);
            if let Some(corner) = state.resize_corner {
                match corner {
                    ResizeCorner::Rotation => ui.ctx().set_cursor_icon(CursorIcon::Crosshair),
                    ResizeCorner::North | ResizeCorner::South => {
                        ui.ctx().set_cursor_icon(CursorIcon::ResizeVertical)
                    }
                    ResizeCorner::East | ResizeCorner::West => {
                        ui.ctx().set_cursor_icon(CursorIcon::ResizeHorizontal)
                    }
                    ResizeCorner::NorthWest | ResizeCorner::SouthEast => {
                        ui.ctx().set_cursor_icon(CursorIcon::ResizeNwSe)
                    }
                    ResizeCorner::NorthEast | ResizeCorner::SouthWest => {
                        ui.ctx().set_cursor_icon(CursorIcon::ResizeNeSw)
                    }
                    ResizeCorner::LineStart | ResizeCorner::LineEnd => {
                        ui.ctx().set_cursor_icon(CursorIcon::Crosshair)
                    }
                    ResizeCorner::ConnectorStart | ResizeCorner::ConnectorEnd => {
                        ui.ctx().set_cursor_icon(CursorIcon::PointingHand)
                    }
                    ResizeCorner::ConnectorControl(_) => ui.ctx().set_cursor_icon(CursorIcon::Grab),
                    ResizeCorner::QuickDiagramDot(_) => {
                        ui.ctx().set_cursor_icon(CursorIcon::PointingHand)
                    }
                }
            } else if state.is_dragging_selection {
                ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
            } else if matches!(state.current_tool, ActiveTool::Lasso | ActiveTool::Shape(_))
                && !all_locked
            {
                let handle_dist = 14.0 / state.zoom;
                if let Some(h_kind) = hit_test_transform_handles(
                    &transform_handles,
                    Point2D::new(page_p.x, page_p.y),
                    handle_dist,
                ) {
                    match h_kind {
                        TransformHandleKind::Rotation => {
                            ui.ctx().set_cursor_icon(CursorIcon::Crosshair)
                        }
                        TransformHandleKind::North | TransformHandleKind::South => {
                            ui.ctx().set_cursor_icon(CursorIcon::ResizeVertical)
                        }
                        TransformHandleKind::East | TransformHandleKind::West => {
                            ui.ctx().set_cursor_icon(CursorIcon::ResizeHorizontal)
                        }
                        TransformHandleKind::NorthWest | TransformHandleKind::SouthEast => {
                            ui.ctx().set_cursor_icon(CursorIcon::ResizeNwSe)
                        }
                        TransformHandleKind::NorthEast | TransformHandleKind::SouthWest => {
                            ui.ctx().set_cursor_icon(CursorIcon::ResizeNeSw)
                        }
                    }
                } else if b.contains(page_p) {
                    ui.ctx().set_cursor_icon(CursorIcon::Grab);
                }
            }
        }

        // 3.4 & A7: Locked Badge bila ada objek terkunci yang sedang dipilih
        if any_locked {
            let lock_pos = s_ne + Vec2::new(12.0, -12.0);
            painter.circle_filled(lock_pos, 9.0, Color32::from_rgb(220, 53, 69));
            painter.text(
                lock_pos,
                egui::Align2::CENTER_CENTER,
                "🔒",
                egui::FontId::proportional(11.0),
                Color32::WHITE,
            );
        }

        // Task 5.2 & 5.5: Panel Gaya Melayang di dekat seleksi
        if has_shapes_selected {
            let min_screen_y = s_nw.y.min(s_ne.y).min(s_rot.y);
            let center_screen_x = (s_nw.x + s_se.x) * 0.5;
            let panel_pos = Pos2::new(
                center_screen_x.clamp(rect.left() + 250.0, rect.right() - 250.0),
                (min_screen_y - 24.0).max(rect.top() + 8.0),
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
                                                let cr =
                                                    ((*width).min(*height) * 0.12).clamp(4.0, 14.0);
                                                change_shape_kind(obj, ShapeKind::Rectangle, cr);
                                            }
                                        }
                                        state.mesh_cache.clear();
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
                                        state.mesh_cache.clear();
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
                                        state.mesh_cache.clear();
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
                                        state.mesh_cache.clear();
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
                                        state.mesh_cache.clear();
                                        state.spatial_grid.rebuild(&page.objects);
                                        ui.close_menu();
                                    }
                                    if ui.button("▱ Jajar Genjang").clicked() {
                                        state.push_undo(&page.id, &page.objects);
                                        for obj in &mut page.objects {
                                            if state.selected_ids.contains(&obj.id().to_string()) {
                                                change_shape_kind(
                                                    obj,
                                                    ShapeKind::Parallelogram,
                                                    0.0,
                                                );
                                            }
                                        }
                                        state.mesh_cache.clear();
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
                                        state.mesh_cache.clear();
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
                                            let cr =
                                                ((*width).min(*height) * 0.12).clamp(4.0, 14.0);
                                            change_shape_kind(obj, ShapeKind::Rectangle, cr);
                                        }
                                    }
                                    state.mesh_cache.clear();
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
                                    let (c_rect, resp) = ui.allocate_exact_size(
                                        Vec2::splat(16.0),
                                        egui::Sense::click(),
                                    );
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
                                        state.mesh_cache.clear();
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
                                        state.mesh_cache.clear();
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
                                    state.mesh_cache.clear();
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
                                    let (f_rect, resp) = ui.allocate_exact_size(
                                        Vec2::splat(16.0),
                                        egui::Sense::click(),
                                    );
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
                                        state.mesh_cache.clear();
                                    }
                                }

                                ui.separator();

                                // Tombol Tambah/Edit Teks di dalam Bentuk (Task 6.1)
                                if ui
                                    .button("✍ Teks")
                                    .on_hover_text("Tulis atau Sunting Teks di dalam Bentuk")
                                    .clicked()
                                    && let Some(id) = state.selected_ids.first().cloned()
                                    && let Some(obj) =
                                        page.objects.iter_mut().find(|o| o.id() == id)
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

        // Task 9.1 & A8: Visual feedback 4 titik tepi (Quick Diagramming edge dots)
        if let Some((_source_id, dots)) = &updated_single_shape_quick_dots {
            let dot_stroke = Stroke::new(1.8_f32, Color32::from_rgb(0, 114, 206));
            let hover_p = pointer_pos.map(screen_to_page);
            for &(site, dpos, _) in dots {
                let s_dot = page_to_screen(dpos);
                let is_hovered = hover_p.is_some_and(|p| (p - dpos).length() <= 14.0 / state.zoom);

                if is_hovered {
                    painter.circle(
                        s_dot,
                        10.0,
                        Color32::from_rgba_unmultiplied(0, 114, 206, 40),
                        Stroke::NONE,
                    );
                    painter.circle(
                        s_dot,
                        8.5,
                        Color32::from_rgb(230, 242, 251),
                        Stroke::new(2.2_f32, Color32::from_rgb(0, 114, 206)),
                    );
                    painter.text(
                        s_dot,
                        egui::Align2::CENTER_CENTER,
                        "+",
                        egui::FontId::proportional(12.0),
                        Color32::from_rgb(0, 114, 206),
                    );
                    egui::show_tooltip(
                        ui.ctx(),
                        ui.layer_id(),
                        egui::Id::new(format!("quick_diagram_tooltip_{:?}", site)),
                        |ui| {
                            ui.label("Quick Diagramming: Ketuk untuk membuat bentuk terhubung. Seret untuk menarik konektor.");
                        },
                    );
                } else {
                    painter.circle(s_dot, 7.0, Color32::WHITE, dot_stroke);
                    painter.circle_filled(s_dot, 2.5, Color32::from_rgb(0, 114, 206));
                }
            }
        }
    }

    // Fase G11: Render Smart Alignment Guides (Garis Panduan Magnetik Berwarna Magenta)
    if !state.active_alignment_guides.is_empty() {
        let guide_stroke = Stroke::new(1.3_f32, Color32::from_rgb(235, 0, 140));
        for guide in &state.active_alignment_guides {
            match guide.orientation {
                crate::geometry::GuideOrientation::Horizontal => {
                    let y = guide.position;
                    let p1 = page_to_screen(Pos2::new(guide.min_span, y));
                    let p2 = page_to_screen(Pos2::new(guide.max_span, y));
                    painter.line_segment([p1, p2], guide_stroke);
                    painter.circle_filled(p1, 2.5, Color32::from_rgb(235, 0, 140));
                    painter.circle_filled(p2, 2.5, Color32::from_rgb(235, 0, 140));
                }
                crate::geometry::GuideOrientation::Vertical => {
                    let x = guide.position;
                    let p1 = page_to_screen(Pos2::new(x, guide.min_span));
                    let p2 = page_to_screen(Pos2::new(x, guide.max_span));
                    painter.line_segment([p1, p2], guide_stroke);
                    painter.circle_filled(p1, 2.5, Color32::from_rgb(235, 0, 140));
                    painter.circle_filled(p2, 2.5, Color32::from_rgb(235, 0, 140));
                }
            }
        }
    }

    // 7. Render Glowing Laser Trail (Multi-stroke Neon Laser Sesuai Gambar Referensi)
    if laser_alpha > 0.0 {
        let mut all_strokes_to_draw: Vec<&[Pos2]> = Vec::new();
        for s in &state.laser_strokes {
            if s.len() > 1 {
                all_strokes_to_draw.push(s.as_slice());
            }
        }
        if state.current_laser_stroke.len() > 1 {
            all_strokes_to_draw.push(state.current_laser_stroke.as_slice());
        }

        let outer_col = Color32::from_rgba_unmultiplied(255, 30, 45, (40.0 * laser_alpha) as u8);
        let halo_col = Color32::from_rgba_unmultiplied(255, 15, 35, (115.0 * laser_alpha) as u8);
        let core_col = Color32::from_rgba_unmultiplied(255, 0, 20, (235.0 * laser_alpha) as u8);
        let beam_col = Color32::from_rgba_unmultiplied(255, 255, 255, (250.0 * laser_alpha) as u8);

        for stroke in &all_strokes_to_draw {
            let smoothed = smooth_laser_points(stroke);
            if smoothed.len() > 1 {
                // Layer 1: Outer Red Neon Aura / Ambient Glow (Pita kontinu bebas artifak kotak/tangga)
                let glow_mesh = tessellate_laser_layer(&smoothed, 20.0, outer_col);
                painter.add(glow_mesh);

                // Layer 2: Mid Neon Halo
                let halo_mesh = tessellate_laser_layer(&smoothed, 11.0, halo_col);
                painter.add(halo_mesh);

                // Layer 3: Solid Vibrant Red Core
                let core_mesh = tessellate_laser_layer(&smoothed, 5.0, core_col);
                painter.add(core_mesh);

                // Layer 4: Hotspot White Laser Beam Center
                let beam_mesh = tessellate_laser_layer(&smoothed, 2.2, beam_col);
                painter.add(beam_mesh);
            }
        }

        // Ujung kepala laser aktif (jika sedang menggambar)
        if state.laser_is_drawing
            && let Some(last_p) = state.current_laser_stroke.last()
        {
            painter.circle_filled(
                *last_p,
                11.0,
                Color32::from_rgba_unmultiplied(255, 40, 50, (90.0 * laser_alpha) as u8),
            );
            painter.circle_filled(
                *last_p,
                5.5,
                Color32::from_rgba_unmultiplied(255, 10, 30, (230.0 * laser_alpha) as u8),
            );
            painter.circle_filled(
                *last_p,
                2.8,
                Color32::from_rgba_unmultiplied(255, 255, 255, (255.0 * laser_alpha) as u8),
            );
        }
    }

    // 8. Fase 8.3: Render Indikator Kotak Target Zoom Window pada Kanvas Utama
    if state.show_zoom_window {
        let z_min = page_to_screen(state.zoom_target_rect.min);
        let z_max = page_to_screen(state.zoom_target_rect.max);
        let z_screen_rect = Rect::from_min_max(z_min, z_max);
        painter.rect_filled(
            z_screen_rect,
            2.0,
            Color32::from_rgba_unmultiplied(0, 114, 206, 18),
        );
        painter.rect_stroke(
            z_screen_rect,
            2.0,
            Stroke::new(2.0_f32, Color32::from_rgb(0, 114, 206)),
            egui::StrokeKind::Outside,
        );
        // Indikator zona auto-advance (25% sisi kanan)
        let adv_left = z_screen_rect.right() - z_screen_rect.width() * 0.25;
        let adv_rect = Rect::from_min_max(
            Pos2::new(adv_left, z_screen_rect.top()),
            z_screen_rect.right_bottom(),
        );
        painter.rect_filled(
            adv_rect,
            0.0,
            Color32::from_rgba_unmultiplied(0, 114, 206, 35),
        );
    }

    // Task 9.6: Quick Diagramming via Keyboard (Tab: Right / Child, Enter: Bottom / Sibling)
    if state.editing_text_id.is_none()
        && let Some((source_id, _)) = &updated_single_shape_quick_dots
    {
        let tab_pressed = ui.input(|i| i.key_pressed(Key::Tab));
        let enter_pressed = ui.input(|i| i.key_pressed(Key::Enter));
        if tab_pressed || enter_pressed {
            let target_site = if tab_pressed {
                ConnectionSite::Right
            } else {
                ConnectionSite::Bottom
            };
            if let Some(source_obj) = page.objects.iter().find(|o| o.id() == source_id).cloned()
                && let DrawObject::Shape {
                    x: sx,
                    y: sy,
                    width: sw,
                    height: sh,
                    shape_type,
                    color,
                    stroke_width,
                    fill,
                    corner_radius,
                    ..
                } = source_obj
            {
                state.push_undo(&page.id, &page.objects);
                let (nx, ny, nw, nh, opp_site) = next_shape_rect(
                    (sx, sy, sw, sh),
                    target_site,
                    60.0,
                    &page.objects,
                    (page.width, page.height),
                );
                let new_shape_id = Uuid::new_v4().to_string();
                let new_shape = DrawObject::Shape {
                    id: new_shape_id.clone(),
                    x: nx,
                    y: ny,
                    width: nw,
                    height: nh,
                    color: color.clone(),
                    stroke_width,
                    fill,
                    shape_type,
                    rotation: 0.0,
                    corner_radius,
                    curve_points: Vec::new(),
                    label: Some(crate::model::ShapeLabel {
                        text: String::new(),
                        font_size: 16.0,
                        color,
                        alignment: crate::model::TextAlign::Center,
                        vertical_alignment: crate::model::VerticalAlign::Middle,
                    }),
                    locked: false,
                };
                let new_conn = DrawObject::Connector {
                    id: Uuid::new_v4().to_string(),
                    start: ConnectorEnd::Attached {
                        shape_id: source_id.clone(),
                        site: target_site,
                    },
                    end: ConnectorEnd::Attached {
                        shape_id: new_shape_id.clone(),
                        site: opp_site,
                    },
                    route: ConnectorRoute::Orthogonal,
                    control_points: Vec::new(),
                    style: ConnectorStyle {
                        stroke_color: ColorData {
                            r: 40,
                            g: 45,
                            b: 55,
                            a: 255,
                        },
                        stroke_width: 2.0,
                        start_marker: MarkerKind::None,
                        end_marker: MarkerKind::Arrow,
                    },
                    locked: false,
                };
                page.objects.push(new_shape);
                page.objects.push(new_conn);
                state.selected_ids = vec![new_shape_id.clone()];
                state.editing_text_id = Some(new_shape_id);
                state.focus_text_edit = true;
                state.spatial_grid.rebuild(&page.objects);
                ui.ctx().request_repaint();
            }
        }
    }

    // Double-click pada control point connector untuk menghapus titik kontrol (Task 8.3)
    if response.double_clicked()
        && let Some(pos) = pointer_pos
        && let Some((conn_id, Some(poly))) = &updated_single_connector
    {
        let page_p = screen_to_page(pos);
        let handle_dist = 14.0 / state.zoom;
        if let Some(crate::geometry::ConnectorHitTarget::ControlPoint(idx)) =
            hit_test_connector_details(Point2D::new(page_p.x, page_p.y), poly, handle_dist)
        {
            state.push_undo(&page.id, &page.objects);
            for obj in &mut page.objects {
                if obj.id() == conn_id
                    && let DrawObject::Connector { control_points, .. } = obj
                {
                    if idx > 0 && idx <= control_points.len() {
                        control_points.remove(idx - 1);
                    } else if !control_points.is_empty() {
                        control_points.pop();
                    }
                }
            }
            ui.ctx().request_repaint();
        }
    }

    // Double-click pada objek Teks, StickyNote, atau Shape untuk menyunting isinya secara langsung (Task 6.1)
    if response.double_clicked()
        && let Some(pos) = pointer_pos
    {
        let page_p = screen_to_page(pos);
        if let Some(obj) = page.objects.iter_mut().rev().find(|obj| {
            matches!(
                obj,
                DrawObject::Text { .. } | DrawObject::StickyNote { .. } | DrawObject::Shape { .. }
            ) && hit_test_object(obj, page_p, 8.0 / state.zoom)
        }) && !obj.is_locked()
        {
            let id = obj.id().to_string();
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
            state.selected_ids = vec![id.clone()];
            state.editing_text_id = Some(id);
            state.focus_text_edit = true;
        }
    }

    // Single-click & Shift+click selection in Lasso mode, Text/StickyNote creation, or precision erase
    if response.clicked()
        && let Some(pos) = pointer_pos
    {
        let page_p = screen_to_page(pos);
        let shift_held = ui.input(|i| i.modifiers.shift);

        if state.current_tool == ActiveTool::Text {
            let hit_id = page
                .objects
                .iter()
                .rev()
                .find(|obj| {
                    matches!(obj, DrawObject::Text { .. } | DrawObject::StickyNote { .. })
                        && hit_test_object(obj, page_p, 8.0 / state.zoom)
                })
                .map(|o| o.id().to_string());
            if let Some(id) = hit_id {
                state.push_undo(&page.id, &page.objects);
                state.selected_ids = vec![id.clone()];
                state.editing_text_id = Some(id);
                state.focus_text_edit = true;
            } else if state.editing_text_id.is_some() {
                // Jika sedang menyunting teks dan klik di luar kotak teks, selesaikan penyuntingan
                state.editing_text_id = None;
                page.objects.retain(
                    |o| !matches!(o, DrawObject::Text { text, .. } if text.trim().is_empty()),
                );
                state.clean_redundant_top_undo(&page.id, &page.objects);
                state.spatial_grid.rebuild(&page.objects);
            } else {
                state.push_undo(&page.id, &page.objects);
                let id = Uuid::new_v4().to_string();
                let new_text = DrawObject::Text {
                    id: id.clone(),
                    x: page_p.x,
                    y: page_p.y,
                    text: String::new(),
                    font_size: (state.stroke_width * 4.0 + 14.0).clamp(12.0, 72.0),
                    color: state.stroke_color.into(),
                    rotation: 0.0,
                };
                page.objects.push(new_text);
                state.spatial_grid.rebuild(&page.objects);
                state.selected_ids = vec![id.clone()];
                state.editing_text_id = Some(id);
                state.focus_text_edit = true;
            }
        } else if state.current_tool == ActiveTool::StickyNote {
            state.push_undo(&page.id, &page.objects);
            let id = Uuid::new_v4().to_string();
            let new_note = DrawObject::StickyNote {
                id: id.clone(),
                x: page_p.x,
                y: page_p.y,
                width: 180.0,
                height: 140.0,
                text: String::new(),
                color: Color32::from_rgb(254, 243, 189).into(),
            };
            page.objects.push(new_note);
            state.spatial_grid.rebuild(&page.objects);
            state.selected_ids = vec![id.clone()];
            state.editing_text_id = Some(id);
            state.focus_text_edit = true;
            state.current_tool = ActiveTool::Lasso;
        } else if state.current_tool == ActiveTool::Eraser {
            state.eraser_undo_pushed = false;
            state.last_whole_erase_pos = None;
            let erase_radius = match state.eraser_mode {
                EraserMode::WholeStroke => (3.8 / state.zoom).clamp(1.8, 4.5),
                EraserMode::Partial => (10.0 / state.zoom).clamp(4.0, 16.0),
            };
            if state.erase_at(page, page_p, erase_radius) {
                ui.ctx().request_repaint();
            }
            state.eraser_undo_pushed = false;
            state.last_whole_erase_pos = None;
        } else if matches!(
            state.current_tool,
            ActiveTool::Lasso | ActiveTool::Shape(_) | ActiveTool::Connector
        ) {
            let hit = page.objects.iter().rev().find(|obj| {
                if let DrawObject::Connector { .. } = obj {
                    hit_test_connector_with_objects(
                        obj,
                        Point2D::new(page_p.x, page_p.y),
                        8.0 / state.zoom,
                        &page.objects,
                    )
                } else {
                    hit_test_object(obj, page_p, 8.0 / state.zoom)
                }
            });
            if let Some(obj) = hit {
                let id = obj.id().to_string();
                if shift_held {
                    if let Some(idx) = state.selected_ids.iter().position(|x| x == &id) {
                        state.selected_ids.remove(idx);
                    } else {
                        state.selected_ids.push(id);
                    }
                } else {
                    state.selected_ids = vec![id];
                }
            } else if !shift_held {
                state.selected_ids.clear();
                state.editing_text_id = None;
            }
        }
    }

    // Escape key: membatalkan seleksi saat tidak sedang menyunting teks (Task 5.4)
    if ui.input(|i| i.key_pressed(Key::Escape)) && state.editing_text_id.is_none() {
        state.selected_ids.clear();
    }

    // Shortcut Ctrl+L: Lock / Unlock selected shape(s) & connector(s) (Fase G10)
    if state.editing_text_id.is_none()
        && ui.input(|i| i.modifiers.command && i.key_pressed(Key::L))
        && state.toggle_lock_selected(page)
    {
        ui.ctx().request_repaint();
    }

    // Enter / F2 key: buka inline editor jika satu bentuk/teks/sticky terpilih dan tidak terkunci (Task 6.1)
    if state.editing_text_id.is_none()
        && state.selected_ids.len() == 1
        && ui.input(|i| i.key_pressed(Key::Enter) || i.key_pressed(Key::F2))
    {
        let sel_id = state.selected_ids[0].clone();
        let can_edit = page.objects.iter().any(|o| {
            if o.id() == sel_id {
                match o {
                    DrawObject::Shape { locked, .. } => !locked,
                    DrawObject::Text { .. } | DrawObject::StickyNote { .. } => true,
                    _ => false,
                }
            } else {
                false
            }
        });
        if can_edit {
            for obj in &mut page.objects {
                if obj.id() == sel_id {
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
                    break;
                }
            }
            state.push_undo(&page.id, &page.objects);
            state.editing_text_id = Some(sel_id);
            state.focus_text_edit = true;
        }
    }

    // 9. Inline Text / StickyNote / Shape Text Editor langsung di atas kanvas halaman
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
                        .font(egui::FontId::proportional(scaled_font))
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
                    ..
                } => {
                    let p1 = page_to_screen(Pos2::new(*x, *y)) + Vec2::new(8.0, 8.0);
                    let p2 =
                        page_to_screen(Pos2::new(*x + *width, *y + *height)) - Vec2::new(8.0, 8.0);
                    let edit_rect = Rect::from_two_pos(p1, p2);
                    let scaled_font = (14.0 * state.zoom).clamp(10.0, 42.0);

                    painter.rect_stroke(
                        edit_rect.expand(6.0),
                        4.0,
                        Stroke::new(2.0_f32, Color32::from_rgb(0, 114, 206)),
                        egui::StrokeKind::Outside,
                    );

                    let te_id = egui::Id::new(("canvas_inline_sticky", &edit_id));
                    let te = egui::TextEdit::multiline(text)
                        .id(te_id)
                        .font(egui::FontId::proportional(scaled_font))
                        .text_color(Color32::from_rgb(35, 35, 35))
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

    // Context menu on selection or blank canvas
    let has_selection_on_this_page = !state.selected_ids.is_empty()
        && page
            .objects
            .iter()
            .any(|o| state.selected_ids.contains(&o.id().to_string()));

    let click_page_p = screen_to_page(
        response
            .interact_pointer_pos()
            .or_else(|| ui.input(|i| i.pointer.hover_pos()))
            .unwrap_or(rect.center()),
    );

    if response.hovered() {
        state.hovered_page_id = Some(page.id.clone());
        state.hovered_page_pos = Some(click_page_p);
    }
    if response.clicked() || response.secondary_clicked() || response.drag_started() {
        state.last_interacted_page_id = Some(page.id.clone());
    }

    response.context_menu(|ui| {
        if has_selection_on_this_page {
            let count = page
                .objects
                .iter()
                .filter(|o| state.selected_ids.contains(&o.id().to_string()))
                .count();
            ui.label(format!("{} objek dipilih", count));
            ui.separator();
            if ui.button("📋 Salin (Ctrl+C)").clicked() {
                state.copy_selected(&page.objects);
                ui.close_menu();
            }
            if ui.button("✂ Potong (Ctrl+X)").clicked() {
                state.cut_selected(page);
                ui.close_menu();
            }
            if ui.button("📄 Gandakan (Ctrl+D)").clicked() {
                state.duplicate_selected(page);
                ui.close_menu();
            }
            if ui.button("🗑 Hapus (Del)").clicked() {
                state.delete_selected(page);
                ui.close_menu();
            }
            ui.separator();
            let is_locked = state.is_selected_locked(page);
            let lock_label = if is_locked {
                "🔓 Buka Kunci (Ctrl+L)"
            } else {
                "🔒 Kunci Objek (Ctrl+L)"
            };
            if ui.button(lock_label).clicked() {
                state.toggle_lock_selected(page);
                ui.close_menu();
            }
            ui.separator();
            if ui.button("🔄 Balik Horizontal").clicked() {
                state.flip_selected_horizontal(page);
                ui.close_menu();
            }
            if ui.button("🔃 Balik Vertikal").clicked() {
                state.flip_selected_vertical(page);
                ui.close_menu();
            }
            ui.separator();
            if ui.button("⬆ Pindah ke Depan").clicked() {
                state.bring_selected_to_front(page);
                ui.close_menu();
            }
            if ui.button("⬇ Pindah ke Belakang").clicked() {
                state.send_selected_to_back(page);
                ui.close_menu();
            }
        } else {
            if !state.object_clipboard.is_empty() {
                if ui.button("📋 Tempel Objek (Ctrl+V)").clicked() {
                    state.paste_clipboard(page, Some(click_page_p));
                    state.last_interacted_page_id = Some(page.id.clone());
                    ui.close_menu();
                }
                ui.separator();
            }
            if ui.button("🖼 Tempel Gambar dari Clipboard").clicked() {
                state.request_paste_image = Some((page.id.clone(), click_page_p));
                state.last_interacted_page_id = Some(page.id.clone());
                ui.close_menu();
            }
            ui.separator();
            if ui.button("Pilih Semua (Ctrl+A)").clicked() {
                state.selected_ids = page.objects.iter().map(|o| o.id().to_string()).collect();
                ui.close_menu();
            }
        }
    });

    response
}

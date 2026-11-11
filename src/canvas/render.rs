use super::gizmo::{SelectionData, render_alignment_guides, render_selection_gizmo};
use super::laser::render_laser_trail;
use super::primitives::{
    draw_non_stroke_object, draw_paper_pattern, draw_rotated_background_image,
    draw_shape_primitive, transform_page_mesh_to_screen,
};
use super::state::CanvasState;
use super::types::{ActiveTool, EraserMode, ResizeCorner};
pub use super::widgets::FloatingToolbarStyle;
use super::widgets::{render_equation_modal_dialog, render_inline_text_editor};
use super::zoom_window::render_zoom_target_indicator;
use crate::geometry::{
    ShapeTransformBox, TransformHandleKind, calculate_proportional_scale,
    calculate_single_axis_resize, combined_bounds, find_alignment_guides, find_connector_snap_site,
    hit_test_connector_details, hit_test_connector_midpoint, hit_test_connector_with_objects,
    hit_test_object, hit_test_transform_handles, insert_control_point_at, next_shape_rect,
    object_bounds, object_in_lasso_mode, point_in_polygon, prune_collinear_control_points,
    prune_invalid_connector_control_points, recognize_handwriting_from_strokes, recognize_shape,
    resolve_connector_endpoints, scale_object, shape_connection_point, should_reject_palm_input,
    smooth_stroke_points, snap_point_to_grid, snap_value_to_grid, tessellate_stroke_styled,
    translate_object,
};
use crate::model::{
    ColorData, ConnectionSite, ConnectorEnd, ConnectorRoute, ConnectorStyle, DrawObject,
    MarkerKind, Page, Point2D, ShapeKind, StrokePoint,
};
use eframe::egui::{
    self, Color32, Key, PointerButton, Pos2, Rect, Response, Sense, Stroke, Ui, Vec2,
};
use std::time::Instant;
use uuid::Uuid;

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

    // 2. Draw existing page objects with Two-Pass Layering & Page-Space Mesh Cache:
    // Pass 1: Highlighters (Underlay Layer) — sits beneath ink handwriting, text, and diagrams so black ink is never dimmed.
    // Pass 2: Content Layer — Pen strokes, shapes, connectors, images, sticky notes, and text.
    let hl_alpha = (state.highlighter_opacity * 255.0).clamp(20.0, 255.0) as u8;

    for pass_is_highlighter in [true, false] {
        for obj in &page.objects {
            match obj {
                DrawObject::Stroke { is_highlighter, .. }
                    if *is_highlighter != pass_is_highlighter =>
                {
                    continue;
                }
                DrawObject::Stroke {
                    id,
                    points,
                    color,
                    width,
                    is_highlighter,
                    style,
                } => {
                    let obj_b = object_bounds(obj);
                    let screen_b =
                        Rect::from_min_max(page_to_screen(obj_b.min), page_to_screen(obj_b.max));
                    if !screen_clip_rect.intersects(screen_b) {
                        continue;
                    }

                    // Simpan mesh di dalam cache menggunakan koordinat lokal kertas (`|p| p`, skala 1.0),
                    // lalu transformasikan ke posisi layar (`origin` + `state.zoom`) setiap frame agar
                    // goresan tidak pernah bergeser atau hilang ketika halaman di-scroll maupun di-zoom.
                    let key = (id.clone(), 0);
                    let page_mesh = if let Some(cached) = state.mesh_cache.get(&key) {
                        cached.clone()
                    } else {
                        let smoothed = smooth_stroke_points(points, state.smoothing_steps);
                        let stroke_c = if *is_highlighter {
                            Color32::from_rgba_unmultiplied(color.r, color.g, color.b, hl_alpha)
                        } else {
                            color.to_color32()
                        };
                        let mesh = tessellate_stroke_styled(
                            &smoothed,
                            *width,
                            stroke_c,
                            *is_highlighter,
                            *style,
                            |p| p,
                        );
                        if state.mesh_cache.len() > 1500 {
                            let to_remove: Vec<_> =
                                state.mesh_cache.keys().take(300).cloned().collect();
                            for k in to_remove {
                                state.mesh_cache.remove(&k);
                            }
                        }
                        state.mesh_cache.insert(key, mesh.clone());
                        mesh
                    };
                    let screen_mesh = transform_page_mesh_to_screen(&page_mesh, origin, state.zoom);
                    painter.add(screen_mesh);
                }
                _ if pass_is_highlighter => {
                    // Non-strokes are rendered in Pass 2 (Content Layer)
                    continue;
                }
                _ => {
                    let obj_b = object_bounds(obj);
                    let screen_b =
                        Rect::from_min_max(page_to_screen(obj_b.min), page_to_screen(obj_b.max));
                    if !screen_clip_rect.intersects(screen_b) {
                        continue;
                    }

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
    let sel = SelectionData::compute(page, state);

    if response.drag_started_by(PointerButton::Primary)
        && let Some(pos) = pointer_pos
    {
        let page_p = screen_to_page(pos);

        // Check if clicking resize/rotation handles on selection
        if let Some((line_start, line_end)) = sel.single_line {
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
        } else if let Some((source_id, dots)) = &sel.single_shape_quick_dots {
            let dot_hit_dist = 14.0 / state.zoom;
            if let Some(&(site, _, _)) = dots
                .iter()
                .find(|(_, dpos, _)| page_p.distance(*dpos) <= dot_hit_dist)
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

        if let Some((conn_id, Some(poly))) = &sel.single_connector {
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
            && let Some(frame) = &sel.frame
            && !sel.is_selection_fully_locked
        {
            let handle_dist = 14.0 / state.zoom;
            if let Some(h_kind) = hit_test_transform_handles(
                &frame.handles,
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
            let inside_selection = if frame.rotation.abs() < 1e-4 {
                sel.rect.map(|r| r.contains(page_p)).unwrap_or(false)
            } else {
                point_in_polygon(page_p, &frame.corners)
            };
            if inside_selection {
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
                    let raw_pressure = ui.input(|i| {
                        for event in &i.raw.events {
                            if let egui::Event::Touch { force: Some(f), .. } = event {
                                return (*f).clamp(0.05, 1.0);
                            }
                        }
                        1.0
                    });
                    let is_highlighter = state.current_tool == ActiveTool::Highlighter;
                    let pressure = if is_highlighter && !state.highlighter_pressure_enabled {
                        1.0
                    } else {
                        state.effective_pressure(raw_pressure)
                    };
                    state.active_stroke_page_id = Some(page.id.clone());
                    state.draft_stroke = Some(vec![StrokePoint {
                        x: page_p.x,
                        y: page_p.y,
                        pressure,
                    }]);
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
                        font_size: state.text_font_size,
                        color: state.text_color.into(),
                        rotation: 0.0,
                        font_family: state.text_font_family,
                        bold: state.text_bold,
                        italic: state.text_italic,
                        alignment: state.text_alignment,
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
                    color: state.sticky_bg_color.into(),
                    text_color: state.text_color.into(),
                    font_size: state.text_font_size.clamp(10.0, 36.0),
                    font_family: state.text_font_family,
                    bold: state.text_bold,
                    italic: state.text_italic,
                    alignment: state.text_alignment,
                    rotation: 0.0,
                };
                page.objects.push(new_note);
                state.spatial_grid.rebuild(&page.objects);
                state.selected_ids = vec![id.clone()];
                state.editing_text_id = Some(id);
                state.focus_text_edit = true;
                state.current_tool = ActiveTool::Lasso;
            }
            ActiveTool::Hand | ActiveTool::Equation => {}
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
                && let Some(b) = sel.rect
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
                    let pivot = if let Some((line_start, _)) = sel.single_line {
                        line_start
                    } else if let Some(frame) = &sel.frame {
                        frame.pivot
                    } else if let Some(b) = sel.rect {
                        b.center()
                    } else {
                        page_p
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
                } else if sel.is_single_selected
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
                            let min_dist = (1.2 / state.zoom).max(0.5);
                            if !rejected
                                && let Some(last) = pts.last()
                                && (page_p - last.to_pos2()).length() >= min_dist
                            {
                                let raw_pressure = ui.input(|i| {
                                    for event in &i.raw.events {
                                        if let egui::Event::Touch { force: Some(f), .. } = event {
                                            return (*f).clamp(0.05, 1.0);
                                        }
                                    }
                                    1.0
                                });
                                let is_highlighter = state.current_tool == ActiveTool::Highlighter;
                                let pressure = if (is_highlighter
                                    && !state.highlighter_pressure_enabled)
                                    || !state.pressure_sensitivity_enabled
                                {
                                    1.0
                                } else {
                                    (state.pressure_min
                                        + (1.0 - state.pressure_min)
                                            * (raw_pressure * state.pressure_multiplier))
                                        .clamp(0.05, 1.0)
                                };
                                pts.push(StrokePoint {
                                    x: page_p.x,
                                    y: page_p.y,
                                    pressure,
                                });
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
                // Ensure exact final point without overwriting genuine pressure
                if let Some(pos) = pointer_pos {
                    let p = screen_to_page(pos);
                    let should_append = pts
                        .last()
                        .is_none_or(|last| (p - last.to_pos2()).length() >= 0.5);
                    if should_append {
                        let pressure = pts.last().map(|pt| pt.pressure).unwrap_or(1.0);
                        pts.push(StrokePoint {
                            x: p.x,
                            y: p.y,
                            pressure,
                        });
                    }
                }

                if !pts.is_empty() {
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
                        let hl_alpha = (state.highlighter_opacity * 255.0).clamp(20.0, 255.0) as u8;
                        Color32::from_rgba_unmultiplied(
                            state.stroke_color.r(),
                            state.stroke_color.g(),
                            state.stroke_color.b(),
                            hl_alpha,
                        )
                    } else {
                        state.stroke_color
                    };
                    // Fase 8.1 & G4: Shape Recognition & Determinisme Undo
                    if let Some(recognized) = recognized_shape {
                        let new_id = Uuid::new_v4().to_string();
                        let is_line = recognized.kind == ShapeKind::Line;
                        let corner_radius = 0.0;

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
                            style: state.stroke_style,
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
                            style: state.stroke_style,
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
                    let corner_radius = 0.0;
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
                        if object_in_lasso_mode(
                            obj,
                            &state.lasso_points,
                            state.lasso_selection_mode,
                        ) {
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
            let corner_radius = 0.0;
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
                let hl_alpha = (state.highlighter_opacity * 255.0).clamp(20.0, 255.0) as u8;
                Color32::from_rgba_unmultiplied(
                    state.stroke_color.r(),
                    state.stroke_color.g(),
                    state.stroke_color.b(),
                    hl_alpha,
                )
            } else {
                state.stroke_color
            };
            let smoothed = smooth_stroke_points(pts, state.smoothing_steps);
            let mesh = tessellate_stroke_styled(
                &smoothed,
                stroke_width,
                color,
                is_highlighter,
                state.stroke_style,
                page_to_screen,
            );
            painter.add(mesh);
        }

        if let (Some(start), Some(curr)) = (state.draft_shape_start, state.draft_shape_current) {
            let stroke = Stroke::new(state.stroke_width * state.zoom, state.stroke_color);
            if let ActiveTool::Shape(kind) = state.current_tool {
                let w = curr.x - start.x;
                let h = curr.y - start.y;
                let corner_radius = 0.0;
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

    // 6. Render Selection Bounding Box, Handles & Floating Toolbars (recomputed after drag mutations)
    let updated_sel = SelectionData::compute(page, state);
    render_selection_gizmo(
        &painter,
        ui,
        page,
        state,
        &updated_sel,
        rect,
        pointer_pos,
        page_to_screen,
        screen_to_page,
    );

    // Fase G11: Render Smart Alignment Guides (Garis Panduan Magnetik Berwarna Magenta)
    render_alignment_guides(&painter, &state.active_alignment_guides, page_to_screen);

    // 7. Render Glowing Laser Trail (Multi-stroke Neon Laser)
    render_laser_trail(&painter, state, laser_alpha);

    // 8. Fase 8.3: Render Indikator Kotak Target Zoom Window pada Kanvas Utama
    render_zoom_target_indicator(&painter, state, page_to_screen);

    // Task 9.6: Quick Diagramming via Keyboard (Tab: Right / Child, Enter: Bottom / Sibling)
    if state.editing_text_id.is_none()
        && let Some((source_id, _)) = &updated_sel.single_shape_quick_dots
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
        && let Some((conn_id, Some(poly))) = &updated_sel.single_connector
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

    // Double-click pada objek Teks, StickyNote, Shape, atau Equation untuk menyunting isinya secara langsung (Task 6.1 & 10)
    if response.double_clicked()
        && let Some(pos) = pointer_pos
    {
        let page_p = screen_to_page(pos);
        if let Some(obj) = page.objects.iter_mut().rev().find(|obj| {
            matches!(
                obj,
                DrawObject::Text { .. }
                    | DrawObject::StickyNote { .. }
                    | DrawObject::Shape { .. }
                    | DrawObject::Equation { .. }
            ) && hit_test_object(obj, page_p, 8.0 / state.zoom)
        }) && !obj.is_locked()
        {
            let id = obj.id().to_string();
            if let DrawObject::Equation {
                latex, font_size, ..
            } = obj
            {
                state.show_equation_dialog = true;
                state.editing_equation_id = Some(id.clone());
                state.equation_latex_buffer = latex.clone();
                state.equation_font_size = *font_size;
                state.selected_ids = vec![id];
            } else {
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
                    font_size: state.text_font_size,
                    color: state.text_color.into(),
                    rotation: 0.0,
                    font_family: state.text_font_family,
                    bold: state.text_bold,
                    italic: state.text_italic,
                    alignment: state.text_alignment,
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
                color: state.sticky_bg_color.into(),
                text_color: state.text_color.into(),
                font_size: state.text_font_size.clamp(10.0, 36.0),
                font_family: state.text_font_family,
                bold: state.text_bold,
                italic: state.text_italic,
                alignment: state.text_alignment,
                rotation: 0.0,
            };
            page.objects.push(new_note);
            state.spatial_grid.rebuild(&page.objects);
            state.selected_ids = vec![id.clone()];
            state.editing_text_id = Some(id);
            state.focus_text_edit = true;
            state.current_tool = ActiveTool::Lasso;
        } else if state.current_tool == ActiveTool::Equation {
            let hit_eq = page
                .objects
                .iter()
                .rev()
                .find(|obj| {
                    matches!(obj, DrawObject::Equation { .. })
                        && hit_test_object(obj, page_p, 8.0 / state.zoom)
                })
                .map(|o| {
                    (
                        o.id().to_string(),
                        if let DrawObject::Equation {
                            latex, font_size, ..
                        } = o
                        {
                            (latex.clone(), *font_size)
                        } else {
                            (String::new(), 22.0)
                        },
                    )
                });

            if let Some((id, (latex, fsz))) = hit_eq {
                state.show_equation_dialog = true;
                state.editing_equation_id = Some(id.clone());
                state.equation_latex_buffer = latex;
                state.equation_font_size = fsz;
                state.selected_ids = vec![id];
            } else {
                state.show_equation_dialog = true;
                state.editing_equation_id = None;
                state.equation_insert_pos = page_p;
                state.equation_latex_buffer = String::new();
            }
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
            ActiveTool::Pen | ActiveTool::Pencil | ActiveTool::Highlighter
        ) {
            // Task 1: Dukung click-only / tap tunggal untuk menghasilkan titik goresan yang valid
            let raw_pressure = ui.input(|i| {
                for event in &i.raw.events {
                    if let egui::Event::Touch { force: Some(f), .. } = event {
                        return (*f).clamp(0.05, 1.0);
                    }
                }
                1.0
            });
            let is_highlighter = state.current_tool == ActiveTool::Highlighter;
            let pressure = if is_highlighter && !state.highlighter_pressure_enabled {
                1.0
            } else {
                state.effective_pressure(raw_pressure)
            };
            let pts = state.draft_stroke.take().unwrap_or_else(|| {
                vec![StrokePoint {
                    x: page_p.x,
                    y: page_p.y,
                    pressure,
                }]
            });
            state.push_undo(&page.id, &page.objects);
            let stroke_width = if is_highlighter {
                state.stroke_width * 6.0
            } else if state.current_tool == ActiveTool::Pencil {
                (state.stroke_width - 0.5).max(1.0)
            } else {
                state.stroke_width
            };
            let color = if is_highlighter {
                let hl_alpha = (state.highlighter_opacity * 255.0).clamp(20.0, 255.0) as u8;
                Color32::from_rgba_unmultiplied(
                    state.stroke_color.r(),
                    state.stroke_color.g(),
                    state.stroke_color.b(),
                    hl_alpha,
                )
            } else {
                state.stroke_color
            };
            page.objects.push(DrawObject::Stroke {
                id: Uuid::new_v4().to_string(),
                points: pts,
                color: color.into(),
                width: stroke_width,
                is_highlighter,
                style: state.stroke_style,
            });
            state.spatial_grid.rebuild(&page.objects);
            page.ocr_text = recognize_handwriting_from_strokes(&page.objects);
            ui.ctx().request_repaint();
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
    render_inline_text_editor(ui, &painter, page, state, page_to_screen);

    // 10. Dialog Modal LaTeX Equation Tool (Task 10)
    render_equation_modal_dialog(ui.ctx(), state, page);

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

use super::gizmo::SelectionData;
use super::sampling::{
    FramePointerBatch, PointerSamplePhase, PointerSource, RawPointerSample, RawScreenEvent,
    append_lasso_sample, append_stroke_sample, collect_frame_pointer_batch, extract_touch_pressure,
};
use super::state::CanvasState;
use super::types::{ActiveTool, EraserMode, ResizeCorner};
use crate::geometry::{
    ShapeTransformBox, TransformHandleKind, calculate_proportional_scale,
    calculate_single_axis_resize, combined_bounds, find_alignment_guides, find_connector_snap_site,
    hit_test_connector_details, hit_test_connector_midpoint, hit_test_object,
    hit_test_transform_handles, insert_control_point_at, next_shape_rect, object_bounds,
    object_in_lasso_mode_with_context, point_in_polygon,
    prune_all_invalid_connector_control_points, prune_collinear_control_points,
    recognize_handwriting_from_strokes, recognize_shape, resolve_connector_endpoints, scale_object,
    shape_connection_point, snap_point_to_grid, snap_value_to_grid, translate_object,
};
use crate::model::{
    ColorData, ConnectionSite, ConnectorEnd, ConnectorRoute, ConnectorStyle, DrawObject,
    MarkerKind, Page, Point2D, ShapeKind,
};
use eframe::egui::{self, Color32, Key, PointerButton, Pos2, Rect, Response, Ui, Vec2};
use std::time::Instant;
use uuid::Uuid;

/// Memeriksa apakah titik halaman `page_p` mengenai handle transformasi, garis, konektor,
/// titik Quick Diagram, atau area seleksi aktif.
fn hits_selection_or_diagram_handle(
    page: &Page,
    state: &CanvasState,
    sel: &SelectionData,
    page_p: Pos2,
) -> bool {
    if let Some((line_start, line_end)) = sel.single_line {
        let handle_dist = 14.0 / state.zoom;
        let dir = line_end - line_start;
        let dir_norm = if dir.length() > 1e-3 {
            dir / dir.length()
        } else {
            Vec2::new(1.0, 0.0)
        };
        let line_rot_handle = line_end + dir_norm * (22.0 / state.zoom);
        if (page_p - line_rot_handle).length() <= handle_dist
            || (page_p - line_end).length() <= handle_dist
            || (page_p - line_start).length() <= handle_dist
            || (state.current_tool == ActiveTool::Lasso
                && crate::geometry::point_to_segment_dist(page_p, line_start, line_end)
                    <= handle_dist)
        {
            return true;
        }
    } else if hit_test_quick_diagram_dot(sel, state, page_p).is_some() {
        return true;
    }

    if let Some((conn_id, Some(poly))) = &sel.single_connector {
        let is_conn_locked = page
            .objects
            .iter()
            .any(|o| o.id() == conn_id && o.is_locked());
        if !is_conn_locked {
            let handle_dist = 14.0 / state.zoom;
            let pt = Point2D::new(page_p.x, page_p.y);
            if hit_test_connector_midpoint(pt, poly, handle_dist).is_some()
                || hit_test_connector_details(pt, poly, handle_dist).is_some()
            {
                return true;
            }
        }
    } else if matches!(state.current_tool, ActiveTool::Lasso | ActiveTool::Shape(_))
        && let Some(frame) = &sel.frame
        && !sel.is_selection_fully_locked
    {
        let handle_dist = 14.0 / state.zoom;
        if hit_test_transform_handles(
            &frame.handles,
            Point2D::new(page_p.x, page_p.y),
            handle_dist,
        )
        .is_some()
        {
            return true;
        }
        let inside_selection = if frame.rotation.abs() < 1e-4 {
            sel.rect.map(|r| r.contains(page_p)).unwrap_or(false)
        } else {
            point_in_polygon(page_p, &frame.corners)
        };
        if inside_selection {
            return true;
        }
    }

    false
}

/// Menangani seluruh interaksi drag primer, sampling pointer berpresisi tinggi, dan hold-to-snap pada kanvas halaman.
#[allow(clippy::too_many_arguments)]
pub(crate) fn handle_pointer_drag_interaction(
    ui: &Ui,
    response: &Response,
    page: &mut Page,
    state: &mut CanvasState,
    sel: &SelectionData,
    pointer_pos: Option<Pos2>,
    now: Instant,
    screen_to_page: impl Fn(Pos2) -> Pos2 + Copy,
) {
    state.stroke_committed_this_frame = false;

    let (press_origin, primary_down, multi_touch, frame_dt, batch) = ui.input(|i| {
        let press_orig = i.pointer.press_origin();
        let prim_down = i.pointer.primary_down();
        let mt = i.multi_touch().is_some();
        let dt = i.stable_dt.clamp(0.001, 0.1);
        let b = collect_frame_pointer_batch(
            &i.raw.events,
            state.active_touch_id,
            press_orig,
            pointer_pos,
        );
        (press_orig, prim_down, mt, dt, b)
    });

    let owns_active_gesture = state.pointer_gesture_active
        && (state.active_stroke_page_id.as_deref() == Some(&page.id)
            || state.last_erase_sample_pos.is_some()
            || state.laser_is_drawing
            || state.is_dragging_selection
            || state.resize_corner.is_some());

    // Tangani pembatalan pointer (PointerGone atau TouchPhase::Cancel) secara aman
    if batch.cancelled && (owns_active_gesture || response.is_pointer_button_down_on()) {
        state.active_alignment_guides.clear();
        state.eraser_undo_pushed = false;
        state.last_whole_erase_pos = None;
        state.last_erase_sample_pos = None;
        state.is_dragging_selection = false;
        state.drag_start_pos = None;
        state.resize_corner = None;
        state.laser_is_drawing = false;
        state.current_laser_stroke.clear();
        state.clear_active_draft_state();
        return;
    }

    // Catat event tekan awal (Press) pada kanvas ini agar tekanan/sumber awal tidak hilang bila drag baru dimulai di frame berikutnya
    if let Some(press_ev) = batch
        .events
        .iter()
        .find(|e| e.phase == PointerSamplePhase::Press && response.rect.contains(e.screen_pos))
    {
        state.pending_press_event = Some(*press_ev);
        if batch.active_touch_id.is_some() {
            state.active_touch_id = batch.active_touch_id;
        }
    }

    let mut started_this_frame = false;
    if response.drag_started_by(PointerButton::Primary)
        && let Some(pos) = pointer_pos
    {
        if !state.pointer_gesture_active {
            handle_drag_started(ui, page, state, sel, pos, &batch, screen_to_page);
            state.pointer_gesture_active = true;
            started_this_frame = true;
        }
    } else if !state.pointer_gesture_active
        && matches!(
            state.current_tool,
            ActiveTool::Pen
                | ActiveTool::Pencil
                | ActiveTool::Highlighter
                | ActiveTool::Eraser
                | ActiveTool::Lasso
                | ActiveTool::Laser
        )
    {
        // Tangkap goresan/gerakan sangat pendek (< threshold drag 6px egui) sejak pointer ditekan dan bergerak
        let press_in_this_canvas = response.is_pointer_button_down_on()
            || ((response.hovered() || response.clicked() || response.drag_stopped())
                && batch.events.iter().any(|e| {
                    e.phase == PointerSamplePhase::Press && response.rect.contains(e.screen_pos)
                }));
        let start_candidate = press_origin
            .or_else(|| {
                batch
                    .events
                    .iter()
                    .find(|e| e.phase == PointerSamplePhase::Press)
                    .map(|e| e.screen_pos)
            })
            .or(pointer_pos);

        if press_in_this_canvas
            && let Some(start_pos) = start_candidate
            && response.rect.contains(start_pos)
        {
            let has_movement = batch
                .events
                .iter()
                .any(|e| (e.screen_pos - start_pos).length_sq() > 1e-6)
                || pointer_pos.is_some_and(|p| (p - start_pos).length_sq() > 1e-6);
            let has_pressure_change = batch.events.len() >= 2
                && batch
                    .events
                    .windows(2)
                    .any(|w| match (w[0].raw_pressure, w[1].raw_pressure) {
                        (Some(a), Some(b)) => (a - b).abs() > 1e-4,
                        _ => false,
                    });

            let start_page_p = screen_to_page(start_pos);
            if (has_movement || has_pressure_change)
                && !hits_selection_or_diagram_handle(page, state, sel, start_page_p)
            {
                handle_drag_started(ui, page, state, sel, start_pos, &batch, screen_to_page);
                state.pointer_gesture_active = true;
                started_this_frame = true;
            }
        }
    }

    let is_active_on_this_page = state.pointer_gesture_active
        && (state.active_stroke_page_id.as_deref() == Some(&page.id)
            || state.last_erase_sample_pos.is_some()
            || state.laser_is_drawing
            || state.is_dragging_selection
            || state.resize_corner.is_some());

    if response.dragged_by(PointerButton::Primary) || is_active_on_this_page {
        let start_ref = press_origin.or(pointer_pos);
        let sample_count = batch.events.len().max(1);
        let dt_per_sample = frame_dt / (sample_count as f32);
        let mut last_processed_screen_pos: Option<Pos2> = None;

        for (idx, ev) in batch.events.iter().enumerate() {
            if ev.phase == PointerSamplePhase::Cancel {
                continue;
            }
            // Jika baru saja memulai gesture di frame ini pada titik awal yang sama dan tekanan yang sama, lewati sampel awal yang sudah dicatat
            if started_this_frame
                && idx == 0
                && let Some(s_ref) = start_ref
                && (ev.screen_pos - s_ref).length_sq() <= 1e-6
                && state.raw_stroke_samples.first().is_some_and(|first| {
                    ev.raw_pressure
                        .is_none_or(|rp| first.raw_pressure == Some(rp))
                })
            {
                last_processed_screen_pos = Some(ev.screen_pos);
                continue;
            }

            handle_drag_moved(
                ui,
                response,
                page,
                state,
                sel,
                *ev,
                dt_per_sample,
                multi_touch,
                now,
                screen_to_page,
            );
            last_processed_screen_pos = Some(ev.screen_pos);
        }

        if let Some(pos) = pointer_pos
            && last_processed_screen_pos.is_none_or(|last_p| (last_p - pos).length_sq() > 1e-6)
            && !(started_this_frame
                && start_ref.is_some_and(|s_ref| (s_ref - pos).length_sq() <= 1e-6))
        {
            let fallback_ev = RawScreenEvent {
                screen_pos: pos,
                raw_pressure: batch.events.last().and_then(|e| e.raw_pressure),
                phase: PointerSamplePhase::Move,
                source: batch
                    .events
                    .last()
                    .map(|e| e.source)
                    .unwrap_or(PointerSource::Mouse),
            };
            handle_drag_moved(
                ui,
                response,
                page,
                state,
                sel,
                fallback_ev,
                dt_per_sample,
                multi_touch,
                now,
                screen_to_page,
            );
        }
    }

    // Fase 8.1 & G4: Deteksi tahan pena (~500 ms) di akhir goresan untuk Shape Recognition
    update_hold_to_snap(ui, page, state, now);

    let released_in_batch = batch
        .events
        .iter()
        .any(|e| e.phase == PointerSamplePhase::Release);
    if response.drag_stopped() || (is_active_on_this_page && (!primary_down || released_in_batch)) {
        let stop_pos = batch
            .events
            .iter()
            .rev()
            .find(|e| e.phase == PointerSamplePhase::Release)
            .map(|e| e.screen_pos)
            .or(pointer_pos)
            .or_else(|| batch.events.last().map(|e| e.screen_pos));
        handle_drag_stopped(ui, page, state, stop_pos, &batch, screen_to_page);
    }
}

/// Menangani awal gesture drag primer: seleksi handle/garis/titik Quick Diagram atau mulai alat aktif.
fn handle_drag_started(
    ui: &Ui,
    page: &mut Page,
    state: &mut CanvasState,
    sel: &SelectionData,
    pos: Pos2,
    batch: &FramePointerBatch,
    screen_to_page: impl Fn(Pos2) -> Pos2 + Copy,
) {
    // Gunakan titik tekan awal (press_origin) untuk hit-test handle agar seretan cepat tidak meleset
    let press_pos = ui.input(|i| i.pointer.press_origin()).unwrap_or(pos);
    let page_p = screen_to_page(press_pos);

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
            return;
        }
        // Handle titik akhir garis (dapat berotasi & diatur terhadap titik awal sebagai pusat)
        if (page_p - line_end).length() <= handle_dist {
            state.resize_corner = Some(ResizeCorner::LineEnd);
            state.drag_start_pos = Some(page_p);
            state.push_undo(&page.id, &page.objects);
            return;
        }
        // Handle titik awal garis (pusat rotasi)
        if (page_p - line_start).length() <= handle_dist {
            state.resize_corner = Some(ResizeCorner::LineStart);
            state.drag_start_pos = Some(page_p);
            state.push_undo(&page.id, &page.objects);
            return;
        }
        // Klik pada badan garis untuk menggeser garis (hanya saat menggunakan alat Lasso)
        if state.current_tool == ActiveTool::Lasso
            && crate::geometry::point_to_segment_dist(page_p, line_start, line_end) <= handle_dist
        {
            state.is_dragging_selection = true;
            state.drag_start_pos = Some(page_p);
            state.push_undo(&page.id, &page.objects);
            return;
        }
    } else if let Some((source_id, site)) = hit_test_quick_diagram_dot(sel, state, page_p) {
        state.clear_active_draft_state();
        state.active_stroke_page_id = Some(page.id.clone());
        state.resize_corner = Some(ResizeCorner::QuickDiagramDot(site));
        state.drag_start_pos = Some(page_p);
        state.draft_connector_start = Some(ConnectorEnd::Attached {
            shape_id: source_id,
            site,
        });
        state.draft_connector_current = Some(Point2D::new(page_p.x, page_p.y));
        return;
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
                        new_idx = insert_control_point_at(control_points, start_pt, end_pt, mid_pt);
                    }
                }
                state.resize_corner = Some(ResizeCorner::ConnectorControl(new_idx));
                state.drag_start_pos = Some(page_p);
                return;
            } else if let Some(target) =
                hit_test_connector_details(Point2D::new(page_p.x, page_p.y), poly, handle_dist)
            {
                state.push_undo(&page.id, &page.objects);
                let corner = match target {
                    crate::geometry::ConnectorHitTarget::StartHandle => {
                        ResizeCorner::ConnectorStart
                    }
                    crate::geometry::ConnectorHitTarget::EndHandle => ResizeCorner::ConnectorEnd,
                    crate::geometry::ConnectorHitTarget::ControlPoint(idx) => {
                        let cp_idx = if idx > 0 { idx - 1 } else { 0 };
                        ResizeCorner::ConnectorControl(cp_idx)
                    }
                    crate::geometry::ConnectorHitTarget::LineSegment(_seg_idx) => {
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
                return;
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
            return;
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
            return;
        }
    }

    match state.current_tool {
        ActiveTool::Pen | ActiveTool::Pencil | ActiveTool::Highlighter => {
            // Fase 8.5: Palm Rejection — tolak awal goresan jika multi-touch terdeteksi
            let multi_touch = ui.input(|i| i.multi_touch().is_some());
            if !(state.palm_rejection_enabled && multi_touch) {
                let saved_press = state
                    .pending_press_event
                    .take()
                    .filter(|e| (e.screen_pos - press_pos).length_sq() <= 4.0);
                let saved_touch_id = state.active_touch_id;
                state.clear_active_draft_state();
                state.active_touch_id = batch.active_touch_id.or(saved_touch_id);
                let init_event = saved_press
                    .or_else(|| {
                        batch
                            .events
                            .iter()
                            .find(|e| (e.screen_pos - press_pos).length_sq() <= 4.0)
                            .copied()
                    })
                    .or_else(|| batch.events.first().copied())
                    .unwrap_or(RawScreenEvent {
                        screen_pos: press_pos,
                        raw_pressure: None,
                        phase: PointerSamplePhase::Press,
                        source: PointerSource::Mouse,
                    });
                let is_highlighter = state.current_tool == ActiveTool::Highlighter;
                let pressure =
                    state.resolve_sample_pressure(init_event.raw_pressure, is_highlighter, None);
                let first_sample = RawPointerSample {
                    screen_pos: press_pos,
                    page_pos: page_p,
                    raw_pressure: init_event.raw_pressure,
                    effective_pressure: pressure,
                    phase: PointerSamplePhase::Press,
                    source: init_event.source,
                    sequence: 0,
                    dt_secs: 0.0,
                };
                state.pointer_sample_seq = 1;
                state.active_stroke_page_id = Some(page.id.clone());
                state.raw_stroke_samples = vec![first_sample];
                state.draft_stroke = Some(vec![first_sample.to_stroke_point()]);
                state.last_stroke_move_time = Some(Instant::now());
                state.selected_ids.clear();
            }
        }
        ActiveTool::Shape(_) => {
            state.clear_active_draft_state();
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
            state.clear_active_draft_state();
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
            state.selected_ids.clear();
        }
        ActiveTool::Eraser => {
            state.eraser_undo_pushed = false;
            state.last_whole_erase_pos = None;
            state.last_erase_sample_pos = Some(page_p);
            state.raw_stroke_samples = vec![RawPointerSample {
                screen_pos: press_pos,
                page_pos: page_p,
                raw_pressure: None,
                effective_pressure: 1.0,
                phase: PointerSamplePhase::Press,
                source: PointerSource::Mouse,
                sequence: 0,
                dt_secs: 0.0,
            }];
            state.pointer_sample_seq = 1;
            let erase_radius = match state.eraser_mode {
                EraserMode::WholeStroke => (3.8 / state.zoom).clamp(1.8, 4.5),
                EraserMode::Partial => (10.0 / state.zoom).clamp(4.0, 16.0),
            };
            state.erase_at(page, page_p, erase_radius);
        }
        ActiveTool::Lasso => {
            state.clear_active_draft_state();
            state.active_stroke_page_id = Some(page.id.clone());
            state.selected_ids.clear();
            let first_sample = RawPointerSample {
                screen_pos: press_pos,
                page_pos: page_p,
                raw_pressure: None,
                effective_pressure: 1.0,
                phase: PointerSamplePhase::Press,
                source: PointerSource::Mouse,
                sequence: 0,
                dt_secs: 0.0,
            };
            state.pointer_sample_seq = 1;
            state.raw_stroke_samples = vec![first_sample];
            state.lasso_points = vec![page_p];
        }
        ActiveTool::Laser => {
            state.laser_is_drawing = true;
            state.laser_last_active = Instant::now();
            state.current_laser_stroke = vec![press_pos];
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
                state.rebuild_spatial_grid(&page.id, &page.objects);
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
            state.rebuild_spatial_grid(&page.id, &page.objects);
            state.selected_ids = vec![id.clone()];
            state.editing_text_id = Some(id);
            state.focus_text_edit = true;
            state.current_tool = ActiveTool::Lasso;
        }
        ActiveTool::Hand | ActiveTool::Equation => {}
    }
}

/// Menangani pergerakan drag primer per sampel event: memindahkan seleksi, mengubah ukuran/rotasi handle, atau memperbarui draft alat.
#[allow(clippy::too_many_arguments)]
fn handle_drag_moved(
    ui: &Ui,
    response: &Response,
    page: &mut Page,
    state: &mut CanvasState,
    sel: &SelectionData,
    ev: RawScreenEvent,
    sample_dt_secs: f32,
    multi_touch: bool,
    now: Instant,
    screen_to_page: impl Fn(Pos2) -> Pos2 + Copy,
) {
    let pos = ev.screen_pos;
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
                let snap = find_connector_snap_site(
                    &page.objects,
                    Point2D::new(page_p.x, page_p.y),
                    24.0 / state.zoom,
                    None,
                );
                state.connector_snap_candidate = snap.clone();
                if let ResizeCorner::ConnectorControl(idx) = corner {
                    let endpoints: Vec<(usize, Point2D, Point2D)> = page
                        .objects
                        .iter()
                        .enumerate()
                        .filter_map(|(obj_idx, obj)| {
                            if state.selected_ids.iter().any(|id| id == obj.id())
                                && let DrawObject::Connector {
                                    start,
                                    end,
                                    route: ConnectorRoute::Orthogonal,
                                    ..
                                } = obj
                            {
                                let ((start_pt, _), (end_pt, _)) =
                                    resolve_connector_endpoints(start, end, &page.objects);
                                Some((obj_idx, start_pt, end_pt))
                            } else {
                                None
                            }
                        })
                        .collect();
                    for (obj_idx, obj) in page.objects.iter_mut().enumerate() {
                        if state.selected_ids.iter().any(|id| id == obj.id())
                            && let DrawObject::Connector {
                                route,
                                control_points,
                                ..
                            } = obj
                        {
                            let mut target_pt = Point2D::new(page_p.x, page_p.y);
                            if *route == ConnectorRoute::Orthogonal
                                && let Some(&(_, start_pt, end_pt)) =
                                    endpoints.iter().find(|&&(i, _, _)| i == obj_idx)
                            {
                                let prev = if idx == 0 {
                                    start_pt
                                } else if idx - 1 < control_points.len() {
                                    control_points[idx - 1]
                                } else {
                                    target_pt
                                };
                                let next = if idx + 1 < control_points.len() {
                                    control_points[idx + 1]
                                } else {
                                    end_pt
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
                    }
                } else {
                    for obj in &mut page.objects {
                        if state.selected_ids.iter().any(|id| id == obj.id())
                            && let DrawObject::Connector { start, end, .. } = obj
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
                                _ => {}
                            }
                        }
                    }
                }
                state.drag_start_pos = Some(page_p);
                ui.ctx().request_repaint();
            } else if matches!(corner, ResizeCorner::QuickDiagramDot(_)) {
                state.draft_connector_current = Some(Point2D::new(page_p.x, page_p.y));
                let snap = find_connector_snap_site(
                    &page.objects,
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
                                let snapped_angle = (angle / snap_interval).round() * snap_interval;
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
                    } else if state.draft_stroke.is_some() {
                        if let PointerSource::Touch { id, .. } = ev.source
                            && state.active_touch_id.is_none()
                        {
                            state.active_touch_id = Some(id);
                        }
                        let dt_secs = state
                            .last_stroke_move_time
                            .map(|t| {
                                now.saturating_duration_since(t)
                                    .as_secs_f32()
                                    .max(sample_dt_secs)
                            })
                            .unwrap_or(sample_dt_secs);
                        let is_highlighter = state.current_tool == ActiveTool::Highlighter;
                        let prev_pressure = state
                            .draft_stroke
                            .as_ref()
                            .and_then(|pts| pts.last().map(|p| p.pressure));
                        let pressure = state.resolve_sample_pressure(
                            ev.raw_pressure,
                            is_highlighter,
                            prev_pressure,
                        );
                        let seq = state.pointer_sample_seq;
                        let sample = RawPointerSample {
                            screen_pos: pos,
                            page_pos: page_p,
                            raw_pressure: ev.raw_pressure,
                            effective_pressure: pressure,
                            phase: ev.phase,
                            source: ev.source,
                            sequence: seq,
                            dt_secs,
                        };
                        let prev_screen_pos = state.raw_stroke_samples.last().map(|s| s.screen_pos);
                        let mut draft_pts = state.draft_stroke.take().unwrap_or_default();
                        let added = append_stroke_sample(
                            &mut state.raw_stroke_samples,
                            &mut draft_pts,
                            sample,
                            state.palm_rejection_enabled,
                            multi_touch,
                        );
                        state.draft_stroke = Some(draft_pts);
                        if added {
                            state.pointer_sample_seq = seq + 1;
                            if prev_screen_pos.is_none_or(|p| (pos - p).length_sq() >= 0.04) {
                                state.last_stroke_move_time = Some(now);
                            }
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
                        Some(ConnectorEnd::Attached { shape_id, .. }) => Some(shape_id.as_str()),
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
                let seq = state.pointer_sample_seq;
                state.pointer_sample_seq = seq + 1;
                state.raw_stroke_samples.push(RawPointerSample {
                    screen_pos: pos,
                    page_pos: page_p,
                    raw_pressure: ev.raw_pressure,
                    effective_pressure: 1.0,
                    phase: ev.phase,
                    source: ev.source,
                    sequence: seq,
                    dt_secs: sample_dt_secs,
                });
                let erased = if let Some(prev_p) = state.last_erase_sample_pos {
                    state.erase_along_segment(page, prev_p, page_p, erase_radius)
                } else {
                    state.erase_at(page, page_p, erase_radius)
                };
                state.last_erase_sample_pos = Some(page_p);
                if erased {
                    ui.ctx().request_repaint();
                }
            }
            ActiveTool::Lasso => {
                if state.active_stroke_page_id.as_deref() == Some(&page.id) {
                    let seq = state.pointer_sample_seq;
                    let sample = RawPointerSample {
                        screen_pos: pos,
                        page_pos: page_p,
                        raw_pressure: ev.raw_pressure,
                        effective_pressure: 1.0,
                        phase: ev.phase,
                        source: ev.source,
                        sequence: seq,
                        dt_secs: sample_dt_secs,
                    };
                    if append_lasso_sample(
                        &mut state.raw_stroke_samples,
                        &mut state.lasso_points,
                        sample,
                    ) {
                        state.pointer_sample_seq = seq + 1;
                    }
                }
            }
            ActiveTool::Laser => {
                state.laser_is_drawing = true;
                state.laser_last_active = Instant::now();
                let should_add = state
                    .current_laser_stroke
                    .last()
                    .is_none_or(|last_p| last_p.distance(pos) > 0.5);
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

/// Menangani akhir gesture drag: menyelesaikan laser, seleksi/resize, Quick Diagramming, goresan, bentuk, konektor, atau lasso.
fn handle_drag_stopped(
    _ui: &Ui,
    page: &mut Page,
    state: &mut CanvasState,
    pointer_pos: Option<Pos2>,
    batch: &FramePointerBatch,
    screen_to_page: impl Fn(Pos2) -> Pos2 + Copy,
) {
    state.pointer_gesture_active = false;
    state.active_alignment_guides.clear();
    if state.current_tool == ActiveTool::Eraser
        && let Some(pos) = pointer_pos
        && let Some(prev_p) = state.last_erase_sample_pos.take()
    {
        let page_p = screen_to_page(pos);
        if (page_p - prev_p).length_sq() > 1e-6 {
            let erase_radius = match state.eraser_mode {
                EraserMode::WholeStroke => (3.8 / state.zoom).clamp(1.8, 4.5),
                EraserMode::Partial => (10.0 / state.zoom).clamp(4.0, 16.0),
            };
            state.erase_along_segment(page, prev_p, page_p, erase_radius);
        }
        state.stroke_committed_this_frame = true;
    }
    state.eraser_undo_pushed = false;
    state.last_whole_erase_pos = None;
    state.last_erase_sample_pos = None;
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
        // Task 8.5: Periksa bila ada shape bergerak yang menimpa control points pada connector tanpa clone objek
        prune_all_invalid_connector_control_points(&mut page.objects);
        state.rebuild_spatial_grid(&page.id, &page.objects);
    } else if let Some(corner) = state.resize_corner.take() {
        let drag_start_pos = state.drag_start_pos.take();
        // Task 8.3: Jika selesai drag control point, bersihkan jika hampir segaris tanpa clone objek
        if matches!(corner, ResizeCorner::ConnectorControl(_)) {
            let endpoints: Vec<(usize, Point2D, Point2D)> = page
                .objects
                .iter()
                .enumerate()
                .filter_map(|(i, obj)| {
                    if state.selected_ids.iter().any(|id| id == obj.id())
                        && let DrawObject::Connector {
                            start,
                            end,
                            control_points,
                            ..
                        } = obj
                        && !control_points.is_empty()
                    {
                        let ((start_pt, _), (end_pt, _)) =
                            resolve_connector_endpoints(start, end, &page.objects);
                        Some((i, start_pt, end_pt))
                    } else {
                        None
                    }
                })
                .collect();
            for (i, start_pt, end_pt) in endpoints {
                if let DrawObject::Connector { control_points, .. } = &mut page.objects[i] {
                    prune_collinear_control_points(control_points, start_pt, end_pt, 6.0);
                }
            }
        }

        // Task 9.2 & 9.3: Quick Diagramming (Ketuk titik atau seret konektor)
        if let ResizeCorner::QuickDiagramDot(site) = corner {
            let stop_page_p = pointer_pos
                .map(screen_to_page)
                .or_else(|| state.draft_connector_current.map(|p| p.to_pos2()))
                .or(drag_start_pos)
                .unwrap_or(Pos2::ZERO);
            complete_quick_diagram_dot_interaction(page, state, site, drag_start_pos, stop_page_p);
        }
        state.rebuild_spatial_grid(&page.id, &page.objects);
    } else if state.active_stroke_page_id.as_deref() == Some(&page.id) {
        state.active_stroke_page_id = None;
        state.last_stroke_move_time = None;
        let recognized_shape = state.recognized_draft_shape.take();
        if let Some(mut pts) = state.draft_stroke.take() {
            // Pastikan titik akhir pelepasan pointer tercatat secara akurat tanpa membuang gerakan kecil
            if let Some(pos) = pointer_pos {
                let p = screen_to_page(pos);
                let release_ev = batch
                    .events
                    .iter()
                    .rev()
                    .find(|e| (e.screen_pos - pos).length_sq() <= 4.0)
                    .or_else(|| batch.events.last());
                let raw_force = release_ev.and_then(|e| e.raw_pressure);
                let is_highlighter = state.current_tool == ActiveTool::Highlighter;
                let prev_pressure = pts.last().map(|pt| pt.pressure);
                let pressure =
                    state.resolve_sample_pressure(raw_force, is_highlighter, prev_pressure);
                let seq = state.pointer_sample_seq;
                let release_sample = RawPointerSample {
                    screen_pos: pos,
                    page_pos: p,
                    raw_pressure: raw_force,
                    effective_pressure: pressure,
                    phase: PointerSamplePhase::Release,
                    source: release_ev.map(|e| e.source).unwrap_or(PointerSource::Mouse),
                    sequence: seq,
                    dt_secs: 0.0,
                };
                if append_stroke_sample(
                    &mut state.raw_stroke_samples,
                    &mut pts,
                    release_sample,
                    false,
                    false,
                ) {
                    state.pointer_sample_seq = seq + 1;
                }
            }
            state.last_completed_raw_samples = std::mem::take(&mut state.raw_stroke_samples);
            state.active_touch_id = None;

            if !pts.is_empty() {
                state.stroke_committed_this_frame = true;
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
                    state.rebuild_spatial_grid(&page.id, &page.objects);
                } else {
                    page.objects.push(DrawObject::Stroke {
                        id: Uuid::new_v4().to_string(),
                        points: pts,
                        color: color.into(),
                        width: stroke_width,
                        is_highlighter,
                        style: state.stroke_style,
                    });
                    state.rebuild_spatial_grid(&page.id, &page.objects);
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
                state.rebuild_spatial_grid(&page.id, &page.objects);
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
                        stroke_color: ColorData {
                            r: 40,
                            g: 45,
                            b: 55,
                            a: 255,
                        },
                        stroke_width: state.stroke_width.clamp(1.5, 6.0),
                        start_marker: MarkerKind::None,
                        end_marker: MarkerKind::Arrow,
                    },
                    locked: false,
                };
                page.objects.push(new_conn);
                state.selected_ids = vec![new_id];
                state.rebuild_spatial_grid(&page.id, &page.objects);
            }
        } else if !state.lasso_points.is_empty() {
            if let Some(pos) = pointer_pos {
                let p = screen_to_page(pos);
                let seq = state.pointer_sample_seq;
                let release_sample = RawPointerSample {
                    screen_pos: pos,
                    page_pos: p,
                    raw_pressure: None,
                    effective_pressure: 1.0,
                    phase: PointerSamplePhase::Release,
                    source: PointerSource::Mouse,
                    sequence: seq,
                    dt_secs: 0.0,
                };
                if append_lasso_sample(
                    &mut state.raw_stroke_samples,
                    &mut state.lasso_points,
                    release_sample,
                ) {
                    state.pointer_sample_seq = seq + 1;
                }
            }
            state.last_completed_raw_samples = std::mem::take(&mut state.raw_stroke_samples);
            if state.lasso_points.len() > 2 {
                state.stroke_committed_this_frame = true;
                let mut selected = Vec::new();
                for obj in &page.objects {
                    if object_in_lasso_mode_with_context(
                        obj,
                        &state.lasso_points,
                        state.lasso_selection_mode,
                        Some(&page.objects),
                    ) {
                        selected.push(obj.id().to_string());
                    }
                }
                state.selected_ids = selected;
            }
            state.lasso_points.clear();
        }
        state.clear_active_draft_state();
    }
}

/// Menangani interaksi setelah rendering overlay: pintasan Quick Diagram, double-click, klik tunggal, dan shortcut kanvas.
pub(crate) fn handle_post_render_pointer_interaction(
    ui: &Ui,
    response: &Response,
    page: &mut Page,
    state: &mut CanvasState,
    updated_sel: &SelectionData,
    pointer_pos: Option<Pos2>,
    screen_to_page: impl Fn(Pos2) -> Pos2 + Copy,
) {
    // Task 9.6: Quick Diagramming via Keyboard (Tab: Right / Child, Enter: Bottom / Sibling)
    handle_quick_diagramming_shortcuts(ui, page, state, updated_sel);

    if response.double_clicked()
        && let Some(pos) = pointer_pos
    {
        handle_double_click(ui, page, state, updated_sel, pos, screen_to_page);
    }

    if response.clicked()
        && let Some(pos) = pointer_pos
    {
        handle_primary_click(ui, page, state, updated_sel, pos, screen_to_page);
    }

    handle_canvas_keyboard_shortcuts(ui, page, state);
}

/// Menangani klik ganda untuk menghapus control point konektor atau membuka editor teks/persamaan.
fn handle_double_click(
    ui: &Ui,
    page: &mut Page,
    state: &mut CanvasState,
    updated_sel: &SelectionData,
    pos: Pos2,
    screen_to_page: impl Fn(Pos2) -> Pos2 + Copy,
) {
    let page_p = screen_to_page(pos);

    // Double-click pada control point connector untuk menghapus titik kontrol (Task 8.3)
    if let Some((conn_id, Some(poly))) = &updated_sel.single_connector {
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

/// Menangani klik tunggal & Shift+klik sesuai alat aktif.
fn handle_primary_click(
    ui: &Ui,
    page: &mut Page,
    state: &mut CanvasState,
    updated_sel: &SelectionData,
    pos: Pos2,
    screen_to_page: impl Fn(Pos2) -> Pos2 + Copy,
) {
    if state.stroke_committed_this_frame {
        state.stroke_committed_this_frame = false;
        return;
    }
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
            page.objects
                .retain(|o| !matches!(o, DrawObject::Text { text, .. } if text.trim().is_empty()));
            state.clean_redundant_top_undo(&page.id, &page.objects);
            state.rebuild_spatial_grid(&page.id, &page.objects);
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
            state.rebuild_spatial_grid(&page.id, &page.objects);
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
        state.rebuild_spatial_grid(&page.id, &page.objects);
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
        let (touch_id, touch_force) =
            ui.input(|i| extract_touch_pressure(&i.raw.events, state.active_touch_id, Some(pos)));
        let raw_pressure = touch_force.unwrap_or(1.0).clamp(0.0, 1.0);
        let is_highlighter = state.current_tool == ActiveTool::Highlighter;
        let pressure = if (is_highlighter && !state.highlighter_pressure_enabled)
            || !state.pressure_sensitivity_enabled
        {
            1.0
        } else {
            state.effective_pressure(raw_pressure)
        };
        let single_sample = RawPointerSample {
            screen_pos: pos,
            page_pos: page_p,
            raw_pressure: touch_force,
            effective_pressure: pressure,
            phase: PointerSamplePhase::Release,
            source: touch_id.map_or(PointerSource::Mouse, |id| PointerSource::Touch {
                device_id: egui::TouchDeviceId(0),
                id,
            }),
            sequence: 0,
            dt_secs: 0.0,
        };
        let pts = state
            .draft_stroke
            .take()
            .unwrap_or_else(|| vec![single_sample.to_stroke_point()]);
        if state.raw_stroke_samples.is_empty() {
            state.last_completed_raw_samples = vec![single_sample];
        } else {
            state.last_completed_raw_samples = std::mem::take(&mut state.raw_stroke_samples);
        }
        state.clear_active_draft_state();
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
        state.rebuild_spatial_grid(&page.id, &page.objects);
        page.ocr_text = recognize_handwriting_from_strokes(&page.objects);
        ui.ctx().request_repaint();
    } else if matches!(
        state.current_tool,
        ActiveTool::Lasso | ActiveTool::Shape(_) | ActiveTool::Connector
    ) {
        if let Some((source_id, site)) = hit_test_quick_diagram_dot(updated_sel, state, page_p)
            && append_quick_diagram_connected_shape(
                page,
                state,
                &source_id,
                site,
                QuickDiagramPlacement::Directional,
            )
        {
            state.rebuild_spatial_grid(&page.id, &page.objects);
            ui.ctx().request_repaint();
            return;
        }
        let tol = 8.0 / state.zoom;
        if state.spatial_grid_page_id.as_deref() != Some(&page.id) {
            state.rebuild_spatial_grid(&page.id, &page.objects);
        }
        let hit = state
            .spatial_grid
            .hit_test_topmost(&page.objects, page_p, tol)
            .map(|(_, obj)| obj);
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

/// Menangani pintasan keyboard kanvas (`Escape`, `Ctrl+L`, `Enter`/`F2` untuk sunting teks inline).
fn handle_canvas_keyboard_shortcuts(ui: &Ui, page: &mut Page, state: &mut CanvasState) {
    // Escape key: membatalkan seleksi saat tidak sedang menyunting teks (Task 5.4)
    if ui.input(|i| i.key_pressed(Key::Escape)) && state.editing_text_id.is_none() {
        state.selected_ids.clear();
        state.clear_active_draft_state();
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
}

/// Mendeteksi jeda pena (~500 ms) di akhir goresan aktif untuk memicu pengenalan bentuk (*hold-to-snap*).
fn update_hold_to_snap(ui: &Ui, page: &Page, state: &mut CanvasState, now: Instant) {
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
}

/// Mode penempatan bentuk baru untuk aksi Quick Diagramming.
#[derive(Clone, Copy, Debug)]
enum QuickDiagramPlacement {
    Directional,
    AtPosition(Pos2),
}

/// Hasil hit-test titik tepi Quick Diagram dengan memprioritaskan handle transformasi jika lebih dekat.
pub(crate) fn hit_test_quick_diagram_dot(
    sel: &SelectionData,
    state: &CanvasState,
    page_p: Pos2,
) -> Option<(String, ConnectionSite)> {
    let (source_id, dots) = sel.single_shape_quick_dots.as_ref()?;
    let dot_hit_dist = 14.0 / state.zoom;
    let (site, best_dot_dist) = dots
        .iter()
        .filter_map(|&(site, dpos, _)| {
            let dist = page_p.distance(dpos);
            (dist <= dot_hit_dist).then_some((site, dist))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))?;

    // Hindari menangkap handle resize tepi atau rotasi ketika kursor lebih dekat ke handle tersebut
    if matches!(state.current_tool, ActiveTool::Lasso | ActiveTool::Shape(_))
        && !sel.is_selection_fully_locked
        && let Some(frame) = &sel.frame
    {
        let handle_dist = 14.0 / state.zoom;
        let nearest_handle_dist = frame
            .handles
            .iter()
            .filter_map(|h| {
                let dist = page_p.distance(h.pos.to_pos2());
                (dist <= handle_dist).then_some(dist)
            })
            .min_by(|a, b| a.total_cmp(b));
        if let Some(h_dist) = nearest_handle_dist
            && h_dist <= best_dot_dist
        {
            return None;
        }
    }

    Some((source_id.clone(), site))
}

/// Menyelesaikan pelepasan gesture drag Quick Diagram Dot (tap -> bentuk baru, drag ke target -> konektor, drag ke kosong -> bentuk di posisi lepas).
pub(crate) fn complete_quick_diagram_dot_interaction(
    page: &mut Page,
    state: &mut CanvasState,
    site: ConnectionSite,
    drag_start_pos: Option<Pos2>,
    stop_page_p: Pos2,
) {
    let drag_dist = drag_start_pos.map_or(0.0, |p| (stop_page_p - p).length());
    if let Some(ConnectorEnd::Attached {
        shape_id: source_id,
        ..
    }) = state.draft_connector_start.take()
    {
        if drag_dist <= 6.0 {
            append_quick_diagram_connected_shape(
                page,
                state,
                &source_id,
                site,
                QuickDiagramPlacement::Directional,
            );
        } else {
            let snap_target = state.connector_snap_candidate.take().or_else(|| {
                find_connector_snap_site(
                    &page.objects,
                    Point2D::new(stop_page_p.x, stop_page_p.y),
                    24.0 / state.zoom,
                    Some(source_id.as_str()),
                )
            });
            if let Some((target_id, target_site, _)) = snap_target {
                // Task 9.3: Lepas pada bentuk lain -> sambungkan konektor
                state.push_undo(&page.id, &page.objects);
                let new_conn = make_quick_diagram_connector(
                    ConnectorEnd::Attached {
                        shape_id: source_id,
                        site,
                    },
                    ConnectorEnd::Attached {
                        shape_id: target_id,
                        site: target_site,
                    },
                );
                page.objects.push(new_conn);
            } else {
                // Task 9.3: Lepas di ruang kosong -> buat bentuk baru di sana
                append_quick_diagram_connected_shape(
                    page,
                    state,
                    &source_id,
                    site,
                    QuickDiagramPlacement::AtPosition(stop_page_p),
                );
            }
        }
    }
    state.clear_active_draft_state();
}

/// Membuat connector standar untuk fitur Quick Diagramming.
fn make_quick_diagram_connector(start: ConnectorEnd, end: ConnectorEnd) -> DrawObject {
    DrawObject::Connector {
        id: Uuid::new_v4().to_string(),
        start,
        end,
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
    }
}

/// Menambahkan bentuk baru yang terhubung ke bentuk sumber beserta connector penghubungnya.
fn append_quick_diagram_connected_shape(
    page: &mut Page,
    state: &mut CanvasState,
    source_id: &str,
    site: ConnectionSite,
    placement: QuickDiagramPlacement,
) -> bool {
    let Some((sx, sy, sw, sh, shape_type, color, stroke_width, fill, corner_radius)) =
        page.objects.iter().find_map(|obj| match obj {
            DrawObject::Shape {
                id,
                x,
                y,
                width,
                height,
                shape_type,
                color,
                stroke_width,
                fill,
                corner_radius,
                ..
            } if id == source_id => Some((
                *x,
                *y,
                *width,
                *height,
                *shape_type,
                color.clone(),
                *stroke_width,
                fill.clone(),
                *corner_radius,
            )),
            _ => None,
        })
    else {
        return false;
    };

    state.push_undo(&page.id, &page.objects);
    let (nx, ny, nw, nh, end_site) = match placement {
        QuickDiagramPlacement::Directional => next_shape_rect(
            (sx, sy, sw, sh),
            site,
            60.0,
            &page.objects,
            (page.width, page.height),
        ),
        QuickDiagramPlacement::AtPosition(stop_page_p) => {
            let margin = 16.0_f32;
            let nw = sw.abs();
            let nh = sh.abs();
            let nx =
                (stop_page_p.x - nw * 0.5).clamp(margin, (page.width - nw - margin).max(margin));
            let ny =
                (stop_page_p.y - nh * 0.5).clamp(margin, (page.height - nh - margin).max(margin));
            (nx, ny, nw, nh, ConnectionSite::Auto)
        }
    };

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
    let new_conn = make_quick_diagram_connector(
        ConnectorEnd::Attached {
            shape_id: source_id.to_string(),
            site,
        },
        ConnectorEnd::Attached {
            shape_id: new_shape_id.clone(),
            site: end_site,
        },
    );
    page.objects.push(new_shape);
    page.objects.push(new_conn);
    state.selected_ids = vec![new_shape_id.clone()];
    state.editing_text_id = Some(new_shape_id);
    state.focus_text_edit = true;
    true
}

/// Menangani pintasan keyboard Quick Diagramming (`Tab` untuk child kanan, `Enter` untuk sibling bawah).
fn handle_quick_diagramming_shortcuts(
    ui: &Ui,
    page: &mut Page,
    state: &mut CanvasState,
    updated_sel: &SelectionData,
) {
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
            if append_quick_diagram_connected_shape(
                page,
                state,
                source_id,
                target_site,
                QuickDiagramPlacement::Directional,
            ) {
                state.rebuild_spatial_grid(&page.id, &page.objects);
                ui.ctx().request_repaint();
            }
        }
    }
}

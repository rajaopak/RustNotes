use super::laser::{smooth_laser_points, tessellate_laser_layer};
use super::primitives::{
    draw_non_stroke_object, draw_paper_pattern, draw_rotated_background_image,
    draw_shape_primitive, transform_page_mesh_to_screen,
};
use super::state::CanvasState;
use super::types::{ActiveTool, EraserMode, ResizeCorner};
use crate::geometry::{
    combined_bounds, hit_test_object, object_bounds, object_in_lasso,
    recognize_handwriting_from_strokes, recognize_shape, scale_object, should_reject_palm_input,
    smooth_stroke_points, tessellate_stroke_ex, translate_object,
};
use crate::model::{DrawObject, Page, ShapeKind, StrokePoint};
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
            if let Some(tex) = state.texture_cache.get(&asset_id) {
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
                    page_to_screen,
                    state.zoom,
                    &state.texture_cache,
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
    let selected_objects: Vec<&DrawObject> = page
        .objects
        .iter()
        .filter(|o| state.selected_ids.contains(&o.id().to_string()))
        .collect();

    let selection_rect = combined_bounds(&selected_objects);
    let selected_single_line: Option<(Pos2, Pos2)> = if selected_objects.len() == 1 {
        if let DrawObject::Shape {
            shape_type: ShapeKind::Line,
            x,
            y,
            width,
            height,
            ..
        } = selected_objects[0]
        {
            Some((Pos2::new(*x, *y), Pos2::new(*x + *width, *y + *height)))
        } else {
            None
        }
    } else {
        None
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
        } else if state.current_tool == ActiveTool::Lasso
            && let Some(b) = selection_rect
        {
            let handle_dist = 14.0 / state.zoom;
            // Check rotation handle first (circle above selection center)
            let rot_handle = Pos2::new(b.center().x, b.top() - 18.0 / state.zoom);
            if (page_p - rot_handle).length() <= handle_dist {
                state.resize_corner = Some(ResizeCorner::Rotation);
                state.drag_start_pos = Some(page_p);
                state.push_undo(&page.id, &page.objects);
                return response;
            }
            if (page_p - b.right_bottom()).length() <= handle_dist {
                state.resize_corner = Some(ResizeCorner::SouthEast);
                state.drag_start_pos = Some(page_p);
                state.push_undo(&page.id, &page.objects);
                return response;
            }
            if (page_p - b.left_top()).length() <= handle_dist {
                state.resize_corner = Some(ResizeCorner::NorthWest);
                state.drag_start_pos = Some(page_p);
                state.push_undo(&page.id, &page.objects);
                return response;
            }
            if (page_p - Pos2::new(b.right(), b.top())).length() <= handle_dist {
                state.resize_corner = Some(ResizeCorner::NorthEast);
                state.drag_start_pos = Some(page_p);
                state.push_undo(&page.id, &page.objects);
                return response;
            }
            if (page_p - Pos2::new(b.left(), b.bottom())).length() <= handle_dist {
                state.resize_corner = Some(ResizeCorner::SouthWest);
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
                state.draft_shape_start = Some(page_p);
                state.draft_shape_current = Some(page_p);
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
                let dx = page_p.x - start.x;
                let dy = page_p.y - start.y;
                for obj in &mut page.objects {
                    if state.selected_ids.contains(&obj.id().to_string()) {
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
                    let angle = v_curr.y.atan2(v_curr.x) - v_start.y.atan2(v_start.x);
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
                } else {
                    let origin = match corner {
                        ResizeCorner::SouthEast => b.left_top(),
                        ResizeCorner::NorthWest => b.right_bottom(),
                        ResizeCorner::NorthEast => b.left_bottom(),
                        ResizeCorner::SouthWest => b.right_top(),
                        ResizeCorner::Rotation
                        | ResizeCorner::LineStart
                        | ResizeCorner::LineEnd => b.center(),
                    };
                    let orig_w = b.width().max(1.0);
                    let orig_h = b.height().max(1.0);
                    let mut sx = ((page_p.x - origin.x).abs() / orig_w).max(0.05);
                    let mut sy = ((page_p.y - origin.y).abs() / orig_h).max(0.05);
                    // Shift+drag: kunci rasio aspek untuk proportional resize
                    if ui.input(|i| i.modifiers.shift) {
                        let s = if (sx - 1.0).abs() > (sy - 1.0).abs() {
                            sx
                        } else {
                            sy
                        };
                        sx = s;
                        sy = s;
                    }
                    for obj in &mut page.objects {
                        if state.selected_ids.contains(&obj.id().to_string()) {
                            scale_object(obj, origin, sx, sy);
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
                            if recognized.kind == ShapeKind::Line {
                                // Sama seperti tools Shape -> Line: titik awal (recognized.x, recognized.y)
                                // menjadi pusat rotasi (pivot), dan titik akhirnya berotasi mengikuti kursor!
                                recognized.width = page_p.x - recognized.x;
                                recognized.height = page_p.y - recognized.y;
                                ui.ctx().request_repaint();
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
                        state.draft_shape_current = Some(page_p);
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

    // Fase 8.1: Deteksi tahan pena (~500 ms) di akhir goresan untuk Shape Recognition
    if state.active_stroke_page_id.as_deref() == Some(&page.id)
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
                state.recognized_draft_shape = Some(recognized);
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
            state.spatial_grid.rebuild(&page.objects);
        } else if state.resize_corner.is_some() {
            state.resize_corner = None;
            state.drag_start_pos = None;
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
                    // Fase 8.1: Shape Recognition — jika sudah ditahan ~500ms dan dikenali sebagai Shape
                    if let Some(recognized) = recognized_shape {
                        let new_id = Uuid::new_v4().to_string();
                        let is_line = recognized.kind == ShapeKind::Line;
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
                    page.objects.push(DrawObject::Shape {
                        id: new_id.clone(),
                        shape_type: kind,
                        x: start.x,
                        y: start.y,
                        width: curr.x - start.x,
                        height: curr.y - start.y,
                        color: state.stroke_color.into(),
                        stroke_width: state.stroke_width,
                        fill: None,
                    });
                    if kind == ShapeKind::Line {
                        state.selected_ids = vec![new_id];
                    }
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
            draw_shape_primitive(
                &painter,
                recognized.kind,
                recognized.x,
                recognized.y,
                recognized.width,
                recognized.height,
                Color32::TRANSPARENT,
                stroke,
                page_to_screen,
            );
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
                draw_shape_primitive(
                    &painter,
                    kind,
                    start.x,
                    start.y,
                    curr.x - start.x,
                    curr.y - start.y,
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
    let updated_selected_objects: Vec<&DrawObject> = page
        .objects
        .iter()
        .filter(|o| state.selected_ids.contains(&o.id().to_string()))
        .collect();
    let updated_selection_rect = combined_bounds(&updated_selected_objects);
    let updated_single_line: Option<(Pos2, Pos2)> = if updated_selected_objects.len() == 1 {
        if let DrawObject::Shape {
            shape_type: ShapeKind::Line,
            x,
            y,
            width,
            height,
            ..
        } = updated_selected_objects[0]
        {
            Some((Pos2::new(*x, *y), Pos2::new(*x + *width, *y + *height)))
        } else {
            None
        }
    } else {
        None
    };

    if let Some((line_start, line_end)) = updated_single_line {
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
        let screen_box = Rect::from_min_max(page_to_screen(b.min), page_to_screen(b.max));
        painter.rect_stroke(
            screen_box.expand(4.0),
            2.0,
            Stroke::new(1.5_f32, Color32::from_rgb(0, 114, 206)),
            egui::StrokeKind::Outside,
        );

        // Corner handles
        let handle_stroke = Stroke::new(2.0_f32, Color32::from_rgb(0, 114, 206));
        painter.circle(
            screen_box.right_bottom(),
            6.0,
            Color32::WHITE,
            handle_stroke,
        );
        painter.circle(screen_box.left_top(), 6.0, Color32::WHITE, handle_stroke);
        painter.circle(screen_box.right_top(), 6.0, Color32::WHITE, handle_stroke);
        painter.circle(screen_box.left_bottom(), 6.0, Color32::WHITE, handle_stroke);

        // Rotation stem & handle (with ⟳ symbol)
        let stem_top = Pos2::new(screen_box.center().x, screen_box.top() - 18.0);
        painter.line_segment(
            [
                Pos2::new(screen_box.center().x, screen_box.top() - 4.0),
                stem_top,
            ],
            handle_stroke,
        );
        painter.circle(stem_top, 7.0, Color32::WHITE, handle_stroke);
        painter.text(
            stem_top,
            egui::Align2::CENTER_CENTER,
            "⟳",
            egui::FontId::proportional(11.0),
            Color32::from_rgb(0, 114, 206),
        );
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

    // Double-click pada objek Teks atau StickyNote untuk menyunting isinya secara langsung
    if response.double_clicked()
        && let Some(pos) = pointer_pos
    {
        let page_p = screen_to_page(pos);
        if let Some(obj) = page.objects.iter().rev().find(|obj| {
            matches!(obj, DrawObject::Text { .. } | DrawObject::StickyNote { .. })
                && hit_test_object(obj, page_p, 8.0 / state.zoom)
        }) {
            let id = obj.id().to_string();
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
        } else if state.current_tool == ActiveTool::Lasso {
            let hit = page
                .objects
                .iter()
                .rev()
                .find(|obj| hit_test_object(obj, page_p, 8.0 / state.zoom));
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

    // 9. Inline Text / StickyNote Editor langsung di atas kanvas halaman
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
                _ => {
                    close_editor = true;
                }
            }
        }
        if close_editor {
            state.editing_text_id = None;
            page.objects
                .retain(|o| !matches!(o, DrawObject::Text { text, .. } if text.trim().is_empty()));
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
                    ui.close_menu();
                }
                ui.separator();
            }
            if ui.button("🖼 Tempel Gambar dari Clipboard").clicked() {
                state.request_paste_image = Some(click_page_p);
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

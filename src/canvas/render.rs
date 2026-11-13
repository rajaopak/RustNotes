use super::gizmo::{SelectionData, render_alignment_guides, render_selection_gizmo};
pub(crate) use super::interaction::extract_touch_pressure;
use super::interaction::{
    PointerInteractionOutcome, handle_pointer_drag_interaction,
    handle_post_render_pointer_interaction,
};
use super::laser::render_laser_trail;
use super::primitives::{
    draw_non_stroke_object, draw_paper_pattern, draw_rotated_background_image,
    draw_shape_primitive, transform_page_mesh_to_screen,
};
use super::state::CanvasState;
use super::types::{ActiveTool, ResizeCorner};
pub use super::widgets::FloatingToolbarStyle;
use super::widgets::{render_equation_modal_dialog, render_inline_text_editor};
use super::zoom_window::render_zoom_target_indicator;
use crate::geometry::{
    object_bounds, object_bounds_with_context, shape_connection_point, smooth_stroke_points,
    tessellate_stroke_styled,
};
use crate::model::{ConnectionSite, ConnectorEnd, DrawObject, Page, Point2D, ShapeKind};
use eframe::egui::{self, Color32, Pos2, Rect, Response, Sense, Stroke, Ui, Vec2};
use std::time::Instant;

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

    // 1 & 2. Draw Paper Background, Patterns, and Two-Pass Page Objects
    render_page_background_and_objects(
        &painter,
        rect,
        screen_clip_rect,
        origin,
        page,
        state,
        page_to_screen,
    );

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

    if handle_pointer_drag_interaction(
        ui,
        &response,
        page,
        state,
        &sel,
        pointer_pos,
        now,
        screen_to_page,
    ) == PointerInteractionOutcome::EarlyReturn
    {
        return response;
    }

    // 4 & 5. Render Active Draft Stroke/Shape, Connector Snap Sites, and Active Lasso Loop
    render_active_draft_and_lasso(ui, &painter, page, state, now, page_to_screen);

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

    handle_post_render_pointer_interaction(
        ui,
        &response,
        page,
        state,
        &updated_sel,
        pointer_pos,
        screen_to_page,
    );

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

    render_canvas_context_menu(
        &response,
        page,
        state,
        click_page_p,
        has_selection_on_this_page,
    );

    response
}

/// Merender latar belakang kertas/aset dan seluruh objek halaman dengan two-pass layering & mesh cache.
fn render_page_background_and_objects(
    painter: &egui::Painter,
    rect: Rect,
    screen_clip_rect: Rect,
    origin: Pos2,
    page: &Page,
    state: &mut CanvasState,
    page_to_screen: impl Fn(Pos2) -> Pos2 + Copy,
) {
    // 1. Draw Paper Background and Patterns
    painter.rect_filled(rect, 4.0, page.paper_color.to_color32());

    match page.effective_background() {
        crate::model::PageBackground::Template(template) => {
            draw_paper_pattern(painter, rect, template, state.zoom);
        }
        crate::model::PageBackground::Image { asset_id }
        | crate::model::PageBackground::Pdf { asset_id, .. } => {
            if let Some(tex) = state.get_texture(&asset_id) {
                draw_rotated_background_image(painter, tex.id(), rect, page.rotation);
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
                    let style_rev: u32 = ((state.smoothing_steps as u32) << 8)
                        | if *is_highlighter { hl_alpha as u32 } else { 0 };
                    let key = (id.clone(), style_rev);
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
                    let obj_b = object_bounds_with_context(obj, &page.objects);
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
                        painter,
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
}

/// Merender pratinjau goresan aktif, bentuk hasil pengenalan, draft shape, konektor, dan loop lasso.
fn render_active_draft_and_lasso(
    ui: &Ui,
    painter: &egui::Painter,
    page: &Page,
    state: &CanvasState,
    now: Instant,
    page_to_screen: impl Fn(Pos2) -> Pos2 + Copy,
) {
    // 4. Render Active Draft Stroke / Shape with live smooth tessellation
    if state.active_stroke_page_id.as_deref() == Some(&page.id) {
        if let Some(recognized) = &state.recognized_draft_shape {
            let stroke = Stroke::new(state.stroke_width * state.zoom, state.stroke_color);
            let corner_radius = 0.0;
            draw_shape_primitive(
                painter,
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
                        painter,
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
                    painter,
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
}

/// Merender menu konteks klik-kanan pada kanvas halaman (aksi seleksi atau tempel kanvas kosong).
fn render_canvas_context_menu(
    response: &Response,
    page: &mut Page,
    state: &mut CanvasState,
    click_page_p: Pos2,
    has_selection_on_this_page: bool,
) {
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
}

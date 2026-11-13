use super::primitives::{draw_non_stroke_object, draw_paper_pattern};
use super::state::CanvasState;
use super::types::ActiveTool;
use crate::geometry::{
    object_bounds, recognize_handwriting_from_strokes, should_reject_palm_input,
    smooth_stroke_points, tessellate_stroke_styled,
};
use crate::model::{DrawObject, Page, StrokePoint};
use eframe::egui::{self, Color32, PointerButton, Pos2, Rect, Sense, Stroke, Ui, Vec2};
use uuid::Uuid;

pub(super) fn zoom_stroke_width(state: &CanvasState) -> f32 {
    if state.current_tool == ActiveTool::Highlighter {
        state.stroke_width * 6.0
    } else if state.current_tool == ActiveTool::Pencil {
        (state.stroke_width - 0.5).max(1.0)
    } else {
        state.stroke_width
    }
}

/// Fase 8.3: Render kanvas Zoom Window (panel menulis detail halus dengan auto-scroll ke kanan).
/// Mengembalikan `true` jika ada perubahan objek pada halaman.
pub fn render_zoom_window_canvas(
    ui: &mut Ui,
    page: &mut Page,
    state: &mut CanvasState,
    scale: f32,
) -> bool {
    let target = state.zoom_target_rect;
    let desired_size = Vec2::new(target.width() * scale, target.height() * scale);
    let (zw_rect, response) = ui.allocate_exact_size(desired_size, Sense::click_and_drag());
    let painter = ui.painter_at(zw_rect);

    let zw_origin = zw_rect.min;
    let target_min = target.min;

    let zw_to_page = move |screen_pos: Pos2| -> Pos2 {
        let rel = screen_pos - zw_origin;
        Pos2::new(target_min.x + rel.x / scale, target_min.y + rel.y / scale)
    };
    let page_to_zw = move |page_pos: Pos2| -> Pos2 {
        let rel = page_pos - target_min;
        zw_origin + Vec2::new(rel.x * scale, rel.y * scale)
    };

    // 1. Latar kertas & pola halaman ter-skala
    painter.rect_filled(zw_rect, 4.0, page.paper_color.to_color32());
    let full_page_zw_rect = Rect::from_min_size(
        page_to_zw(Pos2::ZERO),
        Vec2::new(page.width * scale, page.height * scale),
    );
    if let crate::model::PageBackground::Template(tmpl) = page.effective_background() {
        draw_paper_pattern(&painter, full_page_zw_rect, tmpl, scale);
    }

    // 2. Zona Auto-Advance di 25% sisi kanan Zoom Window
    let adv_screen_left = zw_rect.right() - zw_rect.width() * 0.25;
    let adv_rect = Rect::from_min_max(
        Pos2::new(adv_screen_left, zw_rect.top()),
        zw_rect.right_bottom(),
    );
    painter.rect_filled(
        adv_rect,
        0.0,
        Color32::from_rgba_unmultiplied(0, 114, 206, 22),
    );
    painter.line_segment(
        [
            Pos2::new(adv_screen_left, zw_rect.top()),
            Pos2::new(adv_screen_left, zw_rect.bottom()),
        ],
        Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(0, 114, 206, 90)),
    );

    // 3. Render objek halaman yang tampak di dalam kotak target dengan Two-Pass Layering
    // Pass 1: Highlighters (underlay layer)
    // Pass 2: Content layer (pen strokes, shapes, connectors, images, sticky notes, text)
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
                    points,
                    color,
                    width,
                    is_highlighter,
                    style,
                    ..
                } => {
                    let b = object_bounds(obj);
                    if !b.intersects(target.expand(20.0)) {
                        continue;
                    }
                    let smoothed = smooth_stroke_points(points, state.smoothing_steps);
                    let stroke_w = width * scale;
                    let stroke_c = if *is_highlighter {
                        Color32::from_rgba_unmultiplied(color.r, color.g, color.b, hl_alpha)
                    } else {
                        color.to_color32()
                    };
                    let mesh = tessellate_stroke_styled(
                        &smoothed,
                        stroke_w,
                        stroke_c,
                        *is_highlighter,
                        *style,
                        page_to_zw,
                    );
                    painter.add(mesh);
                }
                _ if pass_is_highlighter => {
                    continue;
                }
                other => {
                    let b = object_bounds(obj);
                    if !b.intersects(target.expand(20.0)) {
                        continue;
                    }
                    draw_non_stroke_object(
                        &painter,
                        other,
                        &page.objects,
                        page_to_zw,
                        scale,
                        |id| state.get_texture(id).map(|t| t.id()),
                    );
                }
            }
        }
    }

    // 4. Tangani input menulis / menghapus di dalam Zoom Window
    let mut modified = false;
    let pointer_pos = response.interact_pointer_pos();
    let adv_page_x = target.right() - target.width() * 0.25;

    if response.drag_started_by(PointerButton::Primary)
        && let Some(pos) = pointer_pos
    {
        let page_p = zw_to_page(pos);
        match state.current_tool {
            ActiveTool::Eraser => {
                state.eraser_undo_pushed = false;
                if state.erase_at(page, page_p, 12.0 / scale * state.zoom) {
                    modified = true;
                }
            }
            _ => {
                let (touch_id, touch_force) = ui.input(|i| {
                    super::render::extract_touch_pressure(&i.raw.events, None, Some(pos))
                });
                state.active_touch_id = touch_id;
                let raw_pressure = touch_force.unwrap_or(1.0).clamp(0.0, 1.0);
                let is_highlighter = state.current_tool == ActiveTool::Highlighter;
                let pressure = if is_highlighter && !state.highlighter_pressure_enabled {
                    1.0
                } else {
                    state.effective_pressure(raw_pressure)
                };
                state.zoom_draft_stroke =
                    Some(vec![StrokePoint::new(page_p.x, page_p.y, pressure)]);
                state.last_stroke_move_time = Some(std::time::Instant::now());
                state.zoom_wrote_in_advance_zone = page_p.x >= adv_page_x;
            }
        }
    }

    if response.dragged_by(PointerButton::Primary)
        && let Some(pos) = pointer_pos
    {
        let page_p = zw_to_page(pos);
        match state.current_tool {
            ActiveTool::Eraser => {
                if state.erase_at(page, page_p, 12.0 / scale * state.zoom) {
                    modified = true;
                }
            }
            _ => {
                if state.zoom_draft_stroke.is_some() {
                    let now = std::time::Instant::now();
                    let dt_secs = state
                        .last_stroke_move_time
                        .map(|t| now.saturating_duration_since(t).as_secs_f32())
                        .unwrap_or(0.016);
                    let multi_touch = ui.input(|i| i.multi_touch().is_some());
                    let min_dist = 1.5 / scale;
                    let should_push = state.zoom_draft_stroke.as_ref().is_some_and(|pts| {
                        let rejected = state.palm_rejection_enabled
                            && should_reject_palm_input(pts, page_p, dt_secs, multi_touch);
                        !rejected
                            && pts
                                .last()
                                .is_none_or(|last| (page_p - last.to_pos2()).length() >= min_dist)
                    });
                    if should_push {
                        let (tid, touch_force) = ui.input(|i| {
                            super::render::extract_touch_pressure(
                                &i.raw.events,
                                state.active_touch_id,
                                Some(pos),
                            )
                        });
                        if state.active_touch_id.is_none() {
                            state.active_touch_id = tid;
                        }
                        let is_highlighter = state.current_tool == ActiveTool::Highlighter;
                        let pressure = if (is_highlighter && !state.highlighter_pressure_enabled)
                            || !state.pressure_sensitivity_enabled
                        {
                            1.0
                        } else if let Some(f) = touch_force {
                            state.effective_pressure(f.clamp(0.0, 1.0))
                        } else {
                            state
                                .zoom_draft_stroke
                                .as_ref()
                                .and_then(|pts| pts.last().map(|p| p.pressure))
                                .unwrap_or(1.0)
                        };
                        if let Some(pts) = &mut state.zoom_draft_stroke {
                            pts.push(StrokePoint::new(page_p.x, page_p.y, pressure));
                        }
                        state.last_stroke_move_time = Some(now);
                        if page_p.x >= adv_page_x {
                            state.zoom_wrote_in_advance_zone = true;
                        }
                    }
                }
            }
        }
    }

    if response.drag_stopped() {
        state.eraser_undo_pushed = false;
        state.last_whole_erase_pos = None;
        state.active_touch_id = None;
        state.last_stroke_move_time = None;
        if let Some(mut pts) = state.zoom_draft_stroke.take() {
            if let Some(pos) = pointer_pos {
                let page_p = zw_to_page(pos);
                if page_p.x >= adv_page_x {
                    state.zoom_wrote_in_advance_zone = true;
                }
                let should_append = pts
                    .last()
                    .is_none_or(|last| (page_p - last.to_pos2()).length() >= 0.5);
                if should_append {
                    let pressure = pts.last().map(|pt| pt.pressure).unwrap_or(1.0);
                    pts.push(StrokePoint {
                        x: page_p.x,
                        y: page_p.y,
                        pressure,
                    });
                }
            }
            if !pts.is_empty() {
                state.push_undo(&page.id, &page.objects);
                let is_highlighter = state.current_tool == ActiveTool::Highlighter;
                let stroke_width = zoom_stroke_width(state);
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
                // Fase 8.6: Perbarui indeks OCR tulisan tangan otomatis saat menulis di Zoom Window
                page.ocr_text = recognize_handwriting_from_strokes(&page.objects);
                modified = true;

                // Auto-advance ke kanan jika pengguna menulis di zona kanan
                if state.zoom_wrote_in_advance_zone {
                    state.advance_zoom_target(page.width, page.height);
                    state.zoom_wrote_in_advance_zone = false;
                }
            }
        }
    }

    // 5. Render draft stroke yang sedang ditulis di Zoom Window
    if let Some(pts) = &state.zoom_draft_stroke {
        let is_highlighter = state.current_tool == ActiveTool::Highlighter;
        let stroke_w = zoom_stroke_width(state) * scale;
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
            stroke_w,
            color,
            is_highlighter,
            state.stroke_style,
            page_to_zw,
        );
        painter.add(mesh);
    }

    painter.rect_stroke(
        zw_rect,
        4.0,
        Stroke::new(2.0_f32, Color32::from_rgb(0, 114, 206)),
        egui::StrokeKind::Outside,
    );

    modified
}

/// Render Indikator Kotak Target Zoom Window pada Kanvas Utama
pub fn render_zoom_target_indicator(
    painter: &egui::Painter,
    state: &CanvasState,
    page_to_screen: impl Fn(Pos2) -> Pos2,
) {
    if !state.show_zoom_window {
        return;
    }
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

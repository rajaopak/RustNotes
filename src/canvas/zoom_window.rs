use super::primitives::{draw_non_stroke_object, draw_paper_pattern};
use super::state::CanvasState;
use super::types::ActiveTool;
use crate::geometry::{
    object_bounds, recognize_handwriting_from_strokes, smooth_stroke_points, tessellate_stroke_ex,
};
use crate::model::{DrawObject, Page, StrokePoint};
use eframe::egui::{self, Color32, PointerButton, Pos2, Rect, Sense, Stroke, Ui, Vec2};
use uuid::Uuid;

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

    // 3. Render objek halaman yang tampak di dalam kotak target
    for obj in &page.objects {
        let b = object_bounds(obj);
        if !b.intersects(target.expand(20.0)) {
            continue;
        }
        match obj {
            DrawObject::Stroke {
                points,
                color,
                width,
                is_highlighter,
                ..
            } => {
                let smoothed = smooth_stroke_points(points, state.smoothing_steps);
                let stroke_w = width * scale;
                let stroke_c = if *is_highlighter {
                    Color32::from_rgba_unmultiplied(color.r, color.g, color.b, 90)
                } else {
                    color.to_color32()
                };
                let mesh = tessellate_stroke_ex(
                    &smoothed,
                    stroke_w,
                    stroke_c,
                    *is_highlighter,
                    page_to_zw,
                );
                painter.add(mesh);
            }
            other => {
                draw_non_stroke_object(&painter, other, page_to_zw, scale, &state.texture_cache);
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
                state.zoom_draft_stroke = Some(vec![StrokePoint::from(page_p)]);
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
                if let Some(pts) = &mut state.zoom_draft_stroke {
                    let min_dist = 1.5 / scale;
                    if pts
                        .last()
                        .is_none_or(|last| (page_p - last.to_pos2()).length() >= min_dist)
                    {
                        pts.push(StrokePoint::from(page_p));
                    }
                    if page_p.x >= adv_page_x {
                        state.zoom_wrote_in_advance_zone = true;
                    }
                }
            }
        }
    }

    if response.drag_stopped() {
        state.eraser_undo_pushed = false;
        state.last_whole_erase_pos = None;
        if let Some(mut pts) = state.zoom_draft_stroke.take() {
            if let Some(pos) = pointer_pos {
                let page_p = zw_to_page(pos);
                if page_p.x >= adv_page_x {
                    state.zoom_wrote_in_advance_zone = true;
                }
                pts.push(StrokePoint::from(page_p));
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
                page.objects.push(DrawObject::Stroke {
                    id: Uuid::new_v4().to_string(),
                    points: pts,
                    color: color.into(),
                    width: stroke_width,
                    is_highlighter,
                });
                state.spatial_grid.rebuild(&page.objects);
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
        let stroke_w = if is_highlighter {
            state.stroke_width * 6.0 * scale
        } else {
            state.stroke_width * scale
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
        let mesh = tessellate_stroke_ex(&smoothed, stroke_w, color, is_highlighter, page_to_zw);
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

use crate::model::{DrawObject, PageTemplate, ShapeKind};
use eframe::egui::{self, Color32, Mesh, Painter, Pos2, Rect, Stroke, Vec2};
use std::collections::HashMap;

/// Transformasikan `Mesh` goresan yang di-cache dalam koordinat lokal halaman (`0..width`, `0..height`)
/// ke koordinat layar terkini berdasarkan `origin` kertas (`rect.min`) dan faktor `zoom`.
/// Dengan cara ini, goresan selalu menempel presisi di atas kertas saat di-scroll maupun di-zoom.
pub fn transform_page_mesh_to_screen(page_mesh: &Mesh, origin: Pos2, zoom: f32) -> Mesh {
    let mut screen_mesh = page_mesh.clone();
    for v in &mut screen_mesh.vertices {
        v.pos = Pos2::new(origin.x + v.pos.x * zoom, origin.y + v.pos.y * zoom);
    }
    screen_mesh
}

pub fn draw_paper_pattern(painter: &Painter, rect: Rect, template: PageTemplate, zoom: f32) {
    let spacing = 30.0 * zoom;
    let line_color = Color32::from_rgb(220, 226, 231);

    match template {
        PageTemplate::Blank => {}
        PageTemplate::Ruled => {
            let mut y = rect.top() + spacing;
            while y < rect.bottom() {
                painter.line_segment(
                    [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
                    Stroke::new(0.8_f32, line_color),
                );
                y += spacing;
            }
        }
        PageTemplate::Grid => {
            let mut y = rect.top() + spacing;
            while y < rect.bottom() {
                painter.line_segment(
                    [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
                    Stroke::new(0.8_f32, line_color),
                );
                y += spacing;
            }
            let mut x = rect.left() + spacing;
            while x < rect.right() {
                painter.line_segment(
                    [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
                    Stroke::new(0.8_f32, line_color),
                );
                x += spacing;
            }
        }
        PageTemplate::Dotted => {
            let mut y = rect.top() + spacing;
            while y < rect.bottom() {
                let mut x = rect.left() + spacing;
                while x < rect.right() {
                    painter.circle_filled(Pos2::new(x, y), 1.0, Color32::from_rgb(204, 211, 218));
                    x += spacing;
                }
                y += spacing;
            }
        }
        PageTemplate::Cornell => {
            let line_color = Color32::from_rgb(200, 210, 220);
            let summary_y = rect.bottom() - rect.height() * 0.16;
            let cue_x = rect.left() + rect.width() * 0.28;

            // Ruled lines in main area
            let mut y = rect.top() + spacing * 1.5;
            while y < summary_y {
                painter.line_segment(
                    [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
                    Stroke::new(0.8_f32, Color32::from_rgb(230, 235, 240)),
                );
                y += spacing;
            }

            // Cue vertical line
            painter.line_segment(
                [
                    Pos2::new(cue_x, rect.top() + rect.height() * 0.04),
                    Pos2::new(cue_x, summary_y),
                ],
                Stroke::new(1.5_f32, line_color),
            );

            // Summary horizontal line
            painter.line_segment(
                [
                    Pos2::new(rect.left(), summary_y),
                    Pos2::new(rect.right(), summary_y),
                ],
                Stroke::new(1.5_f32, line_color),
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn draw_shape_primitive(
    painter: &Painter,
    shape_type: ShapeKind,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    fill_color: Color32,
    stroke: Stroke,
    page_to_screen: impl Fn(Pos2) -> Pos2,
) {
    let p1 = page_to_screen(Pos2::new(x, y));
    let p2 = page_to_screen(Pos2::new(x + width, y + height));
    match shape_type {
        ShapeKind::Line => {
            painter.line_segment([p1, p2], stroke);
        }
        ShapeKind::Rectangle => {
            let r = Rect::from_two_pos(p1, p2);
            painter.rect(r, 0.0, fill_color, stroke, egui::StrokeKind::Outside);
        }
        ShapeKind::Ellipse => {
            let r = Rect::from_two_pos(p1, p2);
            let radius = (r.width().min(r.height()) / 2.0).max(1.0);
            painter.circle(r.center(), radius, fill_color, stroke);
        }
        ShapeKind::Triangle
        | ShapeKind::RightTriangle
        | ShapeKind::ScaleneTriangle { .. }
        | ShapeKind::Parallelogram => {
            let verts: Vec<Pos2> = crate::geometry::shape_vertices(shape_type, x, y, width, height)
                .into_iter()
                .map(&page_to_screen)
                .collect();
            if fill_color != Color32::TRANSPARENT && verts.len() >= 3 {
                painter.add(egui::Shape::convex_polygon(
                    verts.clone(),
                    fill_color,
                    Stroke::NONE,
                ));
            }
            for i in 0..verts.len() {
                painter.line_segment([verts[i], verts[(i + 1) % verts.len()]], stroke);
            }
        }
    }
}

pub fn draw_non_stroke_object(
    painter: &Painter,
    obj: &DrawObject,
    page_to_screen: impl Fn(Pos2) -> Pos2,
    zoom: f32,
    textures: &HashMap<String, egui::TextureHandle>,
) {
    match obj {
        DrawObject::Stroke { .. } => {}
        DrawObject::Shape {
            shape_type,
            x,
            y,
            width,
            height,
            color,
            stroke_width,
            fill,
            ..
        } => {
            let stroke = Stroke::new(stroke_width * zoom, color.to_color32());
            let fill_color = fill
                .as_ref()
                .map_or(Color32::TRANSPARENT, |f| f.to_color32());
            draw_shape_primitive(
                painter,
                *shape_type,
                *x,
                *y,
                *width,
                *height,
                fill_color,
                stroke,
                page_to_screen,
            );
        }
        DrawObject::Text {
            x,
            y,
            text,
            font_size,
            color,
            rotation,
            ..
        } => {
            let pos = page_to_screen(Pos2::new(*x, *y));
            let galley = painter.layout_no_wrap(
                text.clone(),
                egui::FontId::proportional(font_size * zoom),
                color.to_color32(),
            );
            let mut text_shape = egui::epaint::TextShape::new(pos, galley, color.to_color32());
            text_shape.angle = *rotation;
            painter.add(text_shape);
        }
        DrawObject::StickyNote {
            x,
            y,
            width,
            height,
            text,
            color,
            ..
        } => {
            let p1 = page_to_screen(Pos2::new(*x, *y));
            let p2 = page_to_screen(Pos2::new(x + width, y + height));
            let r = Rect::from_two_pos(p1, p2);
            painter.rect_filled(r, 4.0, color.to_color32());
            painter.rect_stroke(
                r,
                4.0,
                Stroke::new(1.0_f32, Color32::from_rgb(235, 208, 116)),
                egui::StrokeKind::Outside,
            );
            painter.text(
                r.left_top() + Vec2::new(10.0, 10.0),
                egui::Align2::LEFT_TOP,
                text,
                egui::FontId::proportional(14.0 * zoom),
                Color32::from_rgb(40, 40, 40),
            );
        }
        DrawObject::Image {
            asset_id,
            x,
            y,
            width,
            height,
            rotation,
            ..
        } => {
            let center = page_to_screen(Pos2::new(*x + *width / 2.0, *y + *height / 2.0));
            let half_w = (*width * zoom) / 2.0;
            let half_h = (*height * zoom) / 2.0;
            let r = Rect::from_center_size(center, Vec2::new(half_w * 2.0, half_h * 2.0));
            if let Some(tex) = textures.get(asset_id) {
                draw_rotated_image_object(painter, tex.id(), center, half_w, half_h, *rotation);
            } else {
                painter.rect_filled(r, 4.0, Color32::from_rgb(50, 50, 54));
                painter.rect_stroke(
                    r,
                    4.0,
                    Stroke::new(1.0_f32, Color32::from_rgb(90, 90, 95)),
                    egui::StrokeKind::Outside,
                );
                painter.text(
                    r.center(),
                    egui::Align2::CENTER_CENTER,
                    "🖼 Image",
                    egui::FontId::proportional(13.0 * zoom),
                    Color32::WHITE,
                );
            }
        }
    }
}

pub fn draw_rotated_background_image(
    painter: &Painter,
    tex_id: egui::TextureId,
    rect: Rect,
    rotation_deg: i32,
) {
    let (uv_tl, uv_tr, uv_br, uv_bl) = match rotation_deg.rem_euclid(360) {
        90 => (
            Pos2::new(0.0, 1.0),
            Pos2::new(0.0, 0.0),
            Pos2::new(1.0, 0.0),
            Pos2::new(1.0, 1.0),
        ),
        180 => (
            Pos2::new(1.0, 1.0),
            Pos2::new(0.0, 1.0),
            Pos2::new(0.0, 0.0),
            Pos2::new(1.0, 0.0),
        ),
        270 => (
            Pos2::new(1.0, 0.0),
            Pos2::new(1.0, 1.0),
            Pos2::new(0.0, 1.0),
            Pos2::new(0.0, 0.0),
        ),
        _ => (
            Pos2::new(0.0, 0.0),
            Pos2::new(1.0, 0.0),
            Pos2::new(1.0, 1.0),
            Pos2::new(0.0, 1.0),
        ),
    };

    let mut mesh = egui::Mesh::with_texture(tex_id);
    let idx = mesh.vertices.len() as u32;
    mesh.vertices.push(egui::epaint::Vertex {
        pos: rect.left_top(),
        uv: uv_tl,
        color: Color32::WHITE,
    });
    mesh.vertices.push(egui::epaint::Vertex {
        pos: rect.right_top(),
        uv: uv_tr,
        color: Color32::WHITE,
    });
    mesh.vertices.push(egui::epaint::Vertex {
        pos: rect.right_bottom(),
        uv: uv_br,
        color: Color32::WHITE,
    });
    mesh.vertices.push(egui::epaint::Vertex {
        pos: rect.left_bottom(),
        uv: uv_bl,
        color: Color32::WHITE,
    });

    mesh.add_triangle(idx, idx + 1, idx + 2);
    mesh.add_triangle(idx, idx + 2, idx + 3);

    painter.add(mesh);
}

pub fn draw_rotated_image_object(
    painter: &Painter,
    tex_id: egui::TextureId,
    center: Pos2,
    half_w: f32,
    half_h: f32,
    rot_rad: f32,
) {
    let cos_a = rot_rad.cos();
    let sin_a = rot_rad.sin();
    let rot = |dx: f32, dy: f32| -> Pos2 {
        center + Vec2::new(dx * cos_a - dy * sin_a, dx * sin_a + dy * cos_a)
    };

    let p0 = rot(-half_w, -half_h);
    let p1 = rot(half_w, -half_h);
    let p2 = rot(half_w, half_h);
    let p3 = rot(-half_w, half_h);

    let mut mesh = egui::Mesh::with_texture(tex_id);
    let idx = mesh.vertices.len() as u32;
    mesh.vertices.push(egui::epaint::Vertex {
        pos: p0,
        uv: Pos2::new(0.0, 0.0),
        color: Color32::WHITE,
    });
    mesh.vertices.push(egui::epaint::Vertex {
        pos: p1,
        uv: Pos2::new(1.0, 0.0),
        color: Color32::WHITE,
    });
    mesh.vertices.push(egui::epaint::Vertex {
        pos: p2,
        uv: Pos2::new(1.0, 1.0),
        color: Color32::WHITE,
    });
    mesh.vertices.push(egui::epaint::Vertex {
        pos: p3,
        uv: Pos2::new(0.0, 1.0),
        color: Color32::WHITE,
    });

    mesh.add_triangle(idx, idx + 1, idx + 2);
    mesh.add_triangle(idx, idx + 2, idx + 3);

    painter.add(mesh);
}

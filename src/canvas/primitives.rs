use crate::model::{DrawObject, PageTemplate, ShapeKind};
use eframe::egui::{self, Color32, Mesh, Painter, Pos2, Rect, Stroke, Vec2};

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
        PageTemplate::RuledNarrow => {
            let sp = 20.0 * zoom;
            let mut y = rect.top() + sp;
            while y < rect.bottom() {
                painter.line_segment(
                    [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
                    Stroke::new(0.7_f32, line_color),
                );
                y += sp;
            }
        }
        PageTemplate::RuledWide => {
            let sp = 40.0 * zoom;
            let mut y = rect.top() + sp;
            while y < rect.bottom() {
                painter.line_segment(
                    [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
                    Stroke::new(0.85_f32, line_color),
                );
                y += sp;
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
        PageTemplate::GridFine => {
            let sp = 15.0 * zoom;
            let mut y = rect.top() + sp;
            while y < rect.bottom() {
                painter.line_segment(
                    [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
                    Stroke::new(0.6_f32, line_color),
                );
                y += sp;
            }
            let mut x = rect.left() + sp;
            while x < rect.right() {
                painter.line_segment(
                    [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
                    Stroke::new(0.6_f32, line_color),
                );
                x += sp;
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
        PageTemplate::DottedFine => {
            let sp = 15.0 * zoom;
            let mut y = rect.top() + sp;
            while y < rect.bottom() {
                let mut x = rect.left() + sp;
                while x < rect.right() {
                    painter.circle_filled(Pos2::new(x, y), 0.75, Color32::from_rgb(204, 211, 218));
                    x += sp;
                }
                y += sp;
            }
        }
        PageTemplate::Isometric => {
            let dx = 30.0 * zoom;
            let dy = 30.0 * 0.8660254 * zoom;
            let dot_col = Color32::from_rgb(204, 211, 218);
            let mut row = 0usize;
            let mut y = rect.top() + dy;
            while y < rect.bottom() {
                let x_offset = if row % 2 == 1 { dx * 0.5 } else { 0.0 };
                let mut x = rect.left() + x_offset + dx;
                while x < rect.right() {
                    painter.circle_filled(Pos2::new(x, y), 0.9, dot_col);
                    x += dx;
                }
                y += dy;
                row += 1;
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
    rotation: f32,
    corner_radius: f32,
    curve_points: &[crate::model::Point2D],
    fill_color: Color32,
    stroke: Stroke,
    page_to_screen: impl Fn(Pos2) -> Pos2,
) {
    let min_x = if width >= 0.0 { x } else { x + width };
    let min_y = if height >= 0.0 { y } else { y + height };
    let abs_w = width.abs();
    let abs_h = height.abs();
    let center = Pos2::new(min_x + abs_w * 0.5, min_y + abs_h * 0.5);

    match shape_type {
        ShapeKind::Line => {
            let mut p1 = Pos2::new(x, y);
            let mut p2 = Pos2::new(x + width, y + height);
            if rotation.abs() > 1e-5 {
                let rot_center = crate::model::Point2D::from(center);
                p1 = crate::model::Point2D::from(p1)
                    .rotate_around(rot_center, rotation)
                    .to_pos2();
                p2 = crate::model::Point2D::from(p2)
                    .rotate_around(rot_center, rotation)
                    .to_pos2();
            }
            painter.line_segment([page_to_screen(p1), page_to_screen(p2)], stroke);
        }
        ShapeKind::Rectangle => {
            if corner_radius > 0.0 || rotation.abs() > 1e-5 {
                let verts = crate::geometry::curves::rounded_rect_vertices(
                    x,
                    y,
                    width,
                    height,
                    corner_radius,
                    rotation,
                    8,
                );
                let screen_verts: Vec<Pos2> = verts
                    .into_iter()
                    .map(|p| page_to_screen(p.to_pos2()))
                    .collect();
                if fill_color != Color32::TRANSPARENT && screen_verts.len() >= 3 {
                    painter.add(egui::Shape::convex_polygon(
                        screen_verts.clone(),
                        fill_color,
                        Stroke::NONE,
                    ));
                }
                for i in 0..screen_verts.len() {
                    let next = (i + 1) % screen_verts.len();
                    painter.line_segment([screen_verts[i], screen_verts[next]], stroke);
                }
            } else {
                let p1 = page_to_screen(Pos2::new(x, y));
                let p2 = page_to_screen(Pos2::new(x + width, y + height));
                let r = Rect::from_two_pos(p1, p2);
                painter.rect(r, 0.0, fill_color, stroke, egui::StrokeKind::Outside);
            }
        }
        ShapeKind::Ellipse => {
            if rotation.abs() > 1e-5 {
                let rx = abs_w * 0.5;
                let ry = abs_h * 0.5;
                let steps = 36;
                let rot_center = crate::model::Point2D::from(center);
                let mut screen_verts = Vec::with_capacity(steps);
                for i in 0..steps {
                    let theta = i as f32 * std::f32::consts::TAU / steps as f32;
                    let local_pt = crate::model::Point2D::new(
                        center.x + rx * theta.cos(),
                        center.y + ry * theta.sin(),
                    );
                    let rot_pt = local_pt.rotate_around(rot_center, rotation);
                    screen_verts.push(page_to_screen(rot_pt.to_pos2()));
                }
                if fill_color != Color32::TRANSPARENT {
                    painter.add(egui::Shape::convex_polygon(
                        screen_verts.clone(),
                        fill_color,
                        Stroke::NONE,
                    ));
                }
                for i in 0..screen_verts.len() {
                    let next = (i + 1) % screen_verts.len();
                    painter.line_segment([screen_verts[i], screen_verts[next]], stroke);
                }
            } else {
                let p1 = page_to_screen(Pos2::new(x, y));
                let p2 = page_to_screen(Pos2::new(x + width, y + height));
                let r = Rect::from_two_pos(p1, p2);
                let radius = (r.width().min(r.height()) / 2.0).max(1.0);
                painter.circle(r.center(), radius, fill_color, stroke);
            }
        }
        ShapeKind::Diamond => {
            let top = Pos2::new(center.x, min_y);
            let right = Pos2::new(min_x + abs_w, center.y);
            let bottom = Pos2::new(center.x, min_y + abs_h);
            let left = Pos2::new(min_x, center.y);
            let raw_verts = [top, right, bottom, left];
            let rot_center = crate::model::Point2D::from(center);
            let screen_verts: Vec<Pos2> = raw_verts
                .into_iter()
                .map(|p| {
                    let rot_pt = if rotation.abs() > 1e-5 {
                        crate::model::Point2D::from(p)
                            .rotate_around(rot_center, rotation)
                            .to_pos2()
                    } else {
                        p
                    };
                    page_to_screen(rot_pt)
                })
                .collect();
            if fill_color != Color32::TRANSPARENT {
                painter.add(egui::Shape::convex_polygon(
                    screen_verts.clone(),
                    fill_color,
                    Stroke::NONE,
                ));
            }
            for i in 0..screen_verts.len() {
                let next = (i + 1) % screen_verts.len();
                painter.line_segment([screen_verts[i], screen_verts[next]], stroke);
            }
        }
        ShapeKind::Curve => {
            if curve_points.len() >= 2 {
                let beziers = crate::geometry::curves::fit_catmull_rom_to_bezier(curve_points);
                for bez in beziers {
                    let sampled = crate::geometry::curves::sample_cubic_bezier(
                        bez[0], bez[1], bez[2], bez[3], 16,
                    );
                    for w in sampled.windows(2) {
                        let p_a = page_to_screen(w[0].to_pos2());
                        let p_b = page_to_screen(w[1].to_pos2());
                        painter.line_segment([p_a, p_b], stroke);
                    }
                }
            } else {
                let p1 = page_to_screen(Pos2::new(x, y));
                let p2 = page_to_screen(Pos2::new(x + width, y + height));
                painter.line_segment([p1, p2], stroke);
            }
        }
        ShapeKind::Triangle
        | ShapeKind::RightTriangle
        | ShapeKind::ScaleneTriangle { .. }
        | ShapeKind::Parallelogram => {
            let rot_center = crate::model::Point2D::from(center);
            let verts: Vec<Pos2> = crate::geometry::shape_vertices(shape_type, x, y, width, height)
                .into_iter()
                .map(|p| {
                    let rot_pt = if rotation.abs() > 1e-5 {
                        crate::model::Point2D::from(p)
                            .rotate_around(rot_center, rotation)
                            .to_pos2()
                    } else {
                        p
                    };
                    page_to_screen(rot_pt)
                })
                .collect();
            if fill_color != Color32::TRANSPARENT && verts.len() >= 3 {
                painter.add(egui::Shape::convex_polygon(
                    verts.clone(),
                    fill_color,
                    Stroke::NONE,
                ));
            }
            for i in 0..verts.len() {
                let next = (i + 1) % verts.len();
                painter.line_segment([verts[i], verts[next]], stroke);
            }
        }
    }
}

pub fn draw_non_stroke_object(
    painter: &Painter,
    obj: &DrawObject,
    all_objects: &[DrawObject],
    page_to_screen: impl Fn(Pos2) -> Pos2,
    zoom: f32,
    get_texture: impl Fn(&str) -> Option<egui::TextureId>,
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
            rotation,
            corner_radius,
            label,
            curve_points,
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
                *rotation,
                *corner_radius,
                curve_points,
                fill_color,
                stroke,
                &page_to_screen,
            );

            // Render label teks di dalam bentuk jika ada (Task 6.2)
            if let Some(lbl) = label
                && !lbl.text.trim().is_empty()
            {
                let min_x = if *width >= 0.0 { *x } else { *x + *width };
                let min_y = if *height >= 0.0 { *y } else { *y + *height };
                let abs_w = width.abs();
                let abs_h = height.abs();
                let center = Pos2::new(min_x + abs_w * 0.5, min_y + abs_h * 0.5);
                let screen_center = page_to_screen(center);

                let (safe_w, safe_h) = crate::geometry::calculate_shape_text_bounds(
                    *shape_type,
                    *width,
                    *height,
                    *corner_radius,
                    *stroke_width,
                );
                let auto_font = crate::geometry::calculate_auto_fit_font_size(
                    lbl.font_size,
                    &lbl.text,
                    safe_w,
                    safe_h,
                    8.0,
                );

                let max_wrap_w = (safe_w * zoom).max(20.0);
                let galley = painter.layout(
                    lbl.text.clone(),
                    egui::FontId::proportional((auto_font * zoom).clamp(8.0, 72.0)),
                    lbl.color.to_color32(),
                    max_wrap_w,
                );
                let galley_size = galley.size();

                let offset_x = match lbl.alignment {
                    crate::model::TextAlign::Left => -safe_w * zoom * 0.5,
                    crate::model::TextAlign::Center => -galley_size.x * 0.5,
                    crate::model::TextAlign::Right => safe_w * zoom * 0.5 - galley_size.x,
                };
                let offset_y = match lbl.vertical_alignment {
                    crate::model::VerticalAlign::Top => -safe_h * zoom * 0.5,
                    crate::model::VerticalAlign::Middle => -galley_size.y * 0.5,
                    crate::model::VerticalAlign::Bottom => safe_h * zoom * 0.5 - galley_size.y,
                };

                let mut text_shape = egui::epaint::TextShape::new(
                    screen_center + Vec2::new(offset_x, offset_y),
                    galley,
                    lbl.color.to_color32(),
                );
                text_shape.angle = *rotation;
                painter.add(text_shape);
            }
        }
        DrawObject::Text {
            x,
            y,
            text,
            font_size,
            color,
            rotation,
            font_family,
            bold,
            italic,
            alignment,
            ..
        } => {
            let pos = page_to_screen(Pos2::new(*x, *y));
            let egui_family = font_family.to_egui_font_family();
            let scaled_font = (*font_size * zoom).clamp(4.0, 200.0);
            let font_id = egui::FontId::new(scaled_font, egui_family);
            let text_c = color.to_color32();

            let mut job = egui::text::LayoutJob {
                halign: alignment.to_egui_align(),
                ..Default::default()
            };
            job.append(
                text,
                0.0,
                egui::TextFormat {
                    font_id,
                    color: text_c,
                    italics: *italic,
                    ..Default::default()
                },
            );
            let galley = painter.layout_job(job);
            let mut text_shape = egui::epaint::TextShape::new(pos, galley.clone(), text_c);
            text_shape.angle = *rotation;
            painter.add(text_shape);
            if *bold {
                let bold_offset = Vec2::new(0.6 * zoom.clamp(0.5, 2.0), 0.0);
                let mut bold_shape =
                    egui::epaint::TextShape::new(pos + bold_offset, galley, text_c);
                bold_shape.angle = *rotation;
                painter.add(bold_shape);
            }
        }
        DrawObject::StickyNote {
            x,
            y,
            width,
            height,
            text,
            color,
            text_color,
            font_size,
            font_family,
            bold,
            italic,
            alignment,
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
            let scaled_font = (*font_size * zoom).clamp(6.0, 100.0);
            let egui_family = font_family.to_egui_font_family();
            let font_id = egui::FontId::new(scaled_font, egui_family);
            let t_col = text_color.to_color32();

            let mut job = egui::text::LayoutJob {
                halign: alignment.to_egui_align(),
                ..Default::default()
            };
            job.wrap.max_width = (r.width() - 20.0).max(10.0);
            job.append(
                text,
                0.0,
                egui::TextFormat {
                    font_id,
                    color: t_col,
                    italics: *italic,
                    ..Default::default()
                },
            );
            let galley = painter.layout_job(job);
            let text_pos = match alignment {
                crate::model::TextAlign::Left => r.left_top() + Vec2::new(10.0, 10.0),
                crate::model::TextAlign::Center => Pos2::new(r.center().x, r.top() + 10.0),
                crate::model::TextAlign::Right => Pos2::new(r.right() - 10.0, r.top() + 10.0),
            };
            let text_shape = egui::epaint::TextShape::new(text_pos, galley.clone(), t_col);
            painter.add(text_shape);
            if *bold {
                let bold_offset = Vec2::new(0.6 * zoom.clamp(0.5, 2.0), 0.0);
                let bold_shape =
                    egui::epaint::TextShape::new(text_pos + bold_offset, galley, t_col);
                painter.add(bold_shape);
            }
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
            if let Some(tex_id) = get_texture(asset_id) {
                draw_rotated_image_object(painter, tex_id, center, half_w, half_h, *rotation);
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
        DrawObject::Connector {
            start,
            end,
            route,
            control_points,
            style,
            ..
        } => {
            let ((start_pt, start_dir), (end_pt, end_dir)) =
                crate::geometry::resolve_connector_endpoints(start, end, all_objects);

            let path: Vec<crate::model::Point2D> = crate::geometry::calculate_connector_path(
                start_pt,
                start_dir,
                end_pt,
                end_dir,
                *route,
                control_points,
            );

            if path.len() >= 2 {
                let stroke = Stroke::new(
                    (style.stroke_width * zoom).max(1.0),
                    style.stroke_color.to_color32(),
                );
                for i in 0..path.len() - 1 {
                    let p1 = page_to_screen(path[i].to_pos2());
                    let p2 = page_to_screen(path[i + 1].to_pos2());
                    painter.line_segment([p1, p2], stroke);
                }

                // Render panah di ujung akhir (end marker)
                if style.end_marker == crate::model::MarkerKind::Arrow {
                    let last = path.len() - 1;
                    let incoming = path[last] - path[last - 1];
                    let arrow = crate::geometry::arrowhead_geometry(
                        path[last],
                        incoming,
                        (12.0 * zoom).max(6.0),
                        (9.0 * zoom).max(5.0),
                    );
                    let screen_arrow: Vec<Pos2> = arrow
                        .into_iter()
                        .map(|p| page_to_screen(p.to_pos2()))
                        .collect();
                    painter.add(egui::Shape::convex_polygon(
                        screen_arrow,
                        style.stroke_color.to_color32(),
                        Stroke::NONE,
                    ));
                }

                // Render panah di ujung awal (start marker)
                if style.start_marker == crate::model::MarkerKind::Arrow {
                    let incoming = path[0] - path[1];
                    let arrow = crate::geometry::arrowhead_geometry(
                        path[0],
                        incoming,
                        (12.0 * zoom).max(6.0),
                        (9.0 * zoom).max(5.0),
                    );
                    let screen_arrow: Vec<Pos2> = arrow
                        .into_iter()
                        .map(|p| page_to_screen(p.to_pos2()))
                        .collect();
                    painter.add(egui::Shape::convex_polygon(
                        screen_arrow,
                        style.stroke_color.to_color32(),
                        Stroke::NONE,
                    ));
                }
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

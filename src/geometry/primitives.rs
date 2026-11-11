use crate::model::{DrawObject, ShapeKind};
use eframe::egui::{Pos2, Rect, Vec2};

pub fn text_approx_size(text: &str, font_size: f32) -> (f32, f32) {
    let lines: Vec<&str> = text.lines().collect();
    let line_count = lines.len().max(1) as f32;
    let max_line_len = lines.iter().map(|l| l.chars().count()).max().unwrap_or(1) as f32;
    let approx_w = (max_line_len * (font_size * 0.58).max(8.0)).max(20.0);
    let approx_h = line_count * (font_size * 1.35).max(18.0);
    (approx_w, approx_h)
}

pub fn equation_approx_size(latex: &str, font_size: f32, scale: f32) -> (f32, f32) {
    let eff_font = font_size * scale;
    let layout = super::math::MathLayout::compute_from_str(latex, eff_font);
    (layout.width.max(20.0), layout.height.max(eff_font))
}

pub fn object_bounds(obj: &DrawObject) -> Rect {
    match obj {
        DrawObject::Stroke { points, width, .. } => {
            if points.is_empty() {
                return Rect::NOTHING;
            }
            let mut min_x = f32::MAX;
            let mut min_y = f32::MAX;
            let mut max_x = f32::MIN;
            let mut max_y = f32::MIN;
            for p in points {
                min_x = min_x.min(p.x);
                min_y = min_y.min(p.y);
                max_x = max_x.max(p.x);
                max_y = max_y.max(p.y);
            }
            let half = *width / 2.0;
            Rect::from_min_max(
                Pos2::new(min_x - half, min_y - half),
                Pos2::new(max_x + half, max_y + half),
            )
        }
        DrawObject::Shape {
            x,
            y,
            width,
            height,
            stroke_width,
            rotation,
            shape_type,
            ..
        } => {
            let half = *stroke_width / 2.0;
            if *shape_type == ShapeKind::Line {
                let mut p1 = Pos2::new(*x, *y);
                let mut p2 = Pos2::new(*x + *width, *y + *height);
                if rotation.abs() > 1e-4 {
                    let cx = *x + *width * 0.5;
                    let cy = *y + *height * 0.5;
                    let cos_a = rotation.cos();
                    let sin_a = rotation.sin();
                    let rot_pt = |p: Pos2| {
                        let dx = p.x - cx;
                        let dy = p.y - cy;
                        Pos2::new(cx + dx * cos_a - dy * sin_a, cy + dx * sin_a + dy * cos_a)
                    };
                    p1 = rot_pt(p1);
                    p2 = rot_pt(p2);
                }
                Rect::from_two_pos(p1, p2).expand(half)
            } else if rotation.abs() < 1e-3 {
                let left = x.min(x + width) - half;
                let right = x.max(x + width) + half;
                let top = y.min(y + height) - half;
                let bottom = y.max(y + height) + half;
                Rect::from_min_max(Pos2::new(left, top), Pos2::new(right, bottom))
            } else {
                let cx = *x + *width / 2.0;
                let cy = *y + *height / 2.0;
                let cos_a = rotation.cos().abs();
                let sin_a = rotation.sin().abs();
                let bb_w = width.abs() * cos_a + height.abs() * sin_a + stroke_width;
                let bb_h = width.abs() * sin_a + height.abs() * cos_a + stroke_width;
                Rect::from_center_size(Pos2::new(cx, cy), Vec2::new(bb_w, bb_h))
            }
        }
        DrawObject::Text {
            x,
            y,
            font_size,
            text,
            rotation,
            ..
        } => {
            let (w, h) = text_approx_size(text, *font_size);
            let rot = *rotation;
            if rot.abs() < 1e-3 {
                Rect::from_min_size(Pos2::new(*x, *y), Vec2::new(w, h))
            } else {
                let cos_a = rot.cos();
                let sin_a = rot.sin();
                let p0 = Pos2::new(*x, *y);
                let p1 = p0 + Vec2::new(w * cos_a, w * sin_a);
                let p2 = p1 + Vec2::new(-h * sin_a, h * cos_a);
                let p3 = p0 + Vec2::new(-h * sin_a, h * cos_a);
                let min_x = p0.x.min(p1.x).min(p2.x).min(p3.x);
                let max_x = p0.x.max(p1.x).max(p2.x).max(p3.x);
                let min_y = p0.y.min(p1.y).min(p2.y).min(p3.y);
                let max_y = p0.y.max(p1.y).max(p2.y).max(p3.y);
                Rect::from_min_max(Pos2::new(min_x, min_y), Pos2::new(max_x, max_y))
            }
        }
        DrawObject::StickyNote {
            x,
            y,
            width,
            height,
            rotation,
            ..
        } => {
            if rotation.abs() < 1e-3 {
                Rect::from_min_size(Pos2::new(*x, *y), Vec2::new(*width, *height))
            } else {
                let cx = *x + *width / 2.0;
                let cy = *y + *height / 2.0;
                let cos_a = rotation.cos().abs();
                let sin_a = rotation.sin().abs();
                let bb_w = width.abs() * cos_a + height.abs() * sin_a;
                let bb_h = width.abs() * sin_a + height.abs() * cos_a;
                Rect::from_center_size(Pos2::new(cx, cy), Vec2::new(bb_w, bb_h))
            }
        }
        DrawObject::Image {
            x,
            y,
            width,
            height,
            rotation,
            ..
        } => {
            let cx = *x + *width / 2.0;
            let cy = *y + *height / 2.0;
            let cos_a = rotation.cos().abs();
            let sin_a = rotation.sin().abs();
            let bb_w = *width * cos_a + *height * sin_a;
            let bb_h = *width * sin_a + *height * cos_a;
            Rect::from_center_size(Pos2::new(cx, cy), Vec2::new(bb_w, bb_h))
        }
        DrawObject::Equation {
            x,
            y,
            latex,
            font_size,
            scale,
            rotation,
            ..
        } => {
            let (w, h) = equation_approx_size(latex, *font_size, *scale);
            let rot = *rotation;
            if rot.abs() < 1e-3 {
                Rect::from_min_size(Pos2::new(*x, *y), Vec2::new(w, h))
            } else {
                let cx = *x + w * 0.5;
                let cy = *y + h * 0.5;
                let cos_a = rot.cos().abs();
                let sin_a = rot.sin().abs();
                let bb_w = w * cos_a + h * sin_a;
                let bb_h = w * sin_a + h * cos_a;
                Rect::from_center_size(Pos2::new(cx, cy), Vec2::new(bb_w, bb_h))
            }
        }
        DrawObject::Connector {
            start,
            end,
            control_points,
            style,
            ..
        } => {
            let mut pts = Vec::new();
            if let crate::model::ConnectorEnd::Free { x, y } = start {
                pts.push(Pos2::new(*x, *y));
            }
            if let crate::model::ConnectorEnd::Free { x, y } = end {
                pts.push(Pos2::new(*x, *y));
            }
            for cp in control_points {
                pts.push(Pos2::new(cp.x, cp.y));
            }
            if pts.is_empty() {
                Rect::from_min_size(Pos2::ZERO, Vec2::splat(10.0))
            } else {
                let mut min_x = f32::MAX;
                let mut min_y = f32::MAX;
                let mut max_x = f32::MIN;
                let mut max_y = f32::MIN;
                for p in pts {
                    min_x = min_x.min(p.x);
                    min_y = min_y.min(p.y);
                    max_x = max_x.max(p.x);
                    max_y = max_y.max(p.y);
                }
                let half = style.stroke_width * 0.5 + 4.0;
                Rect::from_min_max(
                    Pos2::new(min_x - half, min_y - half),
                    Pos2::new(max_x + half, max_y + half),
                )
            }
        }
    }
}

pub fn combined_bounds(objects: &[&DrawObject]) -> Option<Rect> {
    let mut combined: Option<Rect> = None;
    for obj in objects {
        let b = object_bounds(obj);
        if b.is_positive() {
            combined = match combined {
                Some(acc) => Some(acc.union(b)),
                None => Some(b),
            };
        }
    }
    combined
}

pub fn point_to_segment_dist(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let len_sq = ab.length_sq();
    if len_sq < 1e-6 {
        return p.distance(a);
    }
    let t = ((p - a).dot(ab) / len_sq).clamp(0.0, 1.0);
    let proj = a + ab * t;
    p.distance(proj)
}

pub fn shape_vertices(shape_type: ShapeKind, x: f32, y: f32, width: f32, height: f32) -> Vec<Pos2> {
    let p1 = Pos2::new(x, y);
    let p2 = Pos2::new(x + width, y + height);
    let r = Rect::from_two_pos(p1, p2);
    match shape_type {
        ShapeKind::Line | ShapeKind::Curve => vec![p1, p2],
        ShapeKind::Rectangle | ShapeKind::Ellipse => vec![
            r.left_top(),
            r.right_top(),
            r.right_bottom(),
            r.left_bottom(),
        ],
        ShapeKind::Diamond => vec![
            Pos2::new(r.center().x, r.top()),
            Pos2::new(r.right(), r.center().y),
            Pos2::new(r.center().x, r.bottom()),
            Pos2::new(r.left(), r.center().y),
        ],
        ShapeKind::Triangle => vec![
            Pos2::new(r.center().x, r.top()),
            Pos2::new(r.left(), r.bottom()),
            Pos2::new(r.right(), r.bottom()),
        ],
        ShapeKind::RightTriangle => vec![
            Pos2::new(x, y),
            Pos2::new(x, y + height),
            Pos2::new(x + width, y + height),
        ],
        ShapeKind::ScaleneTriangle {
            u0,
            v0,
            u1,
            v1,
            u2,
            v2,
        } => vec![
            Pos2::new(x + u0 * width, y + v0 * height),
            Pos2::new(x + u1 * width, y + v1 * height),
            Pos2::new(x + u2 * width, y + v2 * height),
        ],
        ShapeKind::Parallelogram => {
            let slant = r.width() * 0.22;
            if width >= 0.0 {
                vec![
                    Pos2::new(r.left() + slant, r.top()),
                    Pos2::new(r.right(), r.top()),
                    Pos2::new(r.right() - slant, r.bottom()),
                    Pos2::new(r.left(), r.bottom()),
                ]
            } else {
                vec![
                    Pos2::new(r.left(), r.top()),
                    Pos2::new(r.right() - slant, r.top()),
                    Pos2::new(r.right(), r.bottom()),
                    Pos2::new(r.left() + slant, r.bottom()),
                ]
            }
        }
    }
}

pub fn point_in_polygon(p: Pos2, polygon: &[Pos2]) -> bool {
    if polygon.len() < 3 {
        return false;
    }
    let mut inside = false;
    let mut j = polygon.len() - 1;
    for i in 0..polygon.len() {
        let pi = polygon[i];
        let pj = polygon[j];
        if (pi.y > p.y) != (pj.y > p.y)
            && p.x < (pj.x - pi.x) * (p.y - pi.y) / (pj.y - pi.y).max(1e-6) + pi.x
        {
            inside = !inside;
        }
        j = i;
    }
    inside
}

pub fn segments_cross(a: Pos2, b: Pos2, c: Pos2, d: Pos2) -> bool {
    let ccw = |p1: Pos2, p2: Pos2, p3: Pos2| -> bool {
        (p3.y - p1.y) * (p2.x - p1.x) > (p2.y - p1.y) * (p3.x - p1.x)
    };
    ccw(a, c, d) != ccw(b, c, d) && ccw(a, b, c) != ccw(a, b, d)
}

pub fn screen_to_page_point(screen_pos: Pos2, page_origin: Pos2, zoom: f32) -> Pos2 {
    let rel = screen_pos - page_origin;
    Pos2::new(rel.x / zoom, rel.y / zoom)
}

/// Menghitung simpul batas luar (boundary vertices/corners) dari suatu objek gambar
/// dengan memperhitungkan rotasi dan orientasi sebenarnya di kanvas.
pub fn object_boundary_vertices(obj: &DrawObject) -> Vec<Pos2> {
    match obj {
        DrawObject::Stroke { points, .. } => points.iter().map(|p| p.to_pos2()).collect(),
        DrawObject::Shape {
            x,
            y,
            width,
            height,
            rotation,
            shape_type,
            ..
        } => {
            if *shape_type == ShapeKind::Line {
                let mut p1 = Pos2::new(*x, *y);
                let mut p2 = Pos2::new(*x + *width, *y + *height);
                if rotation.abs() > 1e-4 {
                    let cx = *x + *width * 0.5;
                    let cy = *y + *height * 0.5;
                    let cos_a = rotation.cos();
                    let sin_a = rotation.sin();
                    let rot_pt = |p: Pos2| {
                        let dx = p.x - cx;
                        let dy = p.y - cy;
                        Pos2::new(cx + dx * cos_a - dy * sin_a, cy + dx * sin_a + dy * cos_a)
                    };
                    p1 = rot_pt(p1);
                    p2 = rot_pt(p2);
                }
                vec![p1, p2]
            } else {
                let mut verts = shape_vertices(*shape_type, *x, *y, *width, *height);
                if rotation.abs() > 1e-4 {
                    let cx = *x + *width * 0.5;
                    let cy = *y + *height * 0.5;
                    let cos_a = rotation.cos();
                    let sin_a = rotation.sin();
                    for v in &mut verts {
                        let dx = v.x - cx;
                        let dy = v.y - cy;
                        *v = Pos2::new(cx + dx * cos_a - dy * sin_a, cy + dx * sin_a + dy * cos_a);
                    }
                }
                verts
            }
        }
        DrawObject::Text {
            x,
            y,
            font_size,
            text,
            rotation,
            ..
        } => {
            let (w, h) = text_approx_size(text, *font_size);
            let p0 = Pos2::new(*x, *y);
            let cos_a = rotation.cos();
            let sin_a = rotation.sin();
            let rot_v =
                |lx: f32, ly: f32| Vec2::new(lx * cos_a - ly * sin_a, lx * sin_a + ly * cos_a);
            vec![p0, p0 + rot_v(w, 0.0), p0 + rot_v(w, h), p0 + rot_v(0.0, h)]
        }
        DrawObject::StickyNote {
            x,
            y,
            width,
            height,
            rotation,
            ..
        }
        | DrawObject::Image {
            x,
            y,
            width,
            height,
            rotation,
            ..
        } => {
            let center = Pos2::new(*x + *width * 0.5, *y + *height * 0.5);
            let hw = *width * 0.5;
            let hh = *height * 0.5;
            let cos_a = rotation.cos();
            let sin_a = rotation.sin();
            let rot_v =
                |lx: f32, ly: f32| Vec2::new(lx * cos_a - ly * sin_a, lx * sin_a + ly * cos_a);
            vec![
                center + rot_v(-hw, -hh),
                center + rot_v(hw, -hh),
                center + rot_v(hw, hh),
                center + rot_v(-hw, hh),
            ]
        }
        DrawObject::Equation {
            x,
            y,
            latex,
            font_size,
            scale,
            rotation,
            ..
        } => {
            let (w, h) = equation_approx_size(latex, *font_size, *scale);
            let center = Pos2::new(*x + w * 0.5, *y + h * 0.5);
            let hw = w * 0.5;
            let hh = h * 0.5;
            let cos_a = rotation.cos();
            let sin_a = rotation.sin();
            let rot_v =
                |lx: f32, ly: f32| Vec2::new(lx * cos_a - ly * sin_a, lx * sin_a + ly * cos_a);
            vec![
                center + rot_v(-hw, -hh),
                center + rot_v(hw, -hh),
                center + rot_v(hw, hh),
                center + rot_v(-hw, hh),
            ]
        }
        DrawObject::Connector {
            start,
            end,
            control_points,
            ..
        } => {
            let mut pts = Vec::new();
            if let crate::model::ConnectorEnd::Free { x, y } = start {
                pts.push(Pos2::new(*x, *y));
            }
            for cp in control_points {
                pts.push(Pos2::new(cp.x, cp.y));
            }
            if let crate::model::ConnectorEnd::Free { x, y } = end {
                pts.push(Pos2::new(*x, *y));
            }
            pts
        }
    }
}

pub fn page_to_screen_point(page_pos: Pos2, page_origin: Pos2, zoom: f32) -> Pos2 {
    page_origin + Vec2::new(page_pos.x * zoom, page_pos.y * zoom)
}

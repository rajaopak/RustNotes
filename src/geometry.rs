use crate::model::{DrawObject, ShapeKind, StrokePoint};
use eframe::egui::{Pos2, Rect};

pub fn object_bounds(obj: &DrawObject) -> Rect {
    match obj {
        DrawObject::Stroke { points, width, .. } => {
            if points.is_empty() {
                return Rect::NOTHING;
            }
            let mut min_x = f32::INFINITY;
            let mut min_y = f32::INFINITY;
            let mut max_x = f32::NEG_INFINITY;
            let mut max_y = f32::NEG_INFINITY;
            for p in points {
                min_x = min_x.min(p.x);
                min_y = min_y.min(p.y);
                max_x = max_x.max(p.x);
                max_y = max_y.max(p.y);
            }
            let pad = width / 2.0;
            Rect::from_min_max(
                Pos2::new(min_x - pad, min_y - pad),
                Pos2::new(max_x + pad, max_y + pad),
            )
        }
        DrawObject::Shape {
            x,
            y,
            width,
            height,
            stroke_width,
            ..
        } => {
            let min_x = x.min(x + width);
            let min_y = y.min(y + height);
            let max_x = x.max(x + width);
            let max_y = y.max(y + height);
            let pad = stroke_width / 2.0;
            Rect::from_min_max(
                Pos2::new(min_x - pad, min_y - pad),
                Pos2::new(max_x + pad, max_y + pad),
            )
        }
        DrawObject::Text {
            x,
            y,
            text,
            font_size,
            ..
        } => {
            let lines: Vec<&str> = text.lines().collect();
            let line_count = lines.len().max(1) as f32;
            let max_line_len = lines.iter().map(|l| l.len()).max().unwrap_or(1) as f32;
            let width = (max_line_len * font_size * 0.6).max(80.0);
            let height = line_count * font_size * 1.3;
            Rect::from_min_size(Pos2::new(*x, *y), [width, height].into())
        }
        DrawObject::StickyNote {
            x,
            y,
            width,
            height,
            ..
        } => Rect::from_min_size(Pos2::new(*x, *y), [*width, *height].into()),
        DrawObject::Image {
            x,
            y,
            width,
            height,
            ..
        } => Rect::from_min_size(Pos2::new(*x, *y), [*width, *height].into()),
    }
}

pub fn combined_bounds(objects: &[&DrawObject]) -> Option<Rect> {
    if objects.is_empty() {
        return None;
    }
    let mut total: Option<Rect> = None;
    for obj in objects {
        let b = object_bounds(obj);
        if b.is_finite() && b.width() > 0.0 && b.height() > 0.0 {
            total = Some(total.map_or(b, |t| t.union(b)));
        }
    }
    total
}

pub fn point_to_segment_dist(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let ap = p - a;
    let len_sq = ab.length_sq();
    if len_sq == 0.0 {
        return ap.length();
    }
    let t = (ap.dot(ab) / len_sq).clamp(0.0, 1.0);
    let proj = a + ab * t;
    (p - proj).length()
}

/// Menghasilkan daftar titik sudut (vertices) dalam koordinat halaman untuk setiap `ShapeKind`.
pub fn shape_vertices(shape_type: ShapeKind, x: f32, y: f32, width: f32, height: f32) -> Vec<Pos2> {
    let p1 = Pos2::new(x, y);
    let p2 = Pos2::new(x + width, y + height);
    let r = Rect::from_two_pos(p1, p2);
    match shape_type {
        ShapeKind::Line => vec![p1, p2],
        ShapeKind::Rectangle | ShapeKind::Ellipse => vec![
            r.left_top(),
            r.right_top(),
            r.right_bottom(),
            r.left_bottom(),
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

pub fn distance_to_object(obj: &DrawObject, p: Pos2) -> Option<f32> {
    let b = object_bounds(obj);
    if !b.is_finite() {
        return None;
    }
    match obj {
        DrawObject::Stroke { points, width, .. } => {
            if points.is_empty() {
                return None;
            }
            let half_w = width * 0.5;
            if points.len() == 1 {
                return Some(((p - points[0].to_pos2()).length() - half_w).max(0.0));
            }
            let mut min_dist = f32::INFINITY;
            for i in 1..points.len() {
                let a = points[i - 1].to_pos2();
                let b_pt = points[i].to_pos2();
                let d = point_to_segment_dist(p, a, b_pt);
                if d < min_dist {
                    min_dist = d;
                }
            }
            Some((min_dist - half_w).max(0.0))
        }
        DrawObject::Shape {
            x,
            y,
            width,
            height,
            stroke_width,
            fill,
            shape_type,
            ..
        } => {
            if fill.is_some() && b.contains(p) {
                return Some(0.0);
            }
            let half_w = stroke_width * 0.5;
            match shape_type {
                crate::model::ShapeKind::Line => {
                    let a = Pos2::new(*x, *y);
                    let b_pt = Pos2::new(x + width, y + height);
                    Some((point_to_segment_dist(p, a, b_pt) - half_w).max(0.0))
                }
                crate::model::ShapeKind::Ellipse => {
                    let tl = b.left_top();
                    let tr = b.right_top();
                    let br = b.right_bottom();
                    let bl = b.left_bottom();
                    let d = point_to_segment_dist(p, tl, tr)
                        .min(point_to_segment_dist(p, tr, br))
                        .min(point_to_segment_dist(p, br, bl))
                        .min(point_to_segment_dist(p, bl, tl));
                    Some((d - half_w).max(0.0))
                }
                _ => {
                    let verts = shape_vertices(*shape_type, *x, *y, *width, *height);
                    let mut min_d = f32::INFINITY;
                    for i in 0..verts.len() {
                        let a = verts[i];
                        let b_pt = verts[(i + 1) % verts.len()];
                        min_d = min_d.min(point_to_segment_dist(p, a, b_pt));
                    }
                    Some((min_d - half_w).max(0.0))
                }
            }
        }
        DrawObject::Text { .. } | DrawObject::StickyNote { .. } | DrawObject::Image { .. } => {
            if b.contains(p) { Some(0.0) } else { None }
        }
    }
}

pub fn hit_test_object(obj: &DrawObject, p: Pos2, tolerance: f32) -> bool {
    let b = object_bounds(obj);
    if !b.expand(tolerance).contains(p) {
        return false;
    }
    match obj {
        DrawObject::Stroke { points, width, .. } => {
            let effective_tol = tolerance + width / 2.0;
            if points.len() == 1 {
                return (p - points[0].to_pos2()).length() <= effective_tol;
            }
            for i in 1..points.len() {
                let a = points[i - 1].to_pos2();
                let b = points[i].to_pos2();
                if point_to_segment_dist(p, a, b) <= effective_tol {
                    return true;
                }
            }
            false
        }
        DrawObject::Shape {
            x,
            y,
            width,
            height,
            stroke_width,
            fill,
            shape_type,
            ..
        } => {
            if fill.is_some() {
                return b.contains(p);
            }
            let tol = tolerance + stroke_width / 2.0;
            match shape_type {
                crate::model::ShapeKind::Line => {
                    let a = Pos2::new(*x, *y);
                    let b = Pos2::new(x + width, y + height);
                    point_to_segment_dist(p, a, b) <= tol
                }
                crate::model::ShapeKind::Rectangle | crate::model::ShapeKind::Ellipse => {
                    b.expand(tol).contains(p) && !b.shrink(tol).contains(p)
                }
                _ => {
                    let verts = shape_vertices(*shape_type, *x, *y, *width, *height);
                    for i in 0..verts.len() {
                        let a = verts[i];
                        let b_pt = verts[(i + 1) % verts.len()];
                        if point_to_segment_dist(p, a, b_pt) <= tol {
                            return true;
                        }
                    }
                    false
                }
            }
        }
        DrawObject::Text { .. } | DrawObject::StickyNote { .. } | DrawObject::Image { .. } => {
            b.contains(p)
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
        let intersect = ((pi.y > p.y) != (pj.y > p.y))
            && (p.x < (pj.x - pi.x) * (p.y - pi.y) / (pj.y - pi.y) + pi.x);
        if intersect {
            inside = !inside;
        }
        j = i;
    }
    inside
}

pub fn segments_cross(a: Pos2, b: Pos2, c: Pos2, d: Pos2) -> bool {
    fn ccw(p1: Pos2, p2: Pos2, p3: Pos2) -> bool {
        (p3.y - p1.y) * (p2.x - p1.x) > (p2.y - p1.y) * (p3.x - p1.x)
    }
    (ccw(a, c, d) != ccw(b, c, d)) && (ccw(a, b, c) != ccw(a, b, d))
}

pub fn object_in_lasso(obj: &DrawObject, polygon: &[Pos2]) -> bool {
    if polygon.len() < 3 {
        return false;
    }
    let b = object_bounds(obj);

    // If center of object is inside lasso
    if point_in_polygon(b.center(), polygon) {
        return true;
    }

    // Check individual points
    match obj {
        DrawObject::Stroke { points, .. } => {
            if points
                .iter()
                .any(|pt| point_in_polygon(pt.to_pos2(), polygon))
            {
                return true;
            }
            // Check segment intersection
            for i in 1..points.len() {
                let p1 = points[i - 1].to_pos2();
                let p2 = points[i].to_pos2();
                for j in 0..polygon.len() {
                    let l1 = polygon[j];
                    let l2 = polygon[(j + 1) % polygon.len()];
                    if segments_cross(p1, p2, l1, l2) {
                        return true;
                    }
                }
            }
            false
        }
        DrawObject::Shape {
            shape_type,
            x,
            y,
            width,
            height,
            ..
        } => {
            let verts = shape_vertices(*shape_type, *x, *y, *width, *height);
            if verts.iter().any(|&v| point_in_polygon(v, polygon)) {
                return true;
            }
            for i in 0..verts.len() {
                let p1 = verts[i];
                let p2 = verts[(i + 1) % verts.len()];
                for j in 0..polygon.len() {
                    let l1 = polygon[j];
                    let l2 = polygon[(j + 1) % polygon.len()];
                    if segments_cross(p1, p2, l1, l2) {
                        return true;
                    }
                }
            }
            false
        }
        DrawObject::Text { .. } | DrawObject::StickyNote { .. } | DrawObject::Image { .. } => {
            let corners = [
                b.left_top(),
                b.right_top(),
                b.right_bottom(),
                b.left_bottom(),
            ];
            corners.iter().any(|&c| point_in_polygon(c, polygon))
        }
    }
}

pub fn translate_object(obj: &mut DrawObject, dx: f32, dy: f32) {
    match obj {
        DrawObject::Stroke { points, .. } => {
            for p in points {
                p.x += dx;
                p.y += dy;
            }
        }
        DrawObject::Shape { x, y, .. } => {
            *x += dx;
            *y += dy;
        }
        DrawObject::Text { x, y, .. } => {
            *x += dx;
            *y += dy;
        }
        DrawObject::StickyNote { x, y, .. } => {
            *x += dx;
            *y += dy;
        }
        DrawObject::Image { x, y, .. } => {
            *x += dx;
            *y += dy;
        }
    }
}

pub fn scale_object(obj: &mut DrawObject, origin: Pos2, sx: f32, sy: f32) {
    match obj {
        DrawObject::Stroke { points, width, .. } => {
            for p in points {
                p.x = origin.x + (p.x - origin.x) * sx;
                p.y = origin.y + (p.y - origin.y) * sy;
            }
            *width *= (sx.abs() + sy.abs()) / 2.0;
        }
        DrawObject::Shape {
            x,
            y,
            width,
            height,
            stroke_width,
            ..
        } => {
            *x = origin.x + (*x - origin.x) * sx;
            *y = origin.y + (*y - origin.y) * sy;
            *width *= sx;
            *height *= sy;
            *stroke_width *= (sx.abs() + sy.abs()) / 2.0;
        }
        DrawObject::Text {
            x, y, font_size, ..
        } => {
            *x = origin.x + (*x - origin.x) * sx;
            *y = origin.y + (*y - origin.y) * sy;
            *font_size *= (sx.abs() + sy.abs()) / 2.0;
        }
        DrawObject::StickyNote {
            x,
            y,
            width,
            height,
            ..
        } => {
            *x = origin.x + (*x - origin.x) * sx;
            *y = origin.y + (*y - origin.y) * sy;
            *width *= sx;
            *height *= sy;
        }
        DrawObject::Image {
            x,
            y,
            width,
            height,
            ..
        } => {
            *x = origin.x + (*x - origin.x) * sx;
            *y = origin.y + (*y - origin.y) * sy;
            *width *= sx;
            *height *= sy;
        }
    }
}

pub fn flip_object_horizontal(obj: &mut DrawObject, center_x: f32) {
    match obj {
        DrawObject::Stroke { points, .. } => {
            for p in points {
                p.x = 2.0 * center_x - p.x;
            }
        }
        DrawObject::Shape {
            x,
            width,
            shape_type,
            ..
        } => match shape_type {
            ShapeKind::ScaleneTriangle { u0, u1, u2, .. } => {
                *x = 2.0 * center_x - (*x + *width);
                *u0 = 1.0 - *u0;
                *u1 = 1.0 - *u1;
                *u2 = 1.0 - *u2;
            }
            ShapeKind::RightTriangle | ShapeKind::Parallelogram | ShapeKind::Line => {
                *x = 2.0 * center_x - *x;
                *width = -*width;
            }
            _ => {
                *x = 2.0 * center_x - (*x + *width);
            }
        },
        DrawObject::Text {
            x, text, font_size, ..
        } => {
            let lines: Vec<&str> = text.lines().collect();
            let max_line_len = lines.iter().map(|l| l.len()).max().unwrap_or(1) as f32;
            let width = (max_line_len * *font_size * 0.6).max(80.0);
            *x = 2.0 * center_x - (*x + width);
        }
        DrawObject::StickyNote { x, width, .. } => {
            *x = 2.0 * center_x - (*x + *width);
        }
        DrawObject::Image { x, width, .. } => {
            *x = 2.0 * center_x - (*x + *width);
        }
    }
}

pub fn flip_object_vertical(obj: &mut DrawObject, center_y: f32) {
    match obj {
        DrawObject::Stroke { points, .. } => {
            for p in points {
                p.y = 2.0 * center_y - p.y;
            }
        }
        DrawObject::Shape {
            y,
            height,
            shape_type,
            ..
        } => match shape_type {
            ShapeKind::ScaleneTriangle { v0, v1, v2, .. } => {
                *y = 2.0 * center_y - (*y + *height);
                *v0 = 1.0 - *v0;
                *v1 = 1.0 - *v1;
                *v2 = 1.0 - *v2;
            }
            ShapeKind::RightTriangle | ShapeKind::Line => {
                *y = 2.0 * center_y - *y;
                *height = -*height;
            }
            _ => {
                *y = 2.0 * center_y - (*y + *height);
            }
        },
        DrawObject::Text {
            y, text, font_size, ..
        } => {
            let line_count = text.lines().count().max(1) as f32;
            let height = line_count * *font_size * 1.3;
            *y = 2.0 * center_y - (*y + height);
        }
        DrawObject::StickyNote { y, height, .. } => {
            *y = 2.0 * center_y - (*y + *height);
        }
        DrawObject::Image { y, height, .. } => {
            *y = 2.0 * center_y - (*y + *height);
        }
    }
}

pub fn rotate_object(obj: &mut DrawObject, origin: Pos2, angle_rad: f32) {
    let cos_a = angle_rad.cos();
    let sin_a = angle_rad.sin();
    let rotate_pt = |x: f32, y: f32| -> (f32, f32) {
        let dx = x - origin.x;
        let dy = y - origin.y;
        (
            origin.x + dx * cos_a - dy * sin_a,
            origin.y + dx * sin_a + dy * cos_a,
        )
    };

    match obj {
        DrawObject::Stroke { points, .. } => {
            for p in points {
                let (nx, ny) = rotate_pt(p.x, p.y);
                p.x = nx;
                p.y = ny;
            }
        }
        DrawObject::Shape {
            x,
            y,
            width,
            height,
            shape_type,
            ..
        } => {
            match shape_type {
                crate::model::ShapeKind::Line => {
                    // Untuk garis: titik awal (x, y) menjadi pusat rotasi (pivot),
                    // sedangkan titik akhir (x + width, y + height) berotasi mengelilingi titik awal.
                    let dx = *width;
                    let dy = *height;
                    *width = dx * cos_a - dy * sin_a;
                    *height = dx * sin_a + dy * cos_a;
                }
                crate::model::ShapeKind::Triangle
                | crate::model::ShapeKind::RightTriangle
                | crate::model::ShapeKind::ScaleneTriangle { .. } => {
                    let verts = shape_vertices(*shape_type, *x, *y, *width, *height);
                    if verts.len() == 3 {
                        let r0 = rotate_pt(verts[0].x, verts[0].y);
                        let r1 = rotate_pt(verts[1].x, verts[1].y);
                        let r2 = rotate_pt(verts[2].x, verts[2].y);
                        let min_x = r0.0.min(r1.0).min(r2.0);
                        let min_y = r0.1.min(r1.1).min(r2.1);
                        let max_x = r0.0.max(r1.0).max(r2.0);
                        let max_y = r0.1.max(r1.1).max(r2.1);
                        let nw = (max_x - min_x).max(1.0);
                        let nh = (max_y - min_y).max(1.0);
                        *x = min_x;
                        *y = min_y;
                        *width = nw;
                        *height = nh;
                        *shape_type = crate::model::ShapeKind::ScaleneTriangle {
                            u0: ((r0.0 - min_x) / nw).clamp(0.0, 1.0),
                            v0: ((r0.1 - min_y) / nh).clamp(0.0, 1.0),
                            u1: ((r1.0 - min_x) / nw).clamp(0.0, 1.0),
                            v1: ((r1.1 - min_y) / nh).clamp(0.0, 1.0),
                            u2: ((r2.0 - min_x) / nw).clamp(0.0, 1.0),
                            v2: ((r2.1 - min_y) / nh).clamp(0.0, 1.0),
                        };
                    }
                }
                _ => {
                    let (nx, ny) = rotate_pt(*x + *width / 2.0, *y + *height / 2.0);
                    *x = nx - *width / 2.0;
                    *y = ny - *height / 2.0;
                }
            }
        }
        DrawObject::Text { x, y, .. } => {
            let (nx, ny) = rotate_pt(*x, *y);
            *x = nx;
            *y = ny;
        }
        DrawObject::StickyNote { x, y, .. } => {
            let (nx, ny) = rotate_pt(*x, *y);
            *x = nx;
            *y = ny;
        }
        DrawObject::Image {
            x,
            y,
            width,
            height,
            rotation,
            ..
        } => {
            let (nx, ny) = rotate_pt(*x + *width / 2.0, *y + *height / 2.0);
            *x = nx - *width / 2.0;
            *y = ny - *height / 2.0;
            *rotation += angle_rad;
        }
    }
}

/// Rotasi penuh isi halaman dan ukurannya (90° CW, 90° CCW / 270°, atau 180°).
pub fn rotate_page_content(page: &mut crate::model::Page, degrees: i32) {
    let normalized = degrees.rem_euclid(360);
    if normalized == 0 {
        return;
    }
    let old_w = page.width;
    let old_h = page.height;

    for obj in &mut page.objects {
        match normalized {
            90 => rotate_object_90_cw(obj, old_h),
            180 => rotate_object_180(obj, old_w, old_h),
            270 => rotate_object_90_ccw(obj, old_w),
            _ => {}
        }
    }

    if normalized == 90 || normalized == 270 {
        page.width = old_h;
        page.height = old_w;
    }

    page.rotation = (page.rotation + normalized).rem_euclid(360);
}

fn rotate_object_90_cw(obj: &mut DrawObject, old_h: f32) {
    match obj {
        DrawObject::Stroke { points, .. } => {
            for p in points {
                let nx = old_h - p.y;
                let ny = p.x;
                p.x = nx;
                p.y = ny;
            }
        }
        DrawObject::Shape {
            x,
            y,
            width,
            height,
            shape_type,
            ..
        } => match shape_type {
            ShapeKind::Line => {
                let x1 = *x;
                let y1 = *y;
                let x2 = *x + *width;
                let y2 = *y + *height;
                *x = old_h - y1;
                *y = x1;
                let nx2 = old_h - y2;
                let ny2 = x2;
                *width = nx2 - *x;
                *height = ny2 - *y;
            }
            ShapeKind::Rectangle | ShapeKind::Ellipse => {
                let nx = old_h - (*y + *height);
                let ny = *x;
                *x = nx;
                *y = ny;
                std::mem::swap(width, height);
            }
            _ => {
                let verts = shape_vertices(*shape_type, *x, *y, *width, *height);
                if !verts.is_empty() {
                    let mut r_verts = Vec::with_capacity(verts.len());
                    for v in &verts {
                        r_verts.push(Pos2::new(old_h - v.y, v.x));
                    }
                    let min_x = r_verts.iter().map(|v| v.x).fold(f32::INFINITY, f32::min);
                    let min_y = r_verts.iter().map(|v| v.y).fold(f32::INFINITY, f32::min);
                    let max_x = r_verts
                        .iter()
                        .map(|v| v.x)
                        .fold(f32::NEG_INFINITY, f32::max);
                    let max_y = r_verts
                        .iter()
                        .map(|v| v.y)
                        .fold(f32::NEG_INFINITY, f32::max);
                    let nw = (max_x - min_x).max(1.0);
                    let nh = (max_y - min_y).max(1.0);
                    *x = min_x;
                    *y = min_y;
                    *width = nw;
                    *height = nh;
                    if r_verts.len() == 3 {
                        *shape_type = ShapeKind::ScaleneTriangle {
                            u0: ((r_verts[0].x - min_x) / nw).clamp(0.0, 1.0),
                            v0: ((r_verts[0].y - min_y) / nh).clamp(0.0, 1.0),
                            u1: ((r_verts[1].x - min_x) / nw).clamp(0.0, 1.0),
                            v1: ((r_verts[1].y - min_y) / nh).clamp(0.0, 1.0),
                            u2: ((r_verts[2].x - min_x) / nw).clamp(0.0, 1.0),
                            v2: ((r_verts[2].y - min_y) / nh).clamp(0.0, 1.0),
                        };
                    }
                }
            }
        },
        DrawObject::Text { x, y, .. } => {
            let nx = old_h - *y;
            let ny = *x;
            *x = nx;
            *y = ny;
        }
        DrawObject::StickyNote {
            x,
            y,
            width,
            height,
            ..
        } => {
            let nx = old_h - (*y + *height);
            let ny = *x;
            *x = nx;
            *y = ny;
            std::mem::swap(width, height);
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
            let ncx = old_h - cy;
            let ncy = cx;
            std::mem::swap(width, height);
            *x = ncx - *width / 2.0;
            *y = ncy - *height / 2.0;
            *rotation += std::f32::consts::FRAC_PI_2;
        }
    }
}

fn rotate_object_90_ccw(obj: &mut DrawObject, old_w: f32) {
    match obj {
        DrawObject::Stroke { points, .. } => {
            for p in points {
                let nx = p.y;
                let ny = old_w - p.x;
                p.x = nx;
                p.y = ny;
            }
        }
        DrawObject::Shape {
            x,
            y,
            width,
            height,
            shape_type,
            ..
        } => match shape_type {
            ShapeKind::Line => {
                let x1 = *x;
                let y1 = *y;
                let x2 = *x + *width;
                let y2 = *y + *height;
                *x = y1;
                *y = old_w - x1;
                let nx2 = y2;
                let ny2 = old_w - x2;
                *width = nx2 - *x;
                *height = ny2 - *y;
            }
            ShapeKind::Rectangle | ShapeKind::Ellipse => {
                let nx = *y;
                let ny = old_w - (*x + *width);
                *x = nx;
                *y = ny;
                std::mem::swap(width, height);
            }
            _ => {
                let verts = shape_vertices(*shape_type, *x, *y, *width, *height);
                if !verts.is_empty() {
                    let mut r_verts = Vec::with_capacity(verts.len());
                    for v in &verts {
                        r_verts.push(Pos2::new(v.y, old_w - v.x));
                    }
                    let min_x = r_verts.iter().map(|v| v.x).fold(f32::INFINITY, f32::min);
                    let min_y = r_verts.iter().map(|v| v.y).fold(f32::INFINITY, f32::min);
                    let max_x = r_verts
                        .iter()
                        .map(|v| v.x)
                        .fold(f32::NEG_INFINITY, f32::max);
                    let max_y = r_verts
                        .iter()
                        .map(|v| v.y)
                        .fold(f32::NEG_INFINITY, f32::max);
                    let nw = (max_x - min_x).max(1.0);
                    let nh = (max_y - min_y).max(1.0);
                    *x = min_x;
                    *y = min_y;
                    *width = nw;
                    *height = nh;
                    if r_verts.len() == 3 {
                        *shape_type = ShapeKind::ScaleneTriangle {
                            u0: ((r_verts[0].x - min_x) / nw).clamp(0.0, 1.0),
                            v0: ((r_verts[0].y - min_y) / nh).clamp(0.0, 1.0),
                            u1: ((r_verts[1].x - min_x) / nw).clamp(0.0, 1.0),
                            v1: ((r_verts[1].y - min_y) / nh).clamp(0.0, 1.0),
                            u2: ((r_verts[2].x - min_x) / nw).clamp(0.0, 1.0),
                            v2: ((r_verts[2].y - min_y) / nh).clamp(0.0, 1.0),
                        };
                    }
                }
            }
        },
        DrawObject::Text { x, y, .. } => {
            let nx = *y;
            let ny = old_w - *x;
            *x = nx;
            *y = ny;
        }
        DrawObject::StickyNote {
            x,
            y,
            width,
            height,
            ..
        } => {
            let nx = *y;
            let ny = old_w - (*x + *width);
            *x = nx;
            *y = ny;
            std::mem::swap(width, height);
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
            let ncx = cy;
            let ncy = old_w - cx;
            std::mem::swap(width, height);
            *x = ncx - *width / 2.0;
            *y = ncy - *height / 2.0;
            *rotation -= std::f32::consts::FRAC_PI_2;
        }
    }
}

fn rotate_object_180(obj: &mut DrawObject, old_w: f32, old_h: f32) {
    match obj {
        DrawObject::Stroke { points, .. } => {
            for p in points {
                p.x = old_w - p.x;
                p.y = old_h - p.y;
            }
        }
        DrawObject::Shape {
            x,
            y,
            width,
            height,
            shape_type,
            ..
        } => match shape_type {
            ShapeKind::Line => {
                let x1 = *x;
                let y1 = *y;
                let x2 = *x + *width;
                let y2 = *y + *height;
                *x = old_w - x1;
                *y = old_h - y1;
                let nx2 = old_w - x2;
                let ny2 = old_h - y2;
                *width = nx2 - *x;
                *height = ny2 - *y;
            }
            ShapeKind::Rectangle | ShapeKind::Ellipse => {
                *x = old_w - (*x + *width);
                *y = old_h - (*y + *height);
            }
            _ => {
                let verts = shape_vertices(*shape_type, *x, *y, *width, *height);
                if !verts.is_empty() {
                    let mut r_verts = Vec::with_capacity(verts.len());
                    for v in &verts {
                        r_verts.push(Pos2::new(old_w - v.x, old_h - v.y));
                    }
                    let min_x = r_verts.iter().map(|v| v.x).fold(f32::INFINITY, f32::min);
                    let min_y = r_verts.iter().map(|v| v.y).fold(f32::INFINITY, f32::min);
                    let max_x = r_verts
                        .iter()
                        .map(|v| v.x)
                        .fold(f32::NEG_INFINITY, f32::max);
                    let max_y = r_verts
                        .iter()
                        .map(|v| v.y)
                        .fold(f32::NEG_INFINITY, f32::max);
                    let nw = (max_x - min_x).max(1.0);
                    let nh = (max_y - min_y).max(1.0);
                    *x = min_x;
                    *y = min_y;
                    *width = nw;
                    *height = nh;
                    if r_verts.len() == 3 {
                        *shape_type = ShapeKind::ScaleneTriangle {
                            u0: ((r_verts[0].x - min_x) / nw).clamp(0.0, 1.0),
                            v0: ((r_verts[0].y - min_y) / nh).clamp(0.0, 1.0),
                            u1: ((r_verts[1].x - min_x) / nw).clamp(0.0, 1.0),
                            v1: ((r_verts[1].y - min_y) / nh).clamp(0.0, 1.0),
                            u2: ((r_verts[2].x - min_x) / nw).clamp(0.0, 1.0),
                            v2: ((r_verts[2].y - min_y) / nh).clamp(0.0, 1.0),
                        };
                    }
                }
            }
        },
        DrawObject::Text { x, y, .. } => {
            *x = old_w - *x;
            *y = old_h - *y;
        }
        DrawObject::StickyNote {
            x,
            y,
            width,
            height,
            ..
        } => {
            *x = old_w - (*x + *width);
            *y = old_h - (*y + *height);
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
            let ncx = old_w - cx;
            let ncy = old_h - cy;
            *x = ncx - *width / 2.0;
            *y = ncy - *height / 2.0;
            *rotation += std::f32::consts::PI;
        }
    }
}

/// Smooth raw points using midpoint quadratic Bézier curve interpolation.
/// Guarantees that the start point and end point are exact, avoiding cursor lag.
pub fn smooth_stroke_points(points: &[StrokePoint], steps_per_segment: usize) -> Vec<StrokePoint> {
    if points.len() <= 2 || steps_per_segment <= 1 {
        return points.to_vec();
    }

    let mut result = Vec::with_capacity(points.len() * steps_per_segment);
    result.push(points[0].clone());

    for i in 0..(points.len() - 1) {
        let p0 = if i == 0 {
            points[0].clone()
        } else {
            StrokePoint {
                x: (points[i - 1].x + points[i].x) / 2.0,
                y: (points[i - 1].y + points[i].y) / 2.0,
                pressure: (points[i - 1].pressure + points[i].pressure) / 2.0,
            }
        };

        let p1 = points[i].clone(); // Control point

        let p2 = if i + 2 >= points.len() {
            points[points.len() - 1].clone()
        } else {
            StrokePoint {
                x: (points[i].x + points[i + 1].x) / 2.0,
                y: (points[i].y + points[i + 1].y) / 2.0,
                pressure: (points[i].pressure + points[i + 1].pressure) / 2.0,
            }
        };

        for step in 1..=steps_per_segment {
            let t = step as f32 / steps_per_segment as f32;
            let one_minus_t = 1.0 - t;
            // Quadratic Bezier: B(t) = (1-t)^2 * p0 + 2*(1-t)*t * p1 + t^2 * p2
            let x = one_minus_t * one_minus_t * p0.x + 2.0 * one_minus_t * t * p1.x + t * t * p2.x;
            let y = one_minus_t * one_minus_t * p0.y + 2.0 * one_minus_t * t * p1.y + t * t * p2.y;
            let pressure = one_minus_t * one_minus_t * p0.pressure
                + 2.0 * one_minus_t * t * p1.pressure
                + t * t * p2.pressure;

            result.push(StrokePoint { x, y, pressure });
        }
    }

    // Ensure the very last point matches the raw point exactly
    if let Some(last) = points.last()
        && let Some(res_last) = result.last_mut()
    {
        res_last.x = last.x;
        res_last.y = last.y;
        res_last.pressure = last.pressure;
    }

    result
}

/// Builds an `egui::Mesh` with triangle strip ribbon for seamless, overlap-free inking.
pub fn tessellate_stroke(
    points: &[StrokePoint],
    base_width: f32,
    color: eframe::egui::Color32,
    page_to_screen: impl Fn(Pos2) -> Pos2,
) -> eframe::egui::Mesh {
    tessellate_stroke_ex(points, base_width, color, false, page_to_screen)
}

/// Versi lanjutan tessellator dengan dukungan parameter `is_highlighter`
/// untuk memastikan ketebalan pita seragam dan bebas tumpang tindih (overlap-free).
pub fn tessellate_stroke_ex(
    points: &[StrokePoint],
    base_width: f32,
    color: eframe::egui::Color32,
    is_highlighter: bool,
    page_to_screen: impl Fn(Pos2) -> Pos2,
) -> eframe::egui::Mesh {
    use eframe::egui::{Mesh, Vec2};

    let mut mesh = Mesh::default();
    if points.is_empty() {
        return mesh;
    }

    if points.len() == 1 {
        let center = page_to_screen(points[0].to_pos2());
        let r = if is_highlighter {
            (base_width / 2.0).max(1.0)
        } else {
            (base_width * (0.35 + 0.65 * points[0].pressure) / 2.0).max(1.0)
        };
        add_round_cap(&mut mesh, center, r, color, 12);
        return mesh;
    }

    // Filter titik-titik yang terlalu rapat (< 1.5 pt) untuk mencegah pita melipat diri (ribbon folding overlap)
    let mut filtered: Vec<StrokePoint> = Vec::with_capacity(points.len());
    for pt in points {
        if let Some(last) = filtered.last() {
            let dx = pt.x - last.x;
            let dy = pt.y - last.y;
            if (dx * dx + dy * dy) < 2.0 {
                continue;
            }
        }
        filtered.push(pt.clone());
    }
    if let Some(last) = points.last()
        && filtered
            .last()
            .is_none_or(|p| p.x != last.x || p.y != last.y)
    {
        filtered.push(last.clone());
    }

    let pts_to_use = if filtered.len() >= 2 {
        &filtered[..]
    } else {
        points
    };

    let mut screen_pts = Vec::with_capacity(pts_to_use.len());
    let mut radii = Vec::with_capacity(pts_to_use.len());
    for pt in pts_to_use {
        screen_pts.push(page_to_screen(pt.to_pos2()));
        let r = if is_highlighter {
            (base_width / 2.0).max(1.0)
        } else {
            (base_width * (0.35 + 0.65 * pt.pressure) / 2.0).max(0.5)
        };
        radii.push(r);
    }

    // Compute normals per point
    let mut normals = Vec::with_capacity(screen_pts.len());
    for i in 0..screen_pts.len() {
        let dir = if i == 0 {
            screen_pts[1] - screen_pts[0]
        } else if i == screen_pts.len() - 1 {
            screen_pts[i] - screen_pts[i - 1]
        } else {
            (screen_pts[i + 1] - screen_pts[i - 1]) / 2.0
        };

        let len = dir.length();
        let normal = if len > 1e-4 {
            Vec2::new(-dir.y, dir.x) / len
        } else {
            Vec2::new(0.0, 1.0)
        };
        normals.push(normal);
    }

    // Start semicircle cap (menghadap ke arah luar belakang badan ribbon, tanpa tumpang tindih ke dalam)
    let start_outward = if screen_pts.len() >= 2 {
        let d = screen_pts[0] - screen_pts[1];
        if d.length_sq() > 1e-6 {
            d.normalized()
        } else {
            -normals[0]
        }
    } else {
        -normals[0]
    };
    add_semicircle_cap(&mut mesh, screen_pts[0], radii[0], start_outward, color, 8);

    // Build ribbon vertices
    let base_idx = mesh.vertices.len() as u32;
    for i in 0..screen_pts.len() {
        let p = screen_pts[i];
        let n = normals[i];
        let r = radii[i];

        let left = p + n * r;
        let right = p - n * r;

        mesh.colored_vertex(left, color);
        mesh.colored_vertex(right, color);
    }

    // Build ribbon triangle indices
    for i in 0..(screen_pts.len() - 1) as u32 {
        let l1 = base_idx + i * 2;
        let r1 = l1 + 1;
        let l2 = l1 + 2;
        let r2 = l1 + 3;

        mesh.add_triangle(l1, r1, l2);
        mesh.add_triangle(r1, r2, l2);
    }

    // End semicircle cap (menghadap ke arah luar depan badan ribbon, tanpa tumpang tindih ke dalam)
    let last_idx = screen_pts.len() - 1;
    let end_outward = if screen_pts.len() >= 2 {
        let d = screen_pts[last_idx] - screen_pts[last_idx - 1];
        if d.length_sq() > 1e-6 {
            d.normalized()
        } else {
            normals[last_idx]
        }
    } else {
        normals[last_idx]
    };
    add_semicircle_cap(
        &mut mesh,
        screen_pts[last_idx],
        radii[last_idx],
        end_outward,
        color,
        8,
    );

    mesh
}

fn add_semicircle_cap(
    mesh: &mut eframe::egui::Mesh,
    center: Pos2,
    radius: f32,
    outward_dir: eframe::egui::Vec2,
    color: eframe::egui::Color32,
    segments: usize,
) {
    if outward_dir.length_sq() < 1e-6 {
        return;
    }
    let center_idx = mesh.vertices.len() as u32;
    mesh.colored_vertex(center, color);

    let base_angle = outward_dir.y.atan2(outward_dir.x);
    let start_idx = mesh.vertices.len() as u32;

    for i in 0..=segments {
        let offset_angle =
            -std::f32::consts::FRAC_PI_2 + (i as f32 / segments as f32) * std::f32::consts::PI;
        let angle = base_angle + offset_angle;
        let pos = center + eframe::egui::Vec2::new(angle.cos(), angle.sin()) * radius;
        mesh.colored_vertex(pos, color);
    }

    for i in 0..segments as u32 {
        mesh.add_triangle(center_idx, start_idx + i, start_idx + i + 1);
    }
}

fn add_round_cap(
    mesh: &mut eframe::egui::Mesh,
    center: Pos2,
    radius: f32,
    color: eframe::egui::Color32,
    segments: usize,
) {
    let center_idx = mesh.vertices.len() as u32;
    mesh.colored_vertex(center, color);

    let start_idx = mesh.vertices.len() as u32;
    for i in 0..=segments {
        let angle = (i as f32 / segments as f32) * std::f32::consts::TAU;
        let pos = center + eframe::egui::Vec2::new(angle.cos(), angle.sin()) * radius;
        mesh.colored_vertex(pos, color);
    }

    for i in 0..segments as u32 {
        mesh.add_triangle(center_idx, start_idx + i, start_idx + i + 1);
    }
}

/// 2D Uniform Grid Spatial Index for O(1) / O(k) hit testing on dense pages.
#[derive(Clone, Debug)]
pub struct SpatialGrid {
    cell_size: f32,
    cells: std::collections::HashMap<(i32, i32), Vec<usize>>,
}

impl Default for SpatialGrid {
    fn default() -> Self {
        Self {
            cell_size: 100.0,
            cells: std::collections::HashMap::new(),
        }
    }
}

impl SpatialGrid {
    pub fn new(cell_size: f32) -> Self {
        Self {
            cell_size: cell_size.max(20.0),
            cells: std::collections::HashMap::new(),
        }
    }

    pub fn rebuild(&mut self, objects: &[DrawObject]) {
        self.cells.clear();
        for (idx, obj) in objects.iter().enumerate() {
            let b = object_bounds(obj);
            if !b.is_finite() || b.width() <= 0.0 || b.height() <= 0.0 {
                continue;
            }
            let min_cx = (b.left() / self.cell_size).floor() as i32;
            let max_cx = (b.right() / self.cell_size).floor() as i32;
            let min_cy = (b.top() / self.cell_size).floor() as i32;
            let max_cy = (b.bottom() / self.cell_size).floor() as i32;

            for cx in min_cx..=max_cx {
                for cy in min_cy..=max_cy {
                    self.cells.entry((cx, cy)).or_default().push(idx);
                }
            }
        }
    }

    pub fn query_point(&self, p: Pos2, tolerance: f32) -> Vec<usize> {
        let min_cx = ((p.x - tolerance) / self.cell_size).floor() as i32;
        let max_cx = ((p.x + tolerance) / self.cell_size).floor() as i32;
        let min_cy = ((p.y - tolerance) / self.cell_size).floor() as i32;
        let max_cy = ((p.y + tolerance) / self.cell_size).floor() as i32;

        let mut candidates = Vec::new();
        for cx in min_cx..=max_cx {
            for cy in min_cy..=max_cy {
                if let Some(list) = self.cells.get(&(cx, cy)) {
                    candidates.extend_from_slice(list);
                }
            }
        }
        candidates.sort_unstable();
        candidates.dedup();
        candidates
    }

    pub fn query_rect(&self, r: Rect) -> Vec<usize> {
        let min_cx = (r.left() / self.cell_size).floor() as i32;
        let max_cx = (r.right() / self.cell_size).floor() as i32;
        let min_cy = (r.top() / self.cell_size).floor() as i32;
        let max_cy = (r.bottom() / self.cell_size).floor() as i32;

        let mut candidates = Vec::new();
        for cx in min_cx..=max_cx {
            for cy in min_cy..=max_cy {
                if let Some(list) = self.cells.get(&(cx, cy)) {
                    candidates.extend_from_slice(list);
                }
            }
        }
        candidates.sort_unstable();
        candidates.dedup();
        candidates
    }
}

/// Converts a screen position into a page-local position using the page's top-left screen origin and zoom.
pub fn screen_to_page_point(screen_pos: Pos2, page_origin: Pos2, zoom: f32) -> Pos2 {
    let rel = screen_pos - page_origin;
    Pos2::new(rel.x / zoom, rel.y / zoom)
}

/// Converts a page-local position into a screen position using the page's top-left screen origin and zoom.
pub fn page_to_screen_point(page_pos: Pos2, page_origin: Pos2, zoom: f32) -> Pos2 {
    page_origin + eframe::egui::Vec2::new(page_pos.x * zoom, page_pos.y * zoom)
}

#[derive(Clone, Debug, PartialEq)]
pub struct RecognizedShape {
    pub kind: ShapeKind,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// Menghitung luas poligon berarah menggunakan rumus Shoelace (Gauss Area).
pub fn polygon_area(points: &[Pos2]) -> f32 {
    if points.len() < 3 {
        return 0.0;
    }
    let mut area = 0.0;
    for i in 0..points.len() {
        let j = (i + 1) % points.len();
        area += points[i].x * points[j].y;
        area -= points[j].x * points[i].y;
    }
    area * 0.5
}

/// Penyederhanaan kurva poligon menggunakan algoritma Ramer-Douglas-Peucker (RDP).
pub fn ramer_douglas_peucker(points: &[Pos2], epsilon: f32) -> Vec<Pos2> {
    if points.len() <= 2 {
        return points.to_vec();
    }

    let first = points[0];
    let last = *points.last().unwrap();
    let mut dmax = 0.0;
    let mut index = 0;

    for (i, p) in points.iter().enumerate().take(points.len() - 1).skip(1) {
        let d = point_to_segment_dist(*p, first, last);
        if d > dmax {
            index = i;
            dmax = d;
        }
    }

    if dmax > epsilon {
        let mut rec1 = ramer_douglas_peucker(&points[..=index], epsilon);
        let rec2 = ramer_douglas_peucker(&points[index..], epsilon);
        rec1.pop(); // buang duplikat titik sambungan
        rec1.extend(rec2);
        rec1
    } else {
        vec![first, last]
    }
}

fn extract_polygon_corners(poly_points: &[Pos2], diagonal: f32) -> Vec<Pos2> {
    let epsilon = diagonal * 0.065;
    let mut verts = ramer_douglas_peucker(poly_points, epsilon);
    if verts.len() > 1
        && (*verts.first().unwrap() - *verts.last().unwrap()).length() < diagonal * 0.18
    {
        verts.pop();
    }

    let mut merged: Vec<Pos2> = Vec::with_capacity(verts.len());
    for v in verts {
        if let Some(last) = merged.last_mut()
            && (*last - v).length() < diagonal * 0.12
        {
            *last = Pos2::new((last.x + v.x) * 0.5, (last.y + v.y) * 0.5);
            continue;
        }
        merged.push(v);
    }
    if merged.len() > 2
        && (*merged.first().unwrap() - *merged.last().unwrap()).length() < diagonal * 0.12
    {
        merged.pop();
    }

    loop {
        if merged.len() <= 3 {
            break;
        }
        let n = merged.len();
        let mut remove_idx = None;
        let mut max_cos = 0.90f32;
        for i in 0..n {
            let prev = merged[(i + n - 1) % n];
            let curr = merged[i];
            let next = merged[(i + 1) % n];
            let v1 = curr - prev;
            let v2 = next - curr;
            if v1.length() > 1e-3 && v2.length() > 1e-3 {
                let cos_turn = (v1 / v1.length()).dot(v2 / v2.length());
                if cos_turn > max_cos {
                    max_cos = cos_turn;
                    remove_idx = Some(i);
                }
            }
        }
        if let Some(idx) = remove_idx {
            merged.remove(idx);
        } else {
            break;
        }
    }

    merged
}

fn reduce_corners_to_k(mut corners: Vec<Pos2>, k: usize) -> Vec<Pos2> {
    while corners.len() > k && corners.len() > 3 {
        let n = corners.len();
        let mut best_idx = 0;
        let mut max_cos = f32::NEG_INFINITY;
        for i in 0..n {
            let prev = corners[(i + n - 1) % n];
            let curr = corners[i];
            let next = corners[(i + 1) % n];
            let v1 = curr - prev;
            let v2 = next - curr;
            let cos_turn = if v1.length() > 1e-3 && v2.length() > 1e-3 {
                (v1 / v1.length()).dot(v2 / v2.length())
            } else {
                1.0
            };
            if cos_turn > max_cos {
                max_cos = cos_turn;
                best_idx = i;
            }
        }
        corners.remove(best_idx);
    }
    corners
}

fn classify_triangle_corners(
    corners: &[Pos2],
    min_x: f32,
    min_y: f32,
    max_x: f32,
    max_y: f32,
) -> RecognizedShape {
    let width = (max_x - min_x).max(1.0);
    let height = (max_y - min_y).max(1.0);
    let center = Pos2::new((min_x + max_x) * 0.5, (min_y + max_y) * 0.5);

    // 1. Cek apakah ada sudut siku-siku (~90 derajat, |cos| <= 0.26)
    let mut best_right_idx = None;
    let mut best_abs_cos = 0.26f32;
    for i in 0..3 {
        let p = corners[i];
        let p_prev = corners[(i + 2) % 3];
        let p_next = corners[(i + 1) % 3];
        let v1 = p_prev - p;
        let v2 = p_next - p;
        if v1.length() > 1e-3 && v2.length() > 1e-3 {
            let cos_a = (v1 / v1.length()).dot(v2 / v2.length()).abs();
            if cos_a <= best_abs_cos {
                best_abs_cos = cos_a;
                best_right_idx = Some(i);
            }
        }
    }

    if let Some(ri) = best_right_idx {
        let p_right = corners[ri];
        let p_a = corners[(ri + 2) % 3];
        let p_b = corners[(ri + 1) % 3];
        let v1 = p_a - p_right;
        let v2 = p_b - p_right;
        let u1 = v1 / v1.length().max(1.0);
        let u2 = v2 / v2.length().max(1.0);

        let axis_aligned = (u1.x.abs() <= 0.32 && u2.y.abs() <= 0.32)
            || (u1.y.abs() <= 0.32 && u2.x.abs() <= 0.32);
        if axis_aligned {
            let right_on_left = p_right.x <= center.x;
            let right_on_bottom = p_right.y >= center.y;
            let (rx, rw) = if right_on_left {
                (min_x, width)
            } else {
                (max_x, -width)
            };
            let (ry, rh) = if right_on_bottom {
                (min_y, height)
            } else {
                (max_y, -height)
            };
            return RecognizedShape {
                kind: ShapeKind::RightTriangle,
                x: rx,
                y: ry,
                width: rw,
                height: rh,
            };
        } else {
            let perp = eframe::egui::Vec2::new(-u1.y, u1.x);
            let sign = if v2.dot(perp) >= 0.0 { 1.0 } else { -1.0 };
            let p_b_ortho = p_right + perp * (sign * v2.length());
            let pts = [p_right, p_a, p_b_ortho];
            let t_min_x = pts[0].x.min(pts[1].x).min(pts[2].x);
            let t_min_y = pts[0].y.min(pts[1].y).min(pts[2].y);
            let t_max_x = pts[0].x.max(pts[1].x).max(pts[2].x);
            let t_max_y = pts[0].y.max(pts[1].y).max(pts[2].y);
            let tw = (t_max_x - t_min_x).max(1.0);
            let th = (t_max_y - t_min_y).max(1.0);
            return RecognizedShape {
                kind: ShapeKind::ScaleneTriangle {
                    u0: ((pts[0].x - t_min_x) / tw).clamp(0.0, 1.0),
                    v0: ((pts[0].y - t_min_y) / th).clamp(0.0, 1.0),
                    u1: ((pts[1].x - t_min_x) / tw).clamp(0.0, 1.0),
                    v1: ((pts[1].y - t_min_y) / th).clamp(0.0, 1.0),
                    u2: ((pts[2].x - t_min_x) / tw).clamp(0.0, 1.0),
                    v2: ((pts[2].y - t_min_y) / th).clamp(0.0, 1.0),
                },
                x: t_min_x,
                y: t_min_y,
                width: tw,
                height: th,
            };
        }
    }

    // 2. Cek apakah segitiga sama kaki tegak simetris (ShapeKind::Triangle)
    let mut sorted_y = [corners[0], corners[1], corners[2]];
    sorted_y.sort_by(|a, b| a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal));
    let top = sorted_y[0];
    let b1 = sorted_y[1];
    let b2 = sorted_y[2];

    let is_upright_isosceles = top.y <= min_y + height * 0.20
        && (top.x - center.x).abs() <= width * 0.16
        && b1.y >= max_y - height * 0.22
        && b2.y >= max_y - height * 0.22;

    if is_upright_isosceles {
        return RecognizedShape {
            kind: ShapeKind::Triangle,
            x: min_x,
            y: min_y,
            width,
            height,
        };
    }

    // 3. Segitiga Sembarang (ScaleneTriangle)
    let t_min_x = corners[0].x.min(corners[1].x).min(corners[2].x);
    let t_min_y = corners[0].y.min(corners[1].y).min(corners[2].y);
    let t_max_x = corners[0].x.max(corners[1].x).max(corners[2].x);
    let t_max_y = corners[0].y.max(corners[1].y).max(corners[2].y);
    let tw = (t_max_x - t_min_x).max(1.0);
    let th = (t_max_y - t_min_y).max(1.0);

    RecognizedShape {
        kind: ShapeKind::ScaleneTriangle {
            u0: ((corners[0].x - t_min_x) / tw).clamp(0.0, 1.0),
            v0: ((corners[0].y - t_min_y) / th).clamp(0.0, 1.0),
            u1: ((corners[1].x - t_min_x) / tw).clamp(0.0, 1.0),
            v1: ((corners[1].y - t_min_y) / th).clamp(0.0, 1.0),
            u2: ((corners[2].x - t_min_x) / tw).clamp(0.0, 1.0),
            v2: ((corners[2].y - t_min_y) / th).clamp(0.0, 1.0),
        },
        x: t_min_x,
        y: t_min_y,
        width: tw,
        height: th,
    }
}

/// Fase 8.1: Pengenalan Bentuk Geometri Otomatis (Shape Recognition).
/// Menganalisis urutan titik goresan pena untuk mengenali apakah pengguna menggambar
/// Garis lurus (Line), Lingkaran/Elips (Ellipse), Persegi Panjang (Rectangle),
/// Jajar Genjang (Parallelogram), Segitiga Sama Kaki (Triangle), Segitiga Siku-Siku (RightTriangle),
/// atau Segitiga Sembarang (ScaleneTriangle).
pub fn recognize_shape(points: &[StrokePoint]) -> Option<RecognizedShape> {
    if points.len() < 8 {
        return None;
    }

    let p_first = points.first()?.to_pos2();
    let p_last = points.last()?.to_pos2();

    // 1. Hitung Bounding Box
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;

    for pt in points {
        let p = pt.to_pos2();
        min_x = min_x.min(p.x);
        min_y = min_y.min(p.y);
        max_x = max_x.max(p.x);
        max_y = max_y.max(p.y);
    }

    let width = (max_x - min_x).max(1.0);
    let height = (max_y - min_y).max(1.0);
    let diagonal = (width.powi(2) + height.powi(2)).sqrt();

    // Abaikan jika bentuk terlalu kecil (misalnya titik/titik pena)
    if diagonal < 16.0 {
        return None;
    }

    let close_dist = (p_last - p_first).length();

    // 2. Deteksi Garis Lurus (Line)
    // Syarat:
    // a. Titik awal dan akhir berjauhan (close_dist / diagonal > 0.70)
    // b. Deviasi maksimum setiap titik terhadap segmen p_first -> p_last relatif kecil
    if close_dist / diagonal > 0.70 {
        let mut max_dev = 0.0f32;
        for pt in points {
            let d = point_to_segment_dist(pt.to_pos2(), p_first, p_last);
            if d > max_dev {
                max_dev = d;
            }
        }
        if max_dev / diagonal < 0.14 {
            return Some(RecognizedShape {
                kind: ShapeKind::Line,
                x: p_first.x,
                y: p_first.y,
                width: p_last.x - p_first.x,
                height: p_last.y - p_first.y,
            });
        }
    }

    // 3. Deteksi Bentuk Tertutup (Lingkaran/Elips, Persegi Panjang, Jajar Genjang, Segitiga)
    // Syarat: Titik awal dan akhir saling mendekat
    if close_dist / diagonal <= 0.35 {
        let center = Pos2::new((min_x + max_x) * 0.5, (min_y + max_y) * 0.5);

        // Periksa apakah goresan benar-benar mengelilingi titik pusat satu putaran penuh (~2*PI)
        // untuk menolak coretan zig-zag acak (squiggle) yang kebetulan titik akhirnya dekat titik awal.
        let mut total_angle = 0.0f32;
        let mut abs_angle_sum = 0.0f32;
        for i in 1..points.len() {
            let p0 = points[i - 1].to_pos2() - center;
            let p1 = points[i].to_pos2() - center;
            let a0 = p0.y.atan2(p0.x);
            let a1 = p1.y.atan2(p1.x);
            let mut da = a1 - a0;
            while da > std::f32::consts::PI {
                da -= std::f32::consts::TAU;
            }
            while da < -std::f32::consts::PI {
                da += std::f32::consts::TAU;
            }
            total_angle += da;
            abs_angle_sum += da.abs();
        }

        // Satu putaran tertutup normal memiliki |total_angle| ~ TAU (6.28 rad)
        // dan tidak banyak berbalik arah (abs_angle_sum tidak jauh melebihi |total_angle|).
        let winding = total_angle.abs();
        if !(4.5..=8.0).contains(&winding) || abs_angle_sum > winding * 1.35 {
            return None;
        }

        // a. Uji Lingkaran / Elips
        let rx = width * 0.5;
        let ry = height * 0.5;
        let mut ellipse_radial_errors = Vec::with_capacity(points.len());
        let mut max_ellipse_err = 0.0f32;

        for pt in points {
            let p = pt.to_pos2();
            let norm_x = (p.x - center.x) / rx.max(0.1);
            let norm_y = (p.y - center.y) / ry.max(0.1);
            let dist_from_center_norm = (norm_x.powi(2) + norm_y.powi(2)).sqrt();
            let err = (dist_from_center_norm - 1.0).abs();
            if err > max_ellipse_err {
                max_ellipse_err = err;
            }
            ellipse_radial_errors.push(err);
        }

        let avg_ellipse_err: f32 =
            ellipse_radial_errors.iter().sum::<f32>() / ellipse_radial_errors.len() as f32;
        // Pada persegi panjang, sudut-sudutnya memiliki err = sqrt(2) - 1 = 0.414.
        // Membatasi max_ellipse_err < 0.28 memastikan persegi panjang tidak dikira elips.
        if avg_ellipse_err < 0.14 && max_ellipse_err < 0.28 {
            return Some(RecognizedShape {
                kind: ShapeKind::Ellipse,
                x: min_x,
                y: min_y,
                width,
                height,
            });
        }

        // b. Uji Poligon (Segitiga Siku-Siku / Sembarang / Sama Kaki vs Jajar Genjang vs Persegi Panjang)
        let poly_points: Vec<Pos2> = points.iter().map(|p| p.to_pos2()).collect();
        let corners = extract_polygon_corners(&poly_points, diagonal);
        let poly_area = polygon_area(&poly_points).abs();
        let bbox_area = width * height;
        let fill_ratio = poly_area / bbox_area.max(1.0);

        if corners.len() == 3 {
            return Some(classify_triangle_corners(
                &corners, min_x, min_y, max_x, max_y,
            ));
        }

        if corners.len() == 4 || corners.len() == 5 {
            if fill_ratio < 0.56 {
                let tri_corners = reduce_corners_to_k(corners.clone(), 3);
                if tri_corners.len() == 3 {
                    return Some(classify_triangle_corners(
                        &tri_corners,
                        min_x,
                        min_y,
                        max_x,
                        max_y,
                    ));
                }
            }

            let quad = reduce_corners_to_k(corners, 4);
            if quad.len() == 4 {
                let e0 = quad[1] - quad[0];
                let e1 = quad[2] - quad[1];
                let e2 = quad[3] - quad[2];
                let e3 = quad[0] - quad[3];

                if e0.length() > 1e-3
                    && e1.length() > 1e-3
                    && e2.length() > 1e-3
                    && e3.length() > 1e-3
                {
                    let u0 = e0 / e0.length();
                    let u1 = e1 / e1.length();
                    let u2 = e2 / e2.length();
                    let u3 = e3 / e3.length();

                    let parallel_02 = u0.dot(u2).abs();
                    let parallel_13 = u1.dot(u3).abs();
                    let avg_adj_cos =
                        (u0.dot(u1).abs() + u1.dot(u2).abs() + u2.dot(u3).abs() + u3.dot(u0).abs())
                            * 0.25;

                    // Jajar Genjang: pasangan sisi berhadapan sejajar (parallel >= 0.82)
                    // dan sudut antar sisi bersebelahan miring (bukan 90 derajat tegak lurus).
                    if parallel_02 >= 0.82
                        && parallel_13 >= 0.82
                        && (0.18..=0.90).contains(&avg_adj_cos)
                    {
                        let mut by_y = quad.clone();
                        by_y.sort_by(|a, b| {
                            a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal)
                        });
                        let top_avg_x = (by_y[0].x + by_y[1].x) * 0.5;
                        let bot_avg_x = (by_y[2].x + by_y[3].x) * 0.5;
                        let (px, pw) = if top_avg_x >= bot_avg_x {
                            (min_x, width)
                        } else {
                            (max_x, -width)
                        };
                        return Some(RecognizedShape {
                            kind: ShapeKind::Parallelogram,
                            x: px,
                            y: min_y,
                            width: pw,
                            height,
                        });
                    }
                }
            }

            if fill_ratio >= 0.56 {
                return Some(RecognizedShape {
                    kind: ShapeKind::Rectangle,
                    x: min_x,
                    y: min_y,
                    width,
                    height,
                });
            }
        }
    }

    None
}

fn lerp_stroke_point(a: &StrokePoint, b: &StrokePoint, t: f32) -> StrokePoint {
    let t = t.clamp(0.0, 1.0);
    StrokePoint {
        x: a.x + (b.x - a.x) * t,
        y: a.y + (b.y - a.y) * t,
        pressure: a.pressure + (b.pressure - a.pressure) * t,
    }
}

fn segment_circle_intersections(
    p0: Pos2,
    p1: Pos2,
    center: Pos2,
    radius: f32,
) -> Option<(f32, f32)> {
    let d = p1 - p0;
    let f = p0 - center;
    let a = d.length_sq();
    if a < 1e-8 {
        return None;
    }
    let b = 2.0 * f.dot(d);
    let c = f.length_sq() - radius * radius;
    let discriminant = b * b - 4.0 * a * c;
    if discriminant < 0.0 {
        return None;
    }
    let sqrt_disc = discriminant.sqrt();
    let t1 = (-b - sqrt_disc) / (2.0 * a);
    let t2 = (-b + sqrt_disc) / (2.0 * a);
    Some((t1, t2))
}

/// Fase 8.2: Eraser Parsial (Stroke Splitter).
/// Memotong goresan (`points`) dengan lingkaran penghapus (`eraser_pos`, `radius`).
/// Mengembalikan `None` jika penghapus sama sekali tidak menyentuh goresan.
/// Mengembalikan `Some(segments)` berisi daftar segmen goresan yang tersisa (bisa kosong jika habis terhapus).
pub fn split_stroke_by_eraser(
    points: &[StrokePoint],
    eraser_pos: Pos2,
    radius: f32,
) -> Option<Vec<Vec<StrokePoint>>> {
    if points.is_empty() || radius <= 0.0 {
        return None;
    }

    let r_sq = radius * radius;
    if points.len() == 1 {
        if (points[0].to_pos2() - eraser_pos).length_sq() <= r_sq {
            return Some(Vec::new());
        } else {
            return None;
        }
    }

    let mut hit = false;
    let mut segments: Vec<Vec<StrokePoint>> = Vec::new();
    let mut current_seg: Vec<StrokePoint> = Vec::new();

    let first_in = (points[0].to_pos2() - eraser_pos).length_sq() <= r_sq;
    if !first_in {
        current_seg.push(points[0].clone());
    } else {
        hit = true;
    }

    for i in 0..(points.len() - 1) {
        let a = &points[i];
        let b = &points[i + 1];
        let pa = a.to_pos2();
        let pb = b.to_pos2();
        let in_a = (pa - eraser_pos).length_sq() <= r_sq;
        let in_b = (pb - eraser_pos).length_sq() <= r_sq;

        match (in_a, in_b) {
            (true, true) => {
                hit = true;
            }
            (false, true) => {
                hit = true;
                if let Some((t1, _)) = segment_circle_intersections(pa, pb, eraser_pos, radius) {
                    let entry = lerp_stroke_point(a, b, t1);
                    if current_seg
                        .last()
                        .is_none_or(|last| (last.to_pos2() - entry.to_pos2()).length_sq() > 1e-4)
                    {
                        current_seg.push(entry);
                    }
                }
                if current_seg.len() >= 2 {
                    segments.push(std::mem::take(&mut current_seg));
                } else {
                    current_seg.clear();
                }
            }
            (true, false) => {
                hit = true;
                current_seg.clear();
                if let Some((_, t2)) = segment_circle_intersections(pa, pb, eraser_pos, radius) {
                    let exit = lerp_stroke_point(a, b, t2);
                    current_seg.push(exit);
                }
                if current_seg
                    .last()
                    .is_none_or(|last| (last.to_pos2() - pb).length_sq() > 1e-4)
                {
                    current_seg.push(b.clone());
                }
            }
            (false, false) => {
                if let Some((t1, t2)) = segment_circle_intersections(pa, pb, eraser_pos, radius)
                    && t1 > 0.0
                    && t2 < 1.0
                    && t1 < t2
                {
                    // Lingkaran penghapus memotong bagian tengah segmen lurus panjang
                    hit = true;
                    let entry = lerp_stroke_point(a, b, t1);
                    if current_seg
                        .last()
                        .is_none_or(|last| (last.to_pos2() - entry.to_pos2()).length_sq() > 1e-4)
                    {
                        current_seg.push(entry);
                    }
                    if current_seg.len() >= 2 {
                        segments.push(std::mem::take(&mut current_seg));
                    } else {
                        current_seg.clear();
                    }

                    let exit = lerp_stroke_point(a, b, t2);
                    current_seg.push(exit);
                    if (current_seg.last().unwrap().to_pos2() - pb).length_sq() > 1e-4 {
                        current_seg.push(b.clone());
                    }
                } else {
                    current_seg.push(b.clone());
                }
            }
        }
    }

    if !hit {
        return None;
    }

    if current_seg.len() >= 2 {
        segments.push(current_seg);
    }

    Some(segments)
}

/// Fase 8.5: Palm Rejection — menyaring sentuhan telapak tangan saat menulis dengan pena.
/// Menolak input jika terdeteksi multi-touch aktif saat goresan berlangsung atau
/// jika terjadi lompatan koordinat mendadak yang tidak wajar (> 180 pt dalam < 80 ms)
/// akibat telapak tangan menyentuh sisi lain kanvas.
pub fn should_reject_palm_input(
    existing_points: &[StrokePoint],
    candidate: Pos2,
    dt_secs: f32,
    multi_touch_active: bool,
) -> bool {
    if multi_touch_active {
        return true;
    }
    if let Some(last) = existing_points.last() {
        let dist = (candidate - last.to_pos2()).length();
        if dist > 180.0 && dt_secs < 0.08 {
            return true;
        }
    }
    false
}

/// Fase 8.6: Klasifikasi satu karakter tulisan tangan dari kumpulan goresan (glyph cluster)
/// menggunakan fitur geometri & topologi murni Rust (Zero External Runtime).
fn classify_glyph_strokes(strokes: &[&[StrokePoint]], bounds: Rect) -> char {
    let w = bounds.width().max(1.0);
    let h = bounds.height().max(1.0);
    let aspect = w / h;
    let num_strokes = strokes.len();

    // Titik sangat kecil -> titik baca '.'
    if w < 6.0 && h < 6.0 {
        return '.';
    }

    // Goresan tunggal sangat ramping vertikal -> 'I'
    if aspect < 0.28 {
        return 'I';
    }

    // Goresan tunggal sangat datar horizontal -> '-'
    if h < 8.0 && aspect > 2.5 {
        return '-';
    }

    if num_strokes == 1 {
        let pts = strokes[0];
        if pts.len() >= 2 {
            let first = pts.first().unwrap().to_pos2();
            let last = pts.last().unwrap().to_pos2();
            let diag = (w * w + h * h).sqrt().max(1.0);
            let close_ratio = (first - last).length() / diag;

            // Kurva tertutup (awal dan akhir berdekatan) -> 'O'
            if close_ratio < 0.28 && pts.len() >= 5 {
                return 'O';
            }

            // Cek bentuk sudut 'L', 'V', 'U', 'C', 'Z', 'S'
            let norm_first = Pos2::new((first.x - bounds.min.x) / w, (first.y - bounds.min.y) / h);
            let norm_last = Pos2::new((last.x - bounds.min.x) / w, (last.y - bounds.min.y) / h);

            // Mulai di kiri-atas, berakhir di kanan-bawah, dengan titik sudut di kiri-bawah -> 'L'
            let has_bottom_left = pts.iter().any(|p| {
                let nx = (p.x - bounds.min.x) / w;
                let ny = (p.y - bounds.min.y) / h;
                nx < 0.35 && ny > 0.75
            });
            let has_top_right = pts.iter().any(|p| {
                let nx = (p.x - bounds.min.x) / w;
                let ny = (p.y - bounds.min.y) / h;
                nx > 0.65 && ny < 0.30
            });

            if norm_first.x < 0.35
                && norm_first.y < 0.30
                && norm_last.x > 0.65
                && norm_last.y > 0.70
                && has_bottom_left
                && !has_top_right
            {
                return 'L';
            }

            // Mulai di kiri-atas, turun ke tengah-bawah, naik ke kanan-atas -> 'V' atau 'U'
            if norm_first.y < 0.35
                && norm_last.y < 0.35
                && norm_first.x < 0.35
                && norm_last.x > 0.65
            {
                return 'V';
            }

            // Mulai di kanan-atas, melengkung ke kiri, berakhir di kanan-bawah -> 'C'
            if norm_first.x > 0.60
                && norm_first.y < 0.35
                && norm_last.x > 0.60
                && norm_last.y > 0.65
            {
                let has_right_mid = pts.iter().any(|p| {
                    let nx = (p.x - bounds.min.x) / w;
                    let ny = (p.y - bounds.min.y) / h;
                    nx > 0.65 && (0.35..=0.65).contains(&ny)
                });
                if !has_right_mid {
                    return 'C';
                }
                return 'S';
            }

            if has_top_right && has_bottom_left {
                return 'Z';
            }
        }
        return 'I';
    }

    if num_strokes == 2 {
        // Cek orientasi masing-masing goresan (horizontal vs vertikal vs diagonal)
        let mut horiz = 0;
        let mut vert = 0;
        let mut diag = 0;
        let mut horiz_y = 0.5;
        for s in strokes {
            if let (Some(f), Some(l)) = (s.first(), s.last()) {
                let dx = (l.x - f.x).abs();
                let dy = (l.y - f.y).abs();
                if dx > dy * 1.6 {
                    horiz += 1;
                    horiz_y = (((f.y + l.y) * 0.5) - bounds.min.y) / h;
                } else if dy > dx * 1.6 {
                    vert += 1;
                } else {
                    diag += 1;
                }
            }
        }
        if horiz == 1 && vert == 1 {
            if horiz_y < 0.35 {
                return 'T';
            } else if horiz_y > 0.70 {
                return 'L';
            } else {
                return '+';
            }
        }
        if diag == 2 {
            return 'X';
        }
        return 'D';
    }

    if num_strokes == 3 {
        let mut horiz = 0;
        let mut vert = 0;
        for s in strokes {
            if let (Some(f), Some(l)) = (s.first(), s.last()) {
                let dx = (l.x - f.x).abs();
                let dy = (l.y - f.y).abs();
                if dx > dy * 1.5 {
                    horiz += 1;
                } else if dy > dx * 1.5 {
                    vert += 1;
                }
            }
        }
        if vert == 2 && horiz == 1 {
            return 'H';
        }
        if vert == 1 && horiz == 2 {
            return 'F';
        }
        return 'A';
    }

    // 4+ strokes in one character cell -> 'E' or 'M'
    let mut horiz = 0;
    for s in strokes {
        if let (Some(f), Some(l)) = (s.first(), s.last())
            && (l.x - f.x).abs() > (l.y - f.y).abs() * 1.4
        {
            horiz += 1;
        }
    }
    if horiz >= 3 { 'E' } else { 'M' }
}

/// Fase 8.6: Ekstraksi OCR tulisan tangan murni Rust dari objek halaman untuk indeks pencarian.
/// Mengelompokkan goresan pena ke dalam baris dan karakter, mengenali karakter per klaster,
/// serta menggabungkannya secara otomatis.
pub fn recognize_handwriting_from_strokes(objects: &[DrawObject]) -> String {
    struct StrokeItem<'a> {
        pts: &'a [StrokePoint],
        bounds: Rect,
    }

    let mut items: Vec<StrokeItem<'_>> = Vec::new();
    for obj in objects {
        if let DrawObject::Stroke {
            points,
            is_highlighter,
            ..
        } = obj
        {
            if *is_highlighter || points.len() < 2 {
                continue;
            }
            let mut min_x = f32::INFINITY;
            let mut min_y = f32::INFINITY;
            let mut max_x = f32::NEG_INFINITY;
            let mut max_y = f32::NEG_INFINITY;
            for p in points {
                min_x = min_x.min(p.x);
                min_y = min_y.min(p.y);
                max_x = max_x.max(p.x);
                max_y = max_y.max(p.y);
            }
            items.push(StrokeItem {
                pts: points.as_slice(),
                bounds: Rect::from_min_max(Pos2::new(min_x, min_y), Pos2::new(max_x, max_y)),
            });
        }
    }

    if items.is_empty() {
        return String::new();
    }

    // Urutkan berdasarkan posisi vertikal (Y) terlebih dahulu untuk pengelompokan baris
    items.sort_by(|a, b| {
        a.bounds
            .center()
            .y
            .partial_cmp(&b.bounds.center().y)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut lines: Vec<(Rect, Vec<StrokeItem<'_>>)> = Vec::new();
    for item in items {
        let cy = item.bounds.center().y;
        if let Some((line_rect, line_items)) = lines.iter_mut().find(|(r, _)| {
            let overlap_y =
                item.bounds.min.y <= r.max.y + 8.0 && item.bounds.max.y >= r.min.y - 8.0;
            let close_cy = (cy - r.center().y).abs() <= r.height().max(24.0) * 0.65;
            overlap_y || close_cy
        }) {
            *line_rect = line_rect.union(item.bounds);
            line_items.push(item);
        } else {
            lines.push((item.bounds, vec![item]));
        }
    }

    // Urutkan baris dari atas ke bawah
    lines.sort_by(|a, b| {
        a.0.min
            .y
            .partial_cmp(&b.0.min.y)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut result_lines: Vec<String> = Vec::new();

    for (line_rect, mut line_items) in lines {
        let line_h = line_rect.height().max(20.0);
        // Urutkan goresan di dalam baris dari kiri ke kanan
        line_items.sort_by(|a, b| {
            a.bounds
                .min
                .x
                .partial_cmp(&b.bounds.min.x)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut clusters: Vec<(Rect, Vec<&[StrokePoint]>)> = Vec::new();
        for item in line_items {
            if let Some((c_rect, c_strokes)) = clusters.last_mut() {
                let gap_x = item.bounds.min.x - c_rect.max.x;
                // Jika goresan saling tumpang tindih atau sangat rapat secara horizontal, gabungkan ke karakter yang sama
                if gap_x <= line_h * 0.12 {
                    *c_rect = c_rect.union(item.bounds);
                    c_strokes.push(item.pts);
                    continue;
                }
            }
            clusters.push((item.bounds, vec![item.pts]));
        }

        let mut line_str = String::new();
        let mut prev_max_x: Option<f32> = None;
        for (c_rect, c_strokes) in clusters {
            if let Some(px) = prev_max_x {
                let gap = c_rect.min.x - px;
                if gap > line_h * 0.65 {
                    line_str.push(' ');
                }
            }
            let ch = classify_glyph_strokes(&c_strokes, c_rect);
            line_str.push(ch);
            prev_max_x = Some(c_rect.max.x);
        }

        if !line_str.is_empty() {
            result_lines.push(line_str);
        }
    }

    result_lines.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ColorData, ShapeKind, StrokePoint};
    use eframe::egui::Color32;

    fn dummy_color() -> ColorData {
        ColorData {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
        }
    }

    #[test]
    fn test_point_in_polygon() {
        // Square 0,0 to 100,100
        let poly = vec![
            Pos2::new(0.0, 0.0),
            Pos2::new(100.0, 0.0),
            Pos2::new(100.0, 100.0),
            Pos2::new(0.0, 100.0),
        ];

        // Inside
        assert!(point_in_polygon(Pos2::new(50.0, 50.0), &poly));
        // Outside
        assert!(!point_in_polygon(Pos2::new(150.0, 50.0), &poly));
        assert!(!point_in_polygon(Pos2::new(-10.0, 50.0), &poly));
        assert!(!point_in_polygon(Pos2::new(50.0, 150.0), &poly));

        // Concave L-shaped polygon
        let concave = vec![
            Pos2::new(0.0, 0.0),
            Pos2::new(100.0, 0.0),
            Pos2::new(100.0, 50.0),
            Pos2::new(50.0, 50.0),
            Pos2::new(50.0, 100.0),
            Pos2::new(0.0, 100.0),
        ];
        // In the lower left arm of L
        assert!(point_in_polygon(Pos2::new(25.0, 75.0), &concave));
        // In the cutout top-right
        assert!(!point_in_polygon(Pos2::new(75.0, 75.0), &concave));

        // Fewer than 3 points
        let empty_poly: Vec<Pos2> = vec![];
        assert!(!point_in_polygon(Pos2::new(10.0, 10.0), &empty_poly));
        let two_pts = vec![Pos2::new(0.0, 0.0), Pos2::new(10.0, 10.0)];
        assert!(!point_in_polygon(Pos2::new(5.0, 5.0), &two_pts));
    }

    #[test]
    fn test_segments_cross() {
        // Crossing lines
        let a = Pos2::new(0.0, 0.0);
        let b = Pos2::new(10.0, 10.0);
        let c = Pos2::new(0.0, 10.0);
        let d = Pos2::new(10.0, 0.0);
        assert!(segments_cross(a, b, c, d));

        // Parallel lines
        let c_par = Pos2::new(0.0, 2.0);
        let d_par = Pos2::new(10.0, 12.0);
        assert!(!segments_cross(a, b, c_par, d_par));

        // Non-intersecting disjoint lines
        let c_dis = Pos2::new(20.0, 0.0);
        let d_dis = Pos2::new(20.0, 10.0);
        assert!(!segments_cross(a, b, c_dis, d_dis));
    }

    #[test]
    fn test_point_to_segment_dist() {
        let a = Pos2::new(0.0, 0.0);
        let b = Pos2::new(10.0, 0.0);

        // Projected directly onto segment
        assert!((point_to_segment_dist(Pos2::new(5.0, 3.0), a, b) - 3.0).abs() < 1e-4);

        // Outside endpoint A
        assert!((point_to_segment_dist(Pos2::new(-4.0, 0.0), a, b) - 4.0).abs() < 1e-4);

        // Outside endpoint B
        assert!((point_to_segment_dist(Pos2::new(13.0, 4.0), a, b) - 5.0).abs() < 1e-4);

        // Zero-length segment (a == b)
        let zero_seg = Pos2::new(2.0, 2.0);
        assert!(
            (point_to_segment_dist(Pos2::new(5.0, 6.0), zero_seg, zero_seg) - 5.0).abs() < 1e-4
        );
    }

    #[test]
    fn test_object_bounds() {
        // Empty stroke
        let empty_stroke = DrawObject::Stroke {
            id: "1".into(),
            points: vec![],
            color: dummy_color(),
            width: 4.0,
            is_highlighter: false,
        };
        assert_eq!(object_bounds(&empty_stroke), Rect::NOTHING);

        // Stroke with points and width padding
        let stroke = DrawObject::Stroke {
            id: "2".into(),
            points: vec![
                StrokePoint {
                    x: 10.0,
                    y: 10.0,
                    pressure: 1.0,
                },
                StrokePoint {
                    x: 50.0,
                    y: 60.0,
                    pressure: 1.0,
                },
            ],
            color: dummy_color(),
            width: 6.0,
            is_highlighter: false,
        };
        let b = object_bounds(&stroke);
        // padding = width/2 = 3.0
        assert_eq!(b.min, Pos2::new(7.0, 7.0));
        assert_eq!(b.max, Pos2::new(53.0, 63.0));

        // Shape bounds
        let shape = DrawObject::Shape {
            id: "3".into(),
            shape_type: ShapeKind::Rectangle,
            x: 20.0,
            y: 30.0,
            width: 40.0,
            height: 50.0,
            color: dummy_color(),
            stroke_width: 2.0,
            fill: None,
        };
        let sb = object_bounds(&shape);
        assert_eq!(sb.min, Pos2::new(19.0, 29.0));
        assert_eq!(sb.max, Pos2::new(61.0, 81.0));
    }

    #[test]
    fn test_combined_bounds() {
        assert!(combined_bounds(&[]).is_none());

        let obj1 = DrawObject::Shape {
            id: "1".into(),
            shape_type: ShapeKind::Rectangle,
            x: 0.0,
            y: 0.0,
            width: 50.0,
            height: 50.0,
            color: dummy_color(),
            stroke_width: 2.0,
            fill: None,
        };
        let obj2 = DrawObject::Shape {
            id: "2".into(),
            shape_type: ShapeKind::Rectangle,
            x: 80.0,
            y: 80.0,
            width: 20.0,
            height: 20.0,
            color: dummy_color(),
            stroke_width: 2.0,
            fill: None,
        };

        let cb = combined_bounds(&[&obj1, &obj2]).expect("Should have combined bounds");
        assert_eq!(cb.min, Pos2::new(-1.0, -1.0));
        assert_eq!(cb.max, Pos2::new(101.0, 101.0));
    }

    #[test]
    fn test_hit_test_object() {
        let stroke = DrawObject::Stroke {
            id: "1".into(),
            points: vec![
                StrokePoint {
                    x: 0.0,
                    y: 0.0,
                    pressure: 1.0,
                },
                StrokePoint {
                    x: 100.0,
                    y: 0.0,
                    pressure: 1.0,
                },
            ],
            color: dummy_color(),
            width: 4.0,
            is_highlighter: false,
        };

        // Hit near the line
        assert!(hit_test_object(&stroke, Pos2::new(50.0, 2.0), 5.0));
        // Miss far from the line
        assert!(!hit_test_object(&stroke, Pos2::new(50.0, 30.0), 5.0));

        let rect_filled = DrawObject::Shape {
            id: "2".into(),
            shape_type: ShapeKind::Rectangle,
            x: 10.0,
            y: 10.0,
            width: 80.0,
            height: 80.0,
            color: dummy_color(),
            stroke_width: 2.0,
            fill: Some(dummy_color()),
        };
        // Inside filled shape
        assert!(hit_test_object(&rect_filled, Pos2::new(50.0, 50.0), 5.0));
        // Outside filled shape
        assert!(!hit_test_object(&rect_filled, Pos2::new(120.0, 50.0), 5.0));
    }

    #[test]
    fn test_translate_and_scale_object() {
        let mut obj = DrawObject::Shape {
            id: "1".into(),
            shape_type: ShapeKind::Rectangle,
            x: 10.0,
            y: 10.0,
            width: 20.0,
            height: 30.0,
            color: dummy_color(),
            stroke_width: 2.0,
            fill: None,
        };

        translate_object(&mut obj, 15.0, -5.0);
        if let DrawObject::Shape { x, y, .. } = obj {
            assert_eq!(x, 25.0);
            assert_eq!(y, 5.0);
        } else {
            panic!("Expected shape");
        }

        scale_object(&mut obj, Pos2::new(0.0, 0.0), 2.0, 2.0);
        if let DrawObject::Shape {
            x,
            y,
            width,
            height,
            ..
        } = obj
        {
            assert_eq!(x, 50.0);
            assert_eq!(y, 10.0);
            assert_eq!(width, 40.0);
            assert_eq!(height, 60.0);
        } else {
            panic!("Expected shape");
        }
    }

    #[test]
    fn test_image_object_geometry() {
        let mut img = DrawObject::Image {
            id: "img-test".to_string(),
            asset_id: "test.png".to_string(),
            x: 50.0,
            y: 60.0,
            width: 200.0,
            height: 150.0,
            rotation: 0.0,
            lock_aspect: true,
        };

        let b = object_bounds(&img);
        assert_eq!(b.min, Pos2::new(50.0, 60.0));
        assert_eq!(b.max, Pos2::new(250.0, 210.0));

        assert!(hit_test_object(&img, Pos2::new(100.0, 100.0), 5.0));
        assert!(!hit_test_object(&img, Pos2::new(300.0, 300.0), 5.0));

        translate_object(&mut img, 20.0, -10.0);
        if let DrawObject::Image { x, y, .. } = img {
            assert_eq!(x, 70.0);
            assert_eq!(y, 50.0);
        }

        scale_object(&mut img, Pos2::new(70.0, 50.0), 2.0, 2.0);
        if let DrawObject::Image { width, height, .. } = img {
            assert_eq!(width, 400.0);
            assert_eq!(height, 300.0);
        }
    }

    #[test]
    fn test_flip_and_rotate_object() {
        let mut stroke = DrawObject::Stroke {
            id: "s1".into(),
            points: vec![
                StrokePoint {
                    x: 10.0,
                    y: 20.0,
                    pressure: 1.0,
                },
                StrokePoint {
                    x: 30.0,
                    y: 40.0,
                    pressure: 1.0,
                },
            ],
            color: dummy_color(),
            width: 2.0,
            is_highlighter: false,
        };

        // Flip horizontal around center_x = 50.0:
        // 10 -> 2*50 - 10 = 90
        // 30 -> 2*50 - 30 = 70
        flip_object_horizontal(&mut stroke, 50.0);
        if let DrawObject::Stroke { points, .. } = &stroke {
            assert_eq!(points[0].x, 90.0);
            assert_eq!(points[1].x, 70.0);
        }

        // Flip vertical around center_y = 50.0:
        // 20 -> 2*50 - 20 = 80
        // 40 -> 2*50 - 40 = 60
        flip_object_vertical(&mut stroke, 50.0);
        if let DrawObject::Stroke { points, .. } = &stroke {
            assert_eq!(points[0].y, 80.0);
            assert_eq!(points[1].y, 60.0);
        }

        // Rotate 90 degrees (pi / 2) around (0, 0):
        // (x, y) -> (-y, x)
        let mut pt_obj = DrawObject::Stroke {
            id: "s2".into(),
            points: vec![StrokePoint {
                x: 10.0,
                y: 0.0,
                pressure: 1.0,
            }],
            color: dummy_color(),
            width: 2.0,
            is_highlighter: false,
        };
        rotate_object(
            &mut pt_obj,
            Pos2::new(0.0, 0.0),
            std::f32::consts::FRAC_PI_2,
        );
        if let DrawObject::Stroke { points, .. } = &pt_obj {
            assert!((points[0].x - 0.0).abs() < 1e-4);
            assert!((points[0].y - 10.0).abs() < 1e-4);
        }
    }

    #[test]
    fn test_object_in_lasso() {
        let poly = vec![
            Pos2::new(0.0, 0.0),
            Pos2::new(100.0, 0.0),
            Pos2::new(100.0, 100.0),
            Pos2::new(0.0, 100.0),
        ];

        let inside_stroke = DrawObject::Stroke {
            id: "1".into(),
            points: vec![
                StrokePoint {
                    x: 20.0,
                    y: 20.0,
                    pressure: 1.0,
                },
                StrokePoint {
                    x: 40.0,
                    y: 40.0,
                    pressure: 1.0,
                },
            ],
            color: dummy_color(),
            width: 2.0,
            is_highlighter: false,
        };
        assert!(object_in_lasso(&inside_stroke, &poly));

        let crossing_stroke = DrawObject::Stroke {
            id: "2".into(),
            points: vec![
                StrokePoint {
                    x: -20.0,
                    y: 50.0,
                    pressure: 1.0,
                },
                StrokePoint {
                    x: 120.0,
                    y: 50.0,
                    pressure: 1.0,
                },
            ],
            color: dummy_color(),
            width: 2.0,
            is_highlighter: false,
        };
        assert!(object_in_lasso(&crossing_stroke, &poly));

        let outside_stroke = DrawObject::Stroke {
            id: "3".into(),
            points: vec![
                StrokePoint {
                    x: 200.0,
                    y: 200.0,
                    pressure: 1.0,
                },
                StrokePoint {
                    x: 220.0,
                    y: 220.0,
                    pressure: 1.0,
                },
            ],
            color: dummy_color(),
            width: 2.0,
            is_highlighter: false,
        };
        assert!(!object_in_lasso(&outside_stroke, &poly));
    }

    #[test]
    fn test_smooth_stroke_points() {
        let pts = vec![
            StrokePoint {
                x: 0.0,
                y: 0.0,
                pressure: 0.5,
            },
            StrokePoint {
                x: 10.0,
                y: 20.0,
                pressure: 0.7,
            },
            StrokePoint {
                x: 30.0,
                y: 25.0,
                pressure: 0.9,
            },
            StrokePoint {
                x: 60.0,
                y: 10.0,
                pressure: 0.4,
            },
        ];

        let smoothed = smooth_stroke_points(&pts, 4);
        assert!(smoothed.len() > pts.len());

        // First point must match exactly
        assert_eq!(smoothed[0].x, pts[0].x);
        assert_eq!(smoothed[0].y, pts[0].y);

        // Last point must match exactly
        let last_idx = smoothed.len() - 1;
        assert_eq!(smoothed[last_idx].x, pts[pts.len() - 1].x);
        assert_eq!(smoothed[last_idx].y, pts[pts.len() - 1].y);

        // No NaNs
        for p in &smoothed {
            assert!(p.x.is_finite());
            assert!(p.y.is_finite());
            assert!(p.pressure.is_finite());
        }
    }

    #[test]
    fn test_tessellate_stroke() {
        let pts = vec![
            StrokePoint {
                x: 0.0,
                y: 0.0,
                pressure: 0.5,
            },
            StrokePoint {
                x: 50.0,
                y: 50.0,
                pressure: 1.0,
            },
        ];

        let mesh = tessellate_stroke(&pts, 4.0, Color32::RED, |p| p);
        assert!(!mesh.vertices.is_empty());
        assert!(!mesh.indices.is_empty());
        assert_eq!(mesh.indices.len() % 3, 0); // Must be valid triangles
    }

    #[test]
    fn test_spatial_grid_matches_brute_force() {
        let objects = vec![
            DrawObject::Shape {
                id: "1".into(),
                shape_type: ShapeKind::Rectangle,
                x: 10.0,
                y: 10.0,
                width: 30.0,
                height: 30.0,
                color: dummy_color(),
                stroke_width: 2.0,
                fill: None,
            },
            DrawObject::Shape {
                id: "2".into(),
                shape_type: ShapeKind::Rectangle,
                x: 250.0,
                y: 250.0,
                width: 40.0,
                height: 40.0,
                color: dummy_color(),
                stroke_width: 2.0,
                fill: None,
            },
            DrawObject::Shape {
                id: "3".into(),
                shape_type: ShapeKind::Rectangle,
                x: 500.0,
                y: 500.0,
                width: 20.0,
                height: 20.0,
                color: dummy_color(),
                stroke_width: 2.0,
                fill: None,
            },
        ];

        let mut grid = SpatialGrid::new(100.0);
        grid.rebuild(&objects);

        // Query point near object 1
        let query_pt = Pos2::new(25.0, 25.0);
        let tol = 10.0;
        let candidates = grid.query_point(query_pt, tol);

        // Brute force candidates
        let mut brute_force = Vec::new();
        for (i, obj) in objects.iter().enumerate() {
            let b = object_bounds(obj);
            if b.expand(tol).contains(query_pt) {
                brute_force.push(i);
            }
        }

        for idx in brute_force {
            assert!(
                candidates.contains(&idx),
                "SpatialGrid candidate query must include candidate {idx}"
            );
        }
    }

    #[test]
    fn test_smooth_stroke_points_edge_cases() {
        let empty: Vec<StrokePoint> = Vec::new();
        assert_eq!(smooth_stroke_points(&empty, 4).len(), 0);

        let single = vec![StrokePoint {
            x: 10.0,
            y: 10.0,
            pressure: 1.0,
        }];
        assert_eq!(smooth_stroke_points(&single, 4).len(), 1);

        let pair = vec![
            StrokePoint {
                x: 0.0,
                y: 0.0,
                pressure: 0.5,
            },
            StrokePoint {
                x: 10.0,
                y: 10.0,
                pressure: 0.8,
            },
        ];
        assert_eq!(smooth_stroke_points(&pair, 4).len(), 2);
    }

    #[test]
    fn test_spatial_grid_empty_query() {
        let objects: Vec<DrawObject> = Vec::new();
        let mut grid = SpatialGrid::new(100.0);
        grid.rebuild(&objects);
        let res = grid.query_point(Pos2::new(100.0, 100.0), 10.0);
        assert!(res.is_empty());
    }

    #[test]
    fn test_continuous_coordinate_conversion() {
        // Page 1 origin at screen (100, 50), zoom = 1.5
        let origin_p1 = Pos2::new(100.0, 50.0);
        let zoom = 1.5;
        let screen_pt = Pos2::new(250.0, 200.0);
        let page_pt = screen_to_page_point(screen_pt, origin_p1, zoom);
        // rel_x = 150 / 1.5 = 100, rel_y = 150 / 1.5 = 100
        assert_eq!(page_pt, Pos2::new(100.0, 100.0));
        let roundtrip_screen = page_to_screen_point(page_pt, origin_p1, zoom);
        assert_eq!(roundtrip_screen, screen_pt);

        // Page 2 stacked vertically below page 1 (height 800 * 1.5 + 24 margin = 1224)
        let origin_p2 = Pos2::new(100.0, 50.0 + 1224.0);
        let screen_pt_p2 = Pos2::new(100.0 + 75.0, origin_p2.y + 150.0);
        let page_pt_p2 = screen_to_page_point(screen_pt_p2, origin_p2, zoom);
        assert_eq!(page_pt_p2, Pos2::new(50.0, 100.0));
    }

    #[test]
    fn test_recognize_shape_line() {
        // Line from (10, 10) to (110, 10) with minor jitter
        let points = vec![
            StrokePoint::from(Pos2::new(10.0, 10.0)),
            StrokePoint::from(Pos2::new(30.0, 10.5)),
            StrokePoint::from(Pos2::new(50.0, 9.8)),
            StrokePoint::from(Pos2::new(70.0, 10.2)),
            StrokePoint::from(Pos2::new(90.0, 10.0)),
            StrokePoint::from(Pos2::new(100.0, 10.1)),
            StrokePoint::from(Pos2::new(105.0, 10.0)),
            StrokePoint::from(Pos2::new(110.0, 10.0)),
        ];

        let shape = recognize_shape(&points).expect("Should recognize straight line");
        assert_eq!(shape.kind, ShapeKind::Line);
    }

    #[test]
    fn test_recognize_shape_circle() {
        // Circle around (100, 100) with radius 40
        let center = Pos2::new(100.0, 100.0);
        let radius = 40.0;
        let mut points = Vec::new();
        for i in 0..24 {
            let angle = (i as f32) * std::f32::consts::TAU / 24.0;
            let pt = Pos2::new(
                center.x + radius * angle.cos(),
                center.y + radius * angle.sin(),
            );
            points.push(StrokePoint::from(pt));
        }
        // close the loop
        points.push(points[0].clone());

        let shape = recognize_shape(&points).expect("Should recognize circle");
        assert_eq!(shape.kind, ShapeKind::Ellipse);
    }

    #[test]
    fn test_recognize_shape_rectangle() {
        // Rectangle from (50, 50) to (150, 110)
        let mut points = Vec::new();
        // Top edge
        for x in [50.0, 75.0, 100.0, 125.0, 150.0] {
            points.push(StrokePoint::from(Pos2::new(x, 50.0)));
        }
        // Right edge
        for y in [65.0, 80.0, 95.0, 110.0] {
            points.push(StrokePoint::from(Pos2::new(150.0, y)));
        }
        // Bottom edge
        for x in [125.0, 100.0, 75.0, 50.0] {
            points.push(StrokePoint::from(Pos2::new(x, 110.0)));
        }
        // Left edge & close
        for y in [95.0, 80.0, 65.0, 50.0] {
            points.push(StrokePoint::from(Pos2::new(50.0, y)));
        }

        let shape = recognize_shape(&points).expect("Should recognize rectangle");
        assert_eq!(shape.kind, ShapeKind::Rectangle);
    }

    #[test]
    fn test_recognize_shape_triangle() {
        // Triangle: (100, 20), (40, 120), (160, 120)
        let mut points = Vec::new();
        // Edge 1: top to bottom-left
        for t in 0..6 {
            let frac = t as f32 / 5.0;
            points.push(StrokePoint::from(Pos2::new(
                100.0 - 60.0 * frac,
                20.0 + 100.0 * frac,
            )));
        }
        // Edge 2: bottom-left to bottom-right
        for t in 1..6 {
            let frac = t as f32 / 5.0;
            points.push(StrokePoint::from(Pos2::new(40.0 + 120.0 * frac, 120.0)));
        }
        // Edge 3: bottom-right to top
        for t in 1..=5 {
            let frac = t as f32 / 5.0;
            points.push(StrokePoint::from(Pos2::new(
                160.0 - 60.0 * frac,
                120.0 - 100.0 * frac,
            )));
        }

        let shape = recognize_shape(&points).expect("Should recognize triangle");
        assert_eq!(shape.kind, ShapeKind::Triangle);
    }

    #[test]
    fn test_recognize_shape_random_squiggle_none() {
        // Random squiggle points should NOT be recognized as clean shape
        let points = vec![
            StrokePoint::from(Pos2::new(10.0, 10.0)),
            StrokePoint::from(Pos2::new(20.0, 80.0)),
            StrokePoint::from(Pos2::new(60.0, 15.0)),
            StrokePoint::from(Pos2::new(30.0, 110.0)),
            StrokePoint::from(Pos2::new(90.0, 45.0)),
            StrokePoint::from(Pos2::new(15.0, 95.0)),
            StrokePoint::from(Pos2::new(80.0, 85.0)),
            StrokePoint::from(Pos2::new(40.0, 40.0)),
        ];

        let res = recognize_shape(&points);
        assert!(res.is_none());
    }

    #[test]
    fn test_split_stroke_by_eraser_middle_split() {
        // Garis horizontal dari x=0 ke x=100 pada y=50
        let points: Vec<StrokePoint> = (0..=10)
            .map(|i| StrokePoint {
                x: i as f32 * 10.0,
                y: 50.0,
                pressure: 0.5,
            })
            .collect();

        // Penghapus di (50, 50) dengan radius 12 -> memotong sekitar x=38..62
        let split = split_stroke_by_eraser(&points, Pos2::new(50.0, 50.0), 12.0)
            .expect("Should hit and split stroke");
        assert_eq!(split.len(), 2);

        // Segmen pertama berakhir tepat di tepi kiri lingkaran (x ~ 38)
        let first_end = split[0].last().unwrap();
        assert!((first_end.x - 38.0).abs() < 0.1);

        // Segmen kedua dimulai tepat di tepi kanan lingkaran (x ~ 62)
        let second_start = split[1].first().unwrap();
        assert!((second_start.x - 62.0).abs() < 0.1);
    }

    #[test]
    fn test_split_stroke_by_eraser_long_single_segment() {
        // Satu segmen panjang dari (0, 0) ke (100, 0)
        let points = vec![
            StrokePoint {
                x: 0.0,
                y: 0.0,
                pressure: 0.4,
            },
            StrokePoint {
                x: 100.0,
                y: 0.0,
                pressure: 0.8,
            },
        ];

        // Penghapus di tengah (50, 0) radius 10 -> memotong 1 segmen menjadi 2 segmen: [0..40] dan [60..100]
        let split = split_stroke_by_eraser(&points, Pos2::new(50.0, 0.0), 10.0)
            .expect("Should split single long segment");
        assert_eq!(split.len(), 2);
        assert!((split[0].last().unwrap().x - 40.0).abs() < 0.1);
        assert!((split[1].first().unwrap().x - 60.0).abs() < 0.1);
    }

    #[test]
    fn test_split_stroke_by_eraser_miss_and_full_erase() {
        let points = vec![
            StrokePoint {
                x: 10.0,
                y: 10.0,
                pressure: 0.5,
            },
            StrokePoint {
                x: 20.0,
                y: 10.0,
                pressure: 0.5,
            },
            StrokePoint {
                x: 30.0,
                y: 10.0,
                pressure: 0.5,
            },
        ];

        // Jauh di luar goresan -> None
        assert!(split_stroke_by_eraser(&points, Pos2::new(100.0, 100.0), 10.0).is_none());

        // Radius besar menutupi seluruh goresan -> Some(vec![])
        let full = split_stroke_by_eraser(&points, Pos2::new(20.0, 10.0), 50.0)
            .expect("Should hit stroke");
        assert!(full.is_empty());
    }

    #[test]
    fn test_palm_rejection_filter() {
        let existing = vec![StrokePoint {
            x: 100.0,
            y: 100.0,
            pressure: 0.6,
        }];

        // Gerakan normal pena dekat titik sebelumnya -> diterima (false)
        assert!(!should_reject_palm_input(
            &existing,
            Pos2::new(105.0, 104.0),
            0.016,
            false
        ));

        // Multi-touch aktif saat menulis -> ditolak sebagai sentuhan telapak tangan (true)
        assert!(should_reject_palm_input(
            &existing,
            Pos2::new(105.0, 104.0),
            0.016,
            true
        ));

        // Lompatan jarak ekstrem (> 180pt) dalam waktu sangat singkat (< 80ms) -> ditolak (true)
        assert!(should_reject_palm_input(
            &existing,
            Pos2::new(400.0, 450.0),
            0.010,
            false
        ));
    }

    #[test]
    fn test_handwriting_ocr_recognition() {
        // Buat goresan tulisan tangan untuk huruf "H" (2 garis vertikal + 1 garis horizontal)
        // diikuti huruf "I" (1 garis vertikal)
        let h_left = DrawObject::Stroke {
            id: "h1".into(),
            points: vec![
                StrokePoint::from(Pos2::new(20.0, 20.0)),
                StrokePoint::from(Pos2::new(20.0, 70.0)),
            ],
            color: dummy_color(),
            width: 3.0,
            is_highlighter: false,
        };
        let h_right = DrawObject::Stroke {
            id: "h2".into(),
            points: vec![
                StrokePoint::from(Pos2::new(45.0, 20.0)),
                StrokePoint::from(Pos2::new(45.0, 70.0)),
            ],
            color: dummy_color(),
            width: 3.0,
            is_highlighter: false,
        };
        let h_bar = DrawObject::Stroke {
            id: "h3".into(),
            points: vec![
                StrokePoint::from(Pos2::new(20.0, 45.0)),
                StrokePoint::from(Pos2::new(45.0, 45.0)),
            ],
            color: dummy_color(),
            width: 3.0,
            is_highlighter: false,
        };
        let i_vert = DrawObject::Stroke {
            id: "i1".into(),
            points: vec![
                StrokePoint::from(Pos2::new(65.0, 20.0)),
                StrokePoint::from(Pos2::new(65.0, 70.0)),
            ],
            color: dummy_color(),
            width: 3.0,
            is_highlighter: false,
        };

        let objects = vec![h_left, h_right, h_bar, i_vert];
        let ocr = recognize_handwriting_from_strokes(&objects);
        assert_eq!(ocr, "HI");
    }

    #[test]
    fn test_recognize_shape_right_triangle_and_scalene_triangle() {
        // 1. Segitiga Siku-Siku (RightTriangle) dengan sudut siku di kiri-bawah: (40, 20) -> (40, 120) -> (160, 120)
        let mut right_pts = Vec::new();
        for t in 0..=6 {
            let f = t as f32 / 6.0;
            right_pts.push(StrokePoint::from(Pos2::new(40.0, 20.0 + 100.0 * f)));
        }
        for t in 1..=6 {
            let f = t as f32 / 6.0;
            right_pts.push(StrokePoint::from(Pos2::new(40.0 + 120.0 * f, 120.0)));
        }
        for t in 1..=6 {
            let f = t as f32 / 6.0;
            right_pts.push(StrokePoint::from(Pos2::new(
                160.0 - 120.0 * f,
                120.0 - 100.0 * f,
            )));
        }
        let rec_right = recognize_shape(&right_pts).expect("Should recognize right triangle");
        assert_eq!(rec_right.kind, ShapeKind::RightTriangle);

        // 2. Segitiga Sembarang (ScaleneTriangle): (30, 25) -> (180, 60) -> (75, 140)
        let v0 = Pos2::new(30.0, 25.0);
        let v1 = Pos2::new(180.0, 60.0);
        let v2 = Pos2::new(75.0, 140.0);
        let mut scalene_pts = Vec::new();
        for (a, b) in [(v0, v1), (v1, v2), (v2, v0)] {
            for t in 0..6 {
                let f = t as f32 / 6.0;
                scalene_pts.push(StrokePoint::from(Pos2::new(
                    a.x + (b.x - a.x) * f,
                    a.y + (b.y - a.y) * f,
                )));
            }
        }
        scalene_pts.push(StrokePoint::from(v0));
        let rec_scalene = recognize_shape(&scalene_pts).expect("Should recognize scalene triangle");
        assert!(matches!(
            rec_scalene.kind,
            ShapeKind::ScaleneTriangle { .. }
        ));
    }

    #[test]
    fn test_recognize_shape_parallelogram() {
        // Jajar Genjang (Parallelogram): (70, 40) -> (190, 40) -> (150, 110) -> (30, 110)
        let c0 = Pos2::new(70.0, 40.0);
        let c1 = Pos2::new(190.0, 40.0);
        let c2 = Pos2::new(150.0, 110.0);
        let c3 = Pos2::new(30.0, 110.0);
        let mut para_pts = Vec::new();
        for (a, b) in [(c0, c1), (c1, c2), (c2, c3), (c3, c0)] {
            for t in 0..6 {
                let f = t as f32 / 6.0;
                para_pts.push(StrokePoint::from(Pos2::new(
                    a.x + (b.x - a.x) * f,
                    a.y + (b.y - a.y) * f,
                )));
            }
        }
        para_pts.push(StrokePoint::from(c0));

        let rec_para = recognize_shape(&para_pts).expect("Should recognize parallelogram");
        assert_eq!(rec_para.kind, ShapeKind::Parallelogram);
    }

    #[test]
    fn test_rotate_line_around_start_point() {
        // Garis mulai di (50, 60) dengan vektor (100, 0) -> titik akhir di (150, 60)
        let mut line = DrawObject::Shape {
            id: "line-1".into(),
            shape_type: ShapeKind::Line,
            x: 50.0,
            y: 60.0,
            width: 100.0,
            height: 0.0,
            color: dummy_color(),
            stroke_width: 2.0,
            fill: None,
        };

        // Rotasi 90 derajat (PI / 2): titik awal (50, 60) tetap sebagai pusat,
        // titik akhir berotasi menjadi (50, 160) -> width = 0, height = 100
        rotate_object(
            &mut line,
            Pos2::new(100.0, 60.0),
            std::f32::consts::FRAC_PI_2,
        );
        if let DrawObject::Shape {
            x,
            y,
            width,
            height,
            ..
        } = line
        {
            assert!((x - 50.0).abs() < 1e-4);
            assert!((y - 60.0).abs() < 1e-4);
            assert!(width.abs() < 1e-4);
            assert!((height - 100.0).abs() < 1e-4);
        } else {
            panic!("Expected Line Shape");
        }
    }

    #[test]
    fn test_rotate_page_content_90_cw_and_180() {
        let mut page =
            crate::model::Page::with_size(crate::model::PageTemplate::Blank, 800.0, 1100.0);
        page.objects.push(DrawObject::Stroke {
            id: "s1".into(),
            points: vec![
                StrokePoint {
                    x: 100.0,
                    y: 200.0,
                    pressure: 1.0,
                },
                StrokePoint {
                    x: 300.0,
                    y: 400.0,
                    pressure: 1.0,
                },
            ],
            color: dummy_color(),
            width: 2.0,
            is_highlighter: false,
        });

        // Rotate 90 CW: (x, y) -> (old_h - y, x)
        // old_h = 1100, old_w = 800
        // (100, 200) -> (1100 - 200, 100) = (900, 100)
        // (300, 400) -> (1100 - 400, 300) = (700, 300)
        rotate_page_content(&mut page, 90);
        assert_eq!(page.width, 1100.0);
        assert_eq!(page.height, 800.0);
        assert_eq!(page.rotation, 90);
        if let DrawObject::Stroke { points, .. } = &page.objects[0] {
            assert!((points[0].x - 900.0).abs() < 1e-4);
            assert!((points[0].y - 100.0).abs() < 1e-4);
            assert!((points[1].x - 700.0).abs() < 1e-4);
            assert!((points[1].y - 300.0).abs() < 1e-4);
        } else {
            panic!("Expected stroke");
        }

        // Rotate 180: (x, y) -> (w - x, h - y)
        // current w = 1100, h = 800
        // (900, 100) -> (1100 - 900, 800 - 100) = (200, 700)
        rotate_page_content(&mut page, 180);
        assert_eq!(page.width, 1100.0);
        assert_eq!(page.height, 800.0);
        assert_eq!(page.rotation, 270);
        if let DrawObject::Stroke { points, .. } = &page.objects[0] {
            assert!((points[0].x - 200.0).abs() < 1e-4);
            assert!((points[0].y - 700.0).abs() < 1e-4);
        } else {
            panic!("Expected stroke");
        }
    }

    #[test]
    fn test_highlighter_tessellation_no_overlap() {
        let pts = vec![
            StrokePoint {
                x: 10.0,
                y: 10.0,
                pressure: 0.1, // Pressure bervariasi tidak boleh memengaruhi ketebalan highlighter
            },
            StrokePoint {
                x: 100.0,
                y: 10.0,
                pressure: 0.9,
            },
        ];
        let mesh = tessellate_stroke_ex(
            &pts,
            20.0,
            eframe::egui::Color32::from_rgba_unmultiplied(255, 255, 0, 90),
            true,
            |p| p,
        );
        assert!(!mesh.vertices.is_empty());
        assert!(!mesh.indices.is_empty());
        // Semua index segitiga valid
        for &idx in &mesh.indices {
            assert!((idx as usize) < mesh.vertices.len());
        }
    }
}

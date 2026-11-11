use super::primitives::{
    equation_approx_size, object_boundary_vertices, object_bounds, point_in_polygon,
    point_to_segment_dist, segments_cross, shape_vertices, text_approx_size,
};
use crate::model::DrawObject;
use eframe::egui::{Pos2, Rect, Vec2};

/// Menghitung jarak Euclidean terdekat dari titik `p` ke geometri objek `obj`.
/// Menggunakan transformasi invers koordinat lokal agar rotasi objek dihitung secara presisi.
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
            rotation,
            shape_type,
            ..
        } => {
            let center = Pos2::new(*x + *width * 0.5, *y + *height * 0.5);
            let p_local = if rotation.abs() > 1e-5 {
                let dx = p.x - center.x;
                let dy = p.y - center.y;
                let cos_a = rotation.cos();
                let sin_a = rotation.sin();
                Pos2::new(
                    center.x + dx * cos_a + dy * sin_a,
                    center.y - dx * sin_a + dy * cos_a,
                )
            } else {
                p
            };

            let half_w = stroke_width * 0.5;
            let local_rect =
                Rect::from_two_pos(Pos2::new(*x, *y), Pos2::new(*x + *width, *y + *height));

            if fill.is_some() {
                match shape_type {
                    crate::model::ShapeKind::Rectangle => {
                        if local_rect.contains(p_local) {
                            return Some(0.0);
                        }
                    }
                    crate::model::ShapeKind::Ellipse => {
                        let rx = width.abs() * 0.5;
                        let ry = height.abs() * 0.5;
                        if rx > 1e-4 && ry > 1e-4 {
                            let dx = (p_local.x - center.x) / rx;
                            let dy = (p_local.y - center.y) / ry;
                            if dx * dx + dy * dy <= 1.0 {
                                return Some(0.0);
                            }
                        }
                    }
                    crate::model::ShapeKind::Line => {}
                    _ => {
                        let verts = shape_vertices(*shape_type, *x, *y, *width, *height);
                        if point_in_polygon(p_local, &verts) {
                            return Some(0.0);
                        }
                    }
                }
            }

            match shape_type {
                crate::model::ShapeKind::Line => {
                    let a = Pos2::new(*x, *y);
                    let b_pt = Pos2::new(x + width, y + height);
                    Some((point_to_segment_dist(p_local, a, b_pt) - half_w).max(0.0))
                }
                crate::model::ShapeKind::Rectangle => {
                    let tl = local_rect.left_top();
                    let tr = local_rect.right_top();
                    let br = local_rect.right_bottom();
                    let bl = local_rect.left_bottom();
                    let d = point_to_segment_dist(p_local, tl, tr)
                        .min(point_to_segment_dist(p_local, tr, br))
                        .min(point_to_segment_dist(p_local, br, bl))
                        .min(point_to_segment_dist(p_local, bl, tl));
                    Some((d - half_w).max(0.0))
                }
                crate::model::ShapeKind::Ellipse => {
                    let rx = width.abs() * 0.5;
                    let ry = height.abs() * 0.5;
                    if rx > 1e-4 && ry > 1e-4 {
                        let dx = (p_local.x - center.x) / rx;
                        let dy = (p_local.y - center.y) / ry;
                        let d = (dx * dx + dy * dy).sqrt();
                        let min_r = rx.min(ry);
                        let edge_dist = (d - 1.0).abs() * min_r;
                        Some((edge_dist - half_w).max(0.0))
                    } else {
                        Some(0.0)
                    }
                }
                _ => {
                    let verts = shape_vertices(*shape_type, *x, *y, *width, *height);
                    let mut min_d = f32::INFINITY;
                    for i in 0..verts.len() {
                        let a = verts[i];
                        let b_pt = verts[(i + 1) % verts.len()];
                        min_d = min_d.min(point_to_segment_dist(p_local, a, b_pt));
                    }
                    Some((min_d - half_w).max(0.0))
                }
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
            let p_local = if rotation.abs() > 1e-5 {
                let dx = p.x - p0.x;
                let dy = p.y - p0.y;
                let cos_a = rotation.cos();
                let sin_a = rotation.sin();
                Pos2::new(
                    p0.x + dx * cos_a + dy * sin_a,
                    p0.y - dx * sin_a + dy * cos_a,
                )
            } else {
                p
            };
            let local_rect = Rect::from_min_size(p0, Vec2::new(w, h));
            if local_rect.contains(p_local) {
                Some(0.0)
            } else {
                let dx = (local_rect.min.x - p_local.x)
                    .max(0.0)
                    .max(p_local.x - local_rect.max.x);
                let dy = (local_rect.min.y - p_local.y)
                    .max(0.0)
                    .max(p_local.y - local_rect.max.y);
                Some((dx * dx + dy * dy).sqrt())
            }
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
            let p_local = if rotation.abs() > 1e-5 {
                let dx = p.x - center.x;
                let dy = p.y - center.y;
                let cos_a = rotation.cos();
                let sin_a = rotation.sin();
                Pos2::new(
                    center.x + dx * cos_a + dy * sin_a,
                    center.y - dx * sin_a + dy * cos_a,
                )
            } else {
                p
            };
            let local_rect = Rect::from_center_size(center, Vec2::new(*width, *height));
            if local_rect.contains(p_local) {
                Some(0.0)
            } else {
                let dx = (local_rect.min.x - p_local.x)
                    .max(0.0)
                    .max(p_local.x - local_rect.max.x);
                let dy = (local_rect.min.y - p_local.y)
                    .max(0.0)
                    .max(p_local.y - local_rect.max.y);
                Some((dx * dx + dy * dy).sqrt())
            }
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
            let p_local = if rotation.abs() > 1e-5 {
                let dx = p.x - center.x;
                let dy = p.y - center.y;
                let cos_a = rotation.cos();
                let sin_a = rotation.sin();
                Pos2::new(
                    center.x + dx * cos_a + dy * sin_a,
                    center.y - dx * sin_a + dy * cos_a,
                )
            } else {
                p
            };
            let local_rect = Rect::from_center_size(center, Vec2::new(w, h));
            if local_rect.contains(p_local) {
                Some(0.0)
            } else {
                let dx = (local_rect.min.x - p_local.x)
                    .max(0.0)
                    .max(p_local.x - local_rect.max.x);
                let dy = (local_rect.min.y - p_local.y)
                    .max(0.0)
                    .max(p_local.y - local_rect.max.y);
                Some((dx * dx + dy * dy).sqrt())
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
            for cp in control_points {
                pts.push(Pos2::new(cp.x, cp.y));
            }
            if let crate::model::ConnectorEnd::Free { x, y } = end {
                pts.push(Pos2::new(*x, *y));
            }
            if pts.is_empty() {
                return if b.contains(p) { Some(0.0) } else { None };
            }
            let half_w = style.stroke_width * 0.5;
            if pts.len() == 1 {
                return Some(((p - pts[0]).length() - half_w).max(0.0));
            }
            let mut min_d = f32::INFINITY;
            for i in 1..pts.len() {
                min_d = min_d.min(point_to_segment_dist(p, pts[i - 1], pts[i]));
            }
            Some((min_d - half_w).max(0.0))
        }
    }
}

/// Menguji apakah titik `p` mengenai objek `obj` dengan toleransi `tolerance`.
/// Menggunakan transformasi invers koordinat lokal agar seluruh objek berotasi
/// (Text, StickyNote, Shape, Image, Equation) dapat dipilih secara presisi.
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
            rotation,
            shape_type,
            ..
        } => {
            let center = Pos2::new(*x + *width * 0.5, *y + *height * 0.5);
            let p_local = if rotation.abs() > 1e-5 {
                let dx = p.x - center.x;
                let dy = p.y - center.y;
                let cos_a = rotation.cos();
                let sin_a = rotation.sin();
                Pos2::new(
                    center.x + dx * cos_a + dy * sin_a,
                    center.y - dx * sin_a + dy * cos_a,
                )
            } else {
                p
            };

            let tol = tolerance + stroke_width / 2.0;
            let local_rect =
                Rect::from_two_pos(Pos2::new(*x, *y), Pos2::new(*x + *width, *y + *height));

            if fill.is_some() {
                match shape_type {
                    crate::model::ShapeKind::Rectangle => {
                        return local_rect.expand(tolerance).contains(p_local);
                    }
                    crate::model::ShapeKind::Ellipse => {
                        let rx = width.abs() * 0.5;
                        let ry = height.abs() * 0.5;
                        if rx < 1e-4 || ry < 1e-4 {
                            return local_rect.expand(tolerance).contains(p_local);
                        }
                        let dx = (p_local.x - center.x) / rx;
                        let dy = (p_local.y - center.y) / ry;
                        let norm_d = dx * dx + dy * dy;
                        let max_tol_ratio = 1.0 + tolerance / rx.min(ry);
                        return norm_d <= max_tol_ratio * max_tol_ratio;
                    }
                    crate::model::ShapeKind::Line => {}
                    _ => {
                        let verts = shape_vertices(*shape_type, *x, *y, *width, *height);
                        if point_in_polygon(p_local, &verts) {
                            return true;
                        }
                    }
                }
            }

            match shape_type {
                crate::model::ShapeKind::Line => {
                    let a = Pos2::new(*x, *y);
                    let b = Pos2::new(x + width, y + height);
                    point_to_segment_dist(p_local, a, b) <= tol
                }
                crate::model::ShapeKind::Rectangle => {
                    local_rect.expand(tol).contains(p_local)
                        && (fill.is_some() || !local_rect.shrink(tol).contains(p_local))
                }
                crate::model::ShapeKind::Ellipse => {
                    let rx = width.abs() * 0.5;
                    let ry = height.abs() * 0.5;
                    if rx < 1e-4 || ry < 1e-4 {
                        return local_rect.expand(tol).contains(p_local);
                    }
                    let dx = (p_local.x - center.x) / rx;
                    let dy = (p_local.y - center.y) / ry;
                    let d = (dx * dx + dy * dy).sqrt();
                    let min_r = rx.min(ry);
                    let r_diff = (d - 1.0).abs() * min_r;
                    if fill.is_some() {
                        d <= 1.0 || r_diff <= tol
                    } else {
                        r_diff <= tol
                    }
                }
                _ => {
                    let verts = shape_vertices(*shape_type, *x, *y, *width, *height);
                    for i in 0..verts.len() {
                        let a = verts[i];
                        let b_pt = verts[(i + 1) % verts.len()];
                        if point_to_segment_dist(p_local, a, b_pt) <= tol {
                            return true;
                        }
                    }
                    false
                }
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
            let p_local = if rotation.abs() > 1e-5 {
                let dx = p.x - p0.x;
                let dy = p.y - p0.y;
                let cos_a = rotation.cos();
                let sin_a = rotation.sin();
                Pos2::new(
                    p0.x + dx * cos_a + dy * sin_a,
                    p0.y - dx * sin_a + dy * cos_a,
                )
            } else {
                p
            };
            let local_rect = Rect::from_min_size(p0, Vec2::new(w, h));
            local_rect.expand(tolerance).contains(p_local)
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
            let p_local = if rotation.abs() > 1e-5 {
                let dx = p.x - center.x;
                let dy = p.y - center.y;
                let cos_a = rotation.cos();
                let sin_a = rotation.sin();
                Pos2::new(
                    center.x + dx * cos_a + dy * sin_a,
                    center.y - dx * sin_a + dy * cos_a,
                )
            } else {
                p
            };
            let local_rect = Rect::from_center_size(center, Vec2::new(*width, *height));
            local_rect.expand(tolerance).contains(p_local)
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
            let p_local = if rotation.abs() > 1e-5 {
                let dx = p.x - center.x;
                let dy = p.y - center.y;
                let cos_a = rotation.cos();
                let sin_a = rotation.sin();
                Pos2::new(
                    center.x + dx * cos_a + dy * sin_a,
                    center.y - dx * sin_a + dy * cos_a,
                )
            } else {
                p
            };
            let local_rect = Rect::from_center_size(center, Vec2::new(w, h));
            local_rect.expand(tolerance).contains(p_local)
        }
        DrawObject::Connector {
            start,
            end,
            control_points,
            style,
            ..
        } => {
            let tol = tolerance + style.stroke_width * 0.5;
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
            if pts.len() == 1 {
                return (p - pts[0]).length() <= tol;
            }
            for i in 1..pts.len() {
                if point_to_segment_dist(p, pts[i - 1], pts[i]) <= tol {
                    return true;
                }
            }
            false
        }
    }
}

/// Memeriksa apakah suatu objek memotong atau berada di dalam poligon lasso (Intersection Mode).
/// Memeriksa:
/// 1. Titik pusat objek berada di dalam lasso
/// 2. Lasso berada di dalam objek
/// 3. Simpul batas objek berada di dalam poligon lasso
/// 4. Segmen batas objek memotong segmen batas poligon lasso
pub fn object_in_lasso(obj: &DrawObject, polygon: &[Pos2]) -> bool {
    if polygon.len() < 3 {
        return false;
    }
    let b = object_bounds(obj);
    if !b.is_finite() {
        return false;
    }

    // 1. Jika pusat objek berada di dalam lasso
    if point_in_polygon(b.center(), polygon) {
        return true;
    }

    // 2. Jika seluruh poligon lasso berada di dalam objek
    if hit_test_object(obj, polygon[0], 0.0) {
        return true;
    }

    let verts = object_boundary_vertices(obj);
    if verts.is_empty() {
        return false;
    }

    // 3. Jika ada simpul batas objek yang berada di dalam poligon lasso
    if verts.iter().any(|&v| point_in_polygon(v, polygon)) {
        return true;
    }

    // 4. Jika ada segmen batas objek yang memotong segmen poligon lasso
    let is_closed = match obj {
        DrawObject::Stroke { .. } | DrawObject::Connector { .. } => false,
        DrawObject::Shape { shape_type, .. } => *shape_type != crate::model::ShapeKind::Line,
        _ => true,
    };

    let seg_count = if is_closed {
        verts.len()
    } else {
        verts.len().saturating_sub(1)
    };
    for i in 0..seg_count {
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

/// Memeriksa apakah suatu objek secara penuh dilingkupi di dalam batas poligon lasso (Containment Mode).
/// Menjamin seluruh geometri terlihat objek berada di dalam poligon lasso tanpa ada batas yang terpotong.
pub fn object_contained_in_lasso(obj: &DrawObject, polygon: &[Pos2]) -> bool {
    if polygon.len() < 3 {
        return false;
    }

    // 1. Broad-phase AABB culling
    let mut poly_min_x = f32::INFINITY;
    let mut poly_max_x = f32::NEG_INFINITY;
    let mut poly_min_y = f32::INFINITY;
    let mut poly_max_y = f32::NEG_INFINITY;
    for p in polygon {
        poly_min_x = poly_min_x.min(p.x);
        poly_max_x = poly_max_x.max(p.x);
        poly_min_y = poly_min_y.min(p.y);
        poly_max_y = poly_max_y.max(p.y);
    }

    let b = object_bounds(obj);
    if !b.is_finite() {
        return false;
    }

    // Jika batas luar objek keluar dari batas poligon, objek pasti tidak sepenuhnya dilingkupi
    if b.min.x < poly_min_x - 0.5
        || b.max.x > poly_max_x + 0.5
        || b.min.y < poly_min_y - 0.5
        || b.max.y > poly_max_y + 0.5
    {
        return false;
    }

    // Kasus khusus untuk goresan 1 titik (dot)
    if let DrawObject::Stroke { points, width, .. } = obj {
        if points.is_empty() {
            return false;
        }
        if points.len() == 1 {
            let p = points[0].to_pos2();
            if !point_in_polygon(p, polygon) {
                return false;
            }
            let r = (*width * 0.5).max(1.0);
            let cardinals = [
                p + Vec2::new(r, 0.0),
                p - Vec2::new(r, 0.0),
                p + Vec2::new(0.0, r),
                p - Vec2::new(0.0, r),
            ];
            return cardinals.iter().all(|&pt| point_in_polygon(pt, polygon));
        }
    }

    let verts = object_boundary_vertices(obj);
    if verts.is_empty() {
        return false;
    }

    // 2. Seluruh simpul batas objek harus berada di dalam poligon
    if !verts.iter().all(|&v| point_in_polygon(v, polygon)) {
        return false;
    }

    // 3. Tidak boleh ada segmen batas objek yang memotong batas poligon
    let is_closed = match obj {
        DrawObject::Stroke { .. } | DrawObject::Connector { .. } => false,
        DrawObject::Shape { shape_type, .. } => *shape_type != crate::model::ShapeKind::Line,
        _ => true,
    };

    let seg_count = if is_closed {
        verts.len()
    } else {
        verts.len().saturating_sub(1)
    };
    for i in 0..seg_count {
        let p1 = verts[i];
        let p2 = verts[(i + 1) % verts.len()];
        for j in 0..polygon.len() {
            let l1 = polygon[j];
            let l2 = polygon[(j + 1) % polygon.len()];
            if segments_cross(p1, p2, l1, l2) {
                return false;
            }
        }
    }

    true
}

/// Memilih objek menggunakan mode lasso yang dikonfigurasi.
pub fn object_in_lasso_mode(
    obj: &DrawObject,
    polygon: &[Pos2],
    mode: crate::model::LassoSelectionMode,
) -> bool {
    match mode {
        crate::model::LassoSelectionMode::Containment => object_contained_in_lasso(obj, polygon),
        crate::model::LassoSelectionMode::Intersection => object_in_lasso(obj, polygon),
    }
}

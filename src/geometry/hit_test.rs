use super::primitives::{
    object_bounds, point_in_polygon, point_to_segment_dist, segments_cross, shape_vertices,
};
use crate::model::DrawObject;
use eframe::egui::{Pos2, Vec2};

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
        DrawObject::Text { .. }
        | DrawObject::StickyNote { .. }
        | DrawObject::Image { .. }
        | DrawObject::Equation { .. } => {
            if b.contains(p) {
                Some(0.0)
            } else {
                None
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
        DrawObject::Text { .. }
        | DrawObject::StickyNote { .. }
        | DrawObject::Image { .. }
        | DrawObject::Equation { .. } => b.contains(p),
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
        DrawObject::Text { .. }
        | DrawObject::StickyNote { .. }
        | DrawObject::Image { .. }
        | DrawObject::Equation { .. } => {
            let corners = [
                b.left_top(),
                b.right_top(),
                b.right_bottom(),
                b.left_bottom(),
            ];
            corners.iter().any(|&c| point_in_polygon(c, polygon))
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
            if pts.iter().any(|&pt| point_in_polygon(pt, polygon)) {
                return true;
            }
            for i in 1..pts.len() {
                let p1 = pts[i - 1];
                let p2 = pts[i];
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
    }
}

/// Memeriksa apakah suatu objek secara penuh dilingkupi di dalam batas poligon lasso (Task 6 - Containment Mode).
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

    // Helper: memeriksa apakah segmen garis objek memotong batas poligon
    let crosses_polygon = |p1: Pos2, p2: Pos2| -> bool {
        for j in 0..polygon.len() {
            let l1 = polygon[j];
            let l2 = polygon[(j + 1) % polygon.len()];
            if segments_cross(p1, p2, l1, l2) {
                return true;
            }
        }
        false
    };

    match obj {
        DrawObject::Stroke { points, width, .. } => {
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

            // Semua titik stroke harus berada di dalam poligon
            if !points
                .iter()
                .all(|pt| point_in_polygon(pt.to_pos2(), polygon))
            {
                return false;
            }
            // Tidak boleh ada segmen stroke yang memotong batas poligon
            for i in 1..points.len() {
                if crosses_polygon(points[i - 1].to_pos2(), points[i].to_pos2()) {
                    return false;
                }
            }
            true
        }
        DrawObject::Shape {
            shape_type,
            x,
            y,
            width,
            height,
            rotation,
            ..
        } => {
            if *shape_type == crate::model::ShapeKind::Line {
                let p1 = Pos2::new(*x, *y);
                let p2 = Pos2::new(*x + *width, *y + *height);
                return point_in_polygon(p1, polygon)
                    && point_in_polygon(p2, polygon)
                    && !crosses_polygon(p1, p2);
            }

            let mut verts = shape_vertices(*shape_type, *x, *y, *width, *height);
            if rotation.abs() > 1e-4 {
                let center = Pos2::new(*x + *width / 2.0, *y + *height / 2.0);
                let cos_a = rotation.cos();
                let sin_a = rotation.sin();
                for v in &mut verts {
                    let dx = v.x - center.x;
                    let dy = v.y - center.y;
                    *v = Pos2::new(
                        center.x + dx * cos_a - dy * sin_a,
                        center.y + dx * sin_a + dy * cos_a,
                    );
                }
            }

            if !verts.iter().all(|&v| point_in_polygon(v, polygon)) {
                return false;
            }
            for i in 0..verts.len() {
                let p1 = verts[i];
                let p2 = verts[(i + 1) % verts.len()];
                if crosses_polygon(p1, p2) {
                    return false;
                }
            }
            true
        }
        DrawObject::Text {
            x,
            y,
            font_size,
            text,
            rotation,
            ..
        } => {
            let lines: Vec<&str> = text.lines().collect();
            let line_count = lines.len().max(1) as f32;
            let max_line_len = lines.iter().map(|l| l.chars().count()).max().unwrap_or(1) as f32;
            let approx_w = (max_line_len * (font_size * 0.58).max(8.0)).max(20.0);
            let approx_h = line_count * (font_size * 1.35).max(18.0);
            let cos_a = rotation.cos();
            let sin_a = rotation.sin();
            let rot_v =
                |lx: f32, ly: f32| Vec2::new(lx * cos_a - ly * sin_a, lx * sin_a + ly * cos_a);
            let p0 = Pos2::new(*x, *y);
            let corners = [
                p0,
                p0 + rot_v(approx_w, 0.0),
                p0 + rot_v(approx_w, approx_h),
                p0 + rot_v(0.0, approx_h),
            ];
            if !corners.iter().all(|&c| point_in_polygon(c, polygon)) {
                return false;
            }
            for i in 0..4 {
                if crosses_polygon(corners[i], corners[(i + 1) % 4]) {
                    return false;
                }
            }
            true
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
            let eff_font = *font_size * *scale;
            let layout = crate::geometry::math::MathLayout::compute_from_str(latex, eff_font);
            let w = layout.width.max(20.0);
            let h = layout.height.max(eff_font);
            let center = Pos2::new(*x + w * 0.5, *y + h * 0.5);
            let hw = w * 0.5;
            let hh = h * 0.5;
            let cos_a = rotation.cos();
            let sin_a = rotation.sin();
            let rot_v =
                |lx: f32, ly: f32| Vec2::new(lx * cos_a - ly * sin_a, lx * sin_a + ly * cos_a);
            let corners = [
                center + rot_v(-hw, -hh),
                center + rot_v(hw, -hh),
                center + rot_v(hw, hh),
                center + rot_v(-hw, hh),
            ];
            if !corners.iter().all(|&c| point_in_polygon(c, polygon)) {
                return false;
            }
            for i in 0..4 {
                if crosses_polygon(corners[i], corners[(i + 1) % 4]) {
                    return false;
                }
            }
            true
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
            let center = Pos2::new(*x + *width / 2.0, *y + *height / 2.0);
            let hw = *width * 0.5;
            let hh = *height * 0.5;
            let cos_a = rotation.cos();
            let sin_a = rotation.sin();
            let rot_v =
                |lx: f32, ly: f32| Vec2::new(lx * cos_a - ly * sin_a, lx * sin_a + ly * cos_a);
            let corners = [
                center + rot_v(-hw, -hh),
                center + rot_v(hw, -hh),
                center + rot_v(hw, hh),
                center + rot_v(-hw, hh),
            ];
            if !corners.iter().all(|&c| point_in_polygon(c, polygon)) {
                return false;
            }
            for i in 0..4 {
                if crosses_polygon(corners[i], corners[(i + 1) % 4]) {
                    return false;
                }
            }
            true
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
            if pts.is_empty() {
                return false;
            }
            if !pts.iter().all(|&pt| point_in_polygon(pt, polygon)) {
                return false;
            }
            for i in 1..pts.len() {
                if crosses_polygon(pts[i - 1], pts[i]) {
                    return false;
                }
            }
            true
        }
    }
}

/// Memilih objek menggunakan mode lasso yang dikonfigurasi (Task 6).
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

use super::primitives::{
    object_bounds, point_in_polygon, point_to_segment_dist, segments_cross, shape_vertices,
};
use crate::model::DrawObject;
use eframe::egui::Pos2;

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

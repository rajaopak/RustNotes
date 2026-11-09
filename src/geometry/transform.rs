use super::primitives::shape_vertices;
use crate::model::{DrawObject, Page, ShapeKind};
use eframe::egui::Pos2;

pub fn translate_object(obj: &mut DrawObject, dx: f32, dy: f32) {
    match obj {
        DrawObject::Stroke { points, .. } => {
            for p in points {
                p.x += dx;
                p.y += dy;
            }
        }
        DrawObject::Shape { x, y, locked, .. } => {
            if *locked {
                return;
            }
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
        DrawObject::Connector {
            start,
            end,
            control_points,
            ..
        } => {
            if let crate::model::ConnectorEnd::Free { x, y } = start {
                *x += dx;
                *y += dy;
            }
            for cp in control_points {
                cp.x += dx;
                cp.y += dy;
            }
            if let crate::model::ConnectorEnd::Free { x, y } = end {
                *x += dx;
                *y += dy;
            }
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
            corner_radius,
            locked,
            label,
            ..
        } => {
            if *locked {
                return;
            }
            *x = origin.x + (*x - origin.x) * sx;
            *y = origin.y + (*y - origin.y) * sy;
            *width *= sx;
            *height *= sy;
            let avg_s = (sx.abs() + sy.abs()) / 2.0;
            *stroke_width *= avg_s;
            *corner_radius *= avg_s;
            if let Some(lbl) = label {
                lbl.font_size *= avg_s;
            }
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
        DrawObject::Connector {
            start,
            end,
            control_points,
            style,
            ..
        } => {
            if let crate::model::ConnectorEnd::Free { x, y } = start {
                *x = origin.x + (*x - origin.x) * sx;
                *y = origin.y + (*y - origin.y) * sy;
            }
            for cp in control_points {
                cp.x = origin.x + (cp.x - origin.x) * sx;
                cp.y = origin.y + (cp.y - origin.y) * sy;
            }
            if let crate::model::ConnectorEnd::Free { x, y } = end {
                *x = origin.x + (*x - origin.x) * sx;
                *y = origin.y + (*y - origin.y) * sy;
            }
            style.stroke_width *= (sx.abs() + sy.abs()) / 2.0;
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
            locked,
            ..
        } => {
            if *locked {
                return;
            }
            match shape_type {
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
            }
        }
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
        DrawObject::Connector {
            start,
            end,
            control_points,
            ..
        } => {
            if let crate::model::ConnectorEnd::Free { x, .. } = start {
                *x = 2.0 * center_x - *x;
            }
            for cp in control_points {
                cp.x = 2.0 * center_x - cp.x;
            }
            if let crate::model::ConnectorEnd::Free { x, .. } = end {
                *x = 2.0 * center_x - *x;
            }
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
            locked,
            ..
        } => {
            if *locked {
                return;
            }
            match shape_type {
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
            }
        }
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
        DrawObject::Connector {
            start,
            end,
            control_points,
            ..
        } => {
            if let crate::model::ConnectorEnd::Free { y, .. } = start {
                *y = 2.0 * center_y - *y;
            }
            for cp in control_points {
                cp.y = 2.0 * center_y - cp.y;
            }
            if let crate::model::ConnectorEnd::Free { y, .. } = end {
                *y = 2.0 * center_y - *y;
            }
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
            rotation,
            shape_type,
            locked,
            ..
        } => {
            if *locked {
                return;
            }
            match shape_type {
                crate::model::ShapeKind::Line => {
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
                    *rotation = (*rotation + angle_rad).rem_euclid(std::f32::consts::TAU);
                }
            }
        }
        DrawObject::Text { x, y, rotation, .. } => {
            let (nx, ny) = rotate_pt(*x, *y);
            *x = nx;
            *y = ny;
            *rotation = (*rotation + angle_rad).rem_euclid(std::f32::consts::TAU);
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
        DrawObject::Connector {
            start,
            end,
            control_points,
            ..
        } => {
            if let crate::model::ConnectorEnd::Free { x, y } = start {
                let (nx, ny) = rotate_pt(*x, *y);
                *x = nx;
                *y = ny;
            }
            for cp in control_points {
                let (nx, ny) = rotate_pt(cp.x, cp.y);
                cp.x = nx;
                cp.y = ny;
            }
            if let crate::model::ConnectorEnd::Free { x, y } = end {
                let (nx, ny) = rotate_pt(*x, *y);
                *x = nx;
                *y = ny;
            }
        }
    }
}

/// Rotasi penuh isi halaman dan ukurannya (90° CW, 90° CCW / 270°, atau 180°).
pub fn rotate_page_content(page: &mut Page, degrees: i32) {
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
            rotation,
            shape_type,
            ..
        } => {
            *rotation = (*rotation + std::f32::consts::FRAC_PI_2).rem_euclid(std::f32::consts::TAU);
            match shape_type {
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
            }
        }
        DrawObject::Text { x, y, rotation, .. } => {
            let nx = old_h - *y;
            let ny = *x;
            *x = nx;
            *y = ny;
            *rotation = (*rotation + std::f32::consts::FRAC_PI_2).rem_euclid(std::f32::consts::TAU);
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
            *x = ncx - *width / 2.0;
            *y = ncy - *height / 2.0;
            *rotation = (*rotation + std::f32::consts::FRAC_PI_2).rem_euclid(std::f32::consts::TAU);
        }
        DrawObject::Connector {
            start,
            end,
            control_points,
            ..
        } => {
            if let crate::model::ConnectorEnd::Free { x, y } = start {
                let nx = old_h - *y;
                let ny = *x;
                *x = nx;
                *y = ny;
            }
            for cp in control_points {
                let nx = old_h - cp.y;
                let ny = cp.x;
                cp.x = nx;
                cp.y = ny;
            }
            if let crate::model::ConnectorEnd::Free { x, y } = end {
                let nx = old_h - *y;
                let ny = *x;
                *x = nx;
                *y = ny;
            }
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
            rotation,
            shape_type,
            ..
        } => {
            *rotation = (*rotation - std::f32::consts::FRAC_PI_2).rem_euclid(std::f32::consts::TAU);
            match shape_type {
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
            }
        }
        DrawObject::Text { x, y, rotation, .. } => {
            let nx = *y;
            let ny = old_w - *x;
            *x = nx;
            *y = ny;
            *rotation = (*rotation - std::f32::consts::FRAC_PI_2).rem_euclid(std::f32::consts::TAU);
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
            *x = ncx - *width / 2.0;
            *y = ncy - *height / 2.0;
            *rotation = (*rotation - std::f32::consts::FRAC_PI_2).rem_euclid(std::f32::consts::TAU);
        }
        DrawObject::Connector {
            start,
            end,
            control_points,
            ..
        } => {
            if let crate::model::ConnectorEnd::Free { x, y } = start {
                let nx = *y;
                let ny = old_w - *x;
                *x = nx;
                *y = ny;
            }
            for cp in control_points {
                let nx = cp.y;
                let ny = old_w - cp.x;
                cp.x = nx;
                cp.y = ny;
            }
            if let crate::model::ConnectorEnd::Free { x, y } = end {
                let nx = *y;
                let ny = old_w - *x;
                *x = nx;
                *y = ny;
            }
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
            rotation,
            shape_type,
            ..
        } => {
            *rotation = (*rotation + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU);
            match shape_type {
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
            }
        }
        DrawObject::Text { x, y, rotation, .. } => {
            *x = old_w - *x;
            *y = old_h - *y;
            *rotation = (*rotation + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU);
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
            *rotation = (*rotation + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU);
        }
        DrawObject::Connector {
            start,
            end,
            control_points,
            ..
        } => {
            if let crate::model::ConnectorEnd::Free { x, y } = start {
                *x = old_w - *x;
                *y = old_h - *y;
            }
            for cp in control_points {
                cp.x = old_w - cp.x;
                cp.y = old_h - cp.y;
            }
            if let crate::model::ConnectorEnd::Free { x, y } = end {
                *x = old_w - *x;
                *y = old_h - *y;
            }
        }
    }
}

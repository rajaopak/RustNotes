use super::primitives::{
    curve_shape_polyline, equation_approx_size, object_boundary_vertices_with_context,
    object_bounds, object_bounds_with_context, point_in_polygon, point_strictly_in_polygon,
    point_to_polygon_dist, point_to_polygon_outline_dist, point_to_segment_dist,
    prepare_lasso_polygon, segment_to_segment_dist, segments_cross, segments_properly_cross,
    shape_vertices, stroke_visual_samples, text_approx_size,
};
use super::tessellation::stroke_radius_at_pressure;
use crate::model::{DrawObject, LassoSelectionMode, ShapeKind};
use eframe::egui::{Pos2, Rect, Vec2};

/// Menghitung jarak terdekat dari titik `p` ke segmen kapsul meruncing (*tapered capsule*)
/// dengan interpolasi linier radius `ra` ke `rb` sepanjang proyeksi segmen `a -> b` (Track 3).
pub fn distance_to_stroke_segment_with_pressure(
    p: Pos2,
    a: Pos2,
    b: Pos2,
    ra: f32,
    rb: f32,
) -> f32 {
    let ab = b - a;
    let len_sq = ab.length_sq();
    if len_sq < 1e-6 {
        let r = ra.max(rb).max(0.0);
        return (p.distance(a) - r).max(0.0);
    }
    let t = ((p - a).dot(ab) / len_sq).clamp(0.0, 1.0);
    let proj = a + ab * t;
    let r_t = (ra + (rb - ra) * t).max(0.0);
    (p.distance(proj) - r_t).max(0.0)
}

/// Signed Distance Function (SDF) eksak untuk persegi panjang dengan sudut membulat (`corner_radius`).
/// Mengembalikan nilai `<= 0.0` di bagian dalam, `0.0` tepat di garis tepi, dan `> 0.0` di luar.
fn rounded_rect_sdf(p_local: Pos2, rect: Rect, corner_radius: f32) -> f32 {
    let center = rect.center();
    let half = rect.size() * 0.5;
    let r = corner_radius.clamp(0.0, half.x.min(half.y).max(0.0));
    let q = (p_local - center).abs() - (half - Vec2::splat(r));
    let outside_dist = Vec2::new(q.x.max(0.0), q.y.max(0.0)).length();
    let inside_dist = q.x.max(q.y).min(0.0);
    outside_dist + inside_dist - r
}

/// Menghitung jarak Euclidean terdekat dari `p_local` ke garis keliling elips berpusat di `center`
/// dengan setengah sumbu `rx` dan `ry` secara akurat untuk rasio aspek apa pun.
fn distance_to_ellipse_boundary(p_local: Pos2, center: Pos2, rx: f32, ry: f32) -> f32 {
    let rx = rx.abs();
    let ry = ry.abs();
    if rx < 1e-4 && ry < 1e-4 {
        return p_local.distance(center);
    }
    if rx < 1e-4 {
        return point_to_segment_dist(
            p_local,
            Pos2::new(center.x, center.y - ry),
            Pos2::new(center.x, center.y + ry),
        );
    }
    if ry < 1e-4 {
        return point_to_segment_dist(
            p_local,
            Pos2::new(center.x - rx, center.y),
            Pos2::new(center.x + rx, center.y),
        );
    }
    let px = (p_local.x - center.x).abs();
    let py = (p_local.y - center.y).abs();
    if (rx - ry).abs() < 1e-4 {
        return (Vec2::new(px, py).length() - rx).abs();
    }
    let mut theta = (rx * py)
        .atan2(ry * px)
        .clamp(0.0, std::f32::consts::FRAC_PI_2);
    for _ in 0..4 {
        let (s, c) = theta.sin_cos();
        let ex = rx * c;
        let ey = ry * s;
        let dx = ex - px;
        let dy = ey - py;
        let g = -dx * rx * s + dy * ry * c;
        let g_prime = (rx * rx * s * s + ry * ry * c * c) - dx * rx * c - dy * ry * s;
        if g_prime.abs() > 1e-6 {
            theta = (theta - g / g_prime).clamp(0.0, std::f32::consts::FRAC_PI_2);
        }
    }
    let (s, c) = theta.sin_cos();
    let mut best_dist = Vec2::new(px - rx * c, py - ry * s).length();
    for i in 0..=16 {
        let t = (i as f32) * (std::f32::consts::FRAC_PI_2 / 16.0);
        let (ts, tc) = t.sin_cos();
        let d = Vec2::new(px - rx * tc, py - ry * ts).length();
        if d < best_dist {
            best_dist = d;
        }
    }
    best_dist
}

/// Menghitung jarak Euclidean terdekat dari titik `p` ke geometri objek `obj`.
/// Menggunakan transformasi invers koordinat lokal agar rotasi objek dihitung secara presisi.
pub fn distance_to_object(obj: &DrawObject, p: Pos2) -> Option<f32> {
    distance_to_object_with_context(obj, p, None)
}

/// Menghitung jarak Euclidean terdekat dari titik `p` ke geometri visual objek `obj`
/// dengan konteks daftar objek halaman (untuk resolusi konektor `Attached` dan jalur rute).
pub fn distance_to_object_with_context(
    obj: &DrawObject,
    p: Pos2,
    all_objects: Option<&[DrawObject]>,
) -> Option<f32> {
    let b = match all_objects {
        Some(objs) => object_bounds_with_context(obj, objs),
        None => object_bounds(obj),
    };
    if !b.is_finite() {
        return None;
    }
    match obj {
        DrawObject::Stroke {
            points,
            width,
            is_highlighter,
            ..
        } => {
            if points.is_empty() {
                return None;
            }
            let samples = stroke_visual_samples(points);
            if samples.len() == 1 {
                let r = stroke_radius_at_pressure(*width, samples[0].pressure, *is_highlighter);
                return Some((p.distance(samples[0].to_pos2()) - r).max(0.0));
            }
            let mut min_dist = f32::INFINITY;
            for i in 1..samples.len() {
                let a = samples[i - 1].to_pos2();
                let b_pt = samples[i].to_pos2();
                let ra =
                    stroke_radius_at_pressure(*width, samples[i - 1].pressure, *is_highlighter);
                let rb = stroke_radius_at_pressure(*width, samples[i].pressure, *is_highlighter);
                let d = distance_to_stroke_segment_with_pressure(p, a, b_pt, ra, rb);
                if d < min_dist {
                    min_dist = d;
                }
            }
            Some(min_dist)
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
            corner_radius,
            curve_points,
            ..
        } => {
            let half_w = (*stroke_width * 0.5).max(0.0);
            if *shape_type == ShapeKind::Curve {
                let poly = curve_shape_polyline(*x, *y, *width, *height, *rotation, curve_points);
                if poly.is_empty() {
                    return None;
                }
                if poly.len() == 1 {
                    return Some((p.distance(poly[0]) - half_w).max(0.0));
                }
                let mut min_d = f32::INFINITY;
                for i in 1..poly.len() {
                    min_d = min_d.min(point_to_segment_dist(p, poly[i - 1], poly[i]));
                }
                return Some((min_d - half_w).max(0.0));
            }

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

            let local_rect =
                Rect::from_two_pos(Pos2::new(*x, *y), Pos2::new(*x + *width, *y + *height));

            match shape_type {
                ShapeKind::Line | ShapeKind::Curve => {
                    let a = Pos2::new(*x, *y);
                    let b_pt = Pos2::new(*x + *width, *y + *height);
                    Some((point_to_segment_dist(p_local, a, b_pt) - half_w).max(0.0))
                }
                ShapeKind::Rectangle => {
                    let sdf = rounded_rect_sdf(p_local, local_rect, *corner_radius);
                    if fill.is_some() {
                        if sdf <= 0.0 {
                            Some(0.0)
                        } else {
                            Some((sdf - half_w).max(0.0))
                        }
                    } else {
                        Some((sdf.abs() - half_w).max(0.0))
                    }
                }
                ShapeKind::Ellipse => {
                    let rx = width.abs() * 0.5;
                    let ry = height.abs() * 0.5;
                    if fill.is_some() && rx > 1e-4 && ry > 1e-4 {
                        let dx = (p_local.x - center.x) / rx;
                        let dy = (p_local.y - center.y) / ry;
                        if dx * dx + dy * dy <= 1.0 {
                            return Some(0.0);
                        }
                    }
                    let edge_dist = distance_to_ellipse_boundary(p_local, center, rx, ry);
                    Some((edge_dist - half_w).max(0.0))
                }
                ShapeKind::Triangle
                | ShapeKind::RightTriangle
                | ShapeKind::ScaleneTriangle { .. }
                | ShapeKind::Parallelogram
                | ShapeKind::Diamond => {
                    let verts = shape_vertices(*shape_type, *x, *y, *width, *height);
                    if fill.is_some() && point_in_polygon(p_local, &verts) {
                        return Some(0.0);
                    }
                    let edge_dist = point_to_polygon_outline_dist(p_local, &verts);
                    Some((edge_dist - half_w).max(0.0))
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
            let local_rect = Rect::from_min_size(p0, Vec2::new(w.abs(), h.abs()));
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
            let local_rect = Rect::from_center_size(center, Vec2::new(width.abs(), height.abs()));
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
            let local_rect = Rect::from_center_size(center, Vec2::new(w.abs(), h.abs()));
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
            let both_free = matches!(
                (start, end),
                (
                    crate::model::ConnectorEnd::Free { .. },
                    crate::model::ConnectorEnd::Free { .. }
                )
            );
            let poly_pts: Vec<Pos2> = if let Some(objects) = all_objects
                && let Some(poly) = super::connectors::connector_polyline(obj, objects)
            {
                poly.into_iter().map(|pt| pt.to_pos2()).collect()
            } else if both_free && let Some(poly) = super::connectors::connector_polyline(obj, &[])
            {
                poly.into_iter().map(|pt| pt.to_pos2()).collect()
            } else {
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
            };

            if poly_pts.is_empty() {
                return if b.contains(p) { Some(0.0) } else { None };
            }
            let half_w = (style.stroke_width * 0.5).max(0.0);
            if poly_pts.len() == 1 {
                return Some(((p - poly_pts[0]).length() - half_w).max(0.0));
            }
            let mut min_d = f32::INFINITY;
            for i in 1..poly_pts.len() {
                min_d = min_d.min(point_to_segment_dist(p, poly_pts[i - 1], poly_pts[i]));
            }
            let mut best = (min_d - half_w).max(0.0);

            // Periksa juga geometri ujung panah (arrowhead) jika aktif
            if style.end_marker == crate::model::MarkerKind::Arrow {
                let last = poly_pts.len() - 1;
                let tip = crate::model::Point2D::from(poly_pts[last]);
                let prev = crate::model::Point2D::from(poly_pts[last - 1]);
                let arrow = super::connectors::arrowhead_geometry(tip, tip - prev, 12.0, 9.0);
                let arrow_poly = [arrow[0].to_pos2(), arrow[1].to_pos2(), arrow[2].to_pos2()];
                best = best.min(point_to_polygon_dist(p, &arrow_poly));
            }
            if style.start_marker == crate::model::MarkerKind::Arrow {
                let tip = crate::model::Point2D::from(poly_pts[0]);
                let next = crate::model::Point2D::from(poly_pts[1]);
                let arrow = super::connectors::arrowhead_geometry(tip, tip - next, 12.0, 9.0);
                let arrow_poly = [arrow[0].to_pos2(), arrow[1].to_pos2(), arrow[2].to_pos2()];
                best = best.min(point_to_polygon_dist(p, &arrow_poly));
            }
            Some(best)
        }
    }
}

/// Menguji apakah titik `p` mengenai objek `obj` dengan toleransi `tolerance`.
/// Menggunakan representasi geometri kanonik yang sama dengan `distance_to_object`.
pub fn hit_test_object(obj: &DrawObject, p: Pos2, tolerance: f32) -> bool {
    hit_test_object_with_context(obj, p, tolerance, None)
}

/// Menguji apakah titik `p` mengenai objek `obj` dengan toleransi `tolerance` dan konteks objek halaman.
pub fn hit_test_object_with_context(
    obj: &DrawObject,
    p: Pos2,
    tolerance: f32,
    all_objects: Option<&[DrawObject]>,
) -> bool {
    let tol = tolerance.max(0.0);
    let b = match all_objects {
        Some(objs) => object_bounds_with_context(obj, objs),
        None => object_bounds(obj),
    };
    let pad = match obj {
        DrawObject::Stroke { width, .. } => (*width * 0.25).max(1.0),
        DrawObject::Connector { .. } => 12.0,
        _ => 0.0,
    };
    if !b.expand(tol + pad).contains(p) {
        return false;
    }
    distance_to_object_with_context(obj, p, all_objects).is_some_and(|d| d <= tol)
}

/// Memeriksa apakah suatu objek memotong atau berada di dalam poligon lasso (Intersection Mode).
pub fn object_in_lasso(obj: &DrawObject, polygon: &[Pos2]) -> bool {
    object_in_lasso_with_context(obj, polygon, None)
}

/// Memeriksa apakah suatu objek memotong atau berada di dalam poligon lasso (Intersection Mode)
/// dengan konteks daftar objek seluruh halaman (untuk resolusi konektor diagram).
pub fn object_in_lasso_with_context(
    obj: &DrawObject,
    polygon: &[Pos2],
    all_objects: Option<&[DrawObject]>,
) -> bool {
    let Some((clean_poly, lasso_bounds)) = prepare_lasso_polygon(polygon) else {
        return false;
    };
    object_in_lasso_with_prepared_polygon(obj, &clean_poly, lasso_bounds, all_objects)
}

/// Evaluasi Intersection Mode menggunakan poligon lasso yang telah disanitasi dan `lasso_bounds` (Track 4).
pub fn object_in_lasso_with_prepared_polygon(
    obj: &DrawObject,
    polygon: &[Pos2],
    lasso_bounds: Rect,
    all_objects: Option<&[DrawObject]>,
) -> bool {
    if polygon.len() < 3 {
        return false;
    }

    let obj_b = match all_objects {
        Some(objs) => object_bounds_with_context(obj, objs),
        None => object_bounds(obj),
    };
    let pad = match obj {
        DrawObject::Stroke { width, .. } => (*width * 0.25).max(1.0),
        DrawObject::Connector { .. } => 14.0,
        _ => 1.0,
    };
    if !obj_b.is_finite() || !obj_b.expand(pad).intersects(lasso_bounds) {
        return false;
    }

    // 1. Jika salah satu simpul poligon lasso berada di dalam atau menyentuh geometri objek
    if polygon
        .iter()
        .any(|&lp| hit_test_object_with_context(obj, lp, 0.0, all_objects))
    {
        return true;
    }

    let verts = object_boundary_vertices_with_context(obj, all_objects);
    if verts.is_empty() {
        return false;
    }

    // 2. Jika ada simpul batas objek yang berada di dalam atau pada batas poligon lasso
    if verts.iter().any(|&v| point_in_polygon(v, polygon)) {
        return true;
    }

    // 3. Jika ada segmen batas objek yang memotong atau menyentuh segmen poligon lasso
    let is_closed = match obj {
        DrawObject::Stroke { .. } | DrawObject::Connector { .. } => false,
        DrawObject::Shape { shape_type, .. } => {
            !matches!(*shape_type, ShapeKind::Line | ShapeKind::Curve)
        }
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

    // 4. Ketebalan & pressure per-titik untuk DrawObject::Stroke (pita kapsul visual & sampel asli)
    if let DrawObject::Stroke {
        points,
        width,
        is_highlighter,
        ..
    } = obj
    {
        let samples = stroke_visual_samples(points);
        for slice in [&samples[..], points.as_slice()] {
            for p in polygon {
                for i in 1..slice.len() {
                    let a = slice[i - 1].to_pos2();
                    let b = slice[i].to_pos2();
                    let ra =
                        stroke_radius_at_pressure(*width, slice[i - 1].pressure, *is_highlighter);
                    let rb = stroke_radius_at_pressure(*width, slice[i].pressure, *is_highlighter);
                    if distance_to_stroke_segment_with_pressure(*p, a, b, ra, rb) <= 0.0 {
                        return true;
                    }
                }
            }
            for pt in slice {
                let p = pt.to_pos2();
                let r = stroke_radius_at_pressure(*width, pt.pressure, *is_highlighter);
                for j in 0..polygon.len() {
                    let l1 = polygon[j];
                    let l2 = polygon[(j + 1) % polygon.len()];
                    if point_to_segment_dist(p, l1, l2) <= r {
                        return true;
                    }
                }
            }
        }
    }

    // 5. Ketebalan garis untuk DrawObject::Shape dan DrawObject::Connector
    let edge_half_w = match obj {
        DrawObject::Shape { stroke_width, .. } => *stroke_width * 0.5,
        DrawObject::Connector { style, .. } => style.stroke_width * 0.5,
        _ => 0.0,
    };
    if edge_half_w > 0.0 {
        for i in 0..seg_count {
            let p1 = verts[i];
            let p2 = verts[(i + 1) % verts.len()];
            for j in 0..polygon.len() {
                let l1 = polygon[j];
                let l2 = polygon[(j + 1) % polygon.len()];
                if segment_to_segment_dist(p1, p2, l1, l2) <= edge_half_w {
                    return true;
                }
            }
        }
    }

    false
}

/// Memeriksa apakah suatu objek secara penuh dilingkupi di dalam batas poligon lasso (Containment Mode).
/// Menjamin seluruh geometri terlihat objek berada di dalam atau tepat pada batas poligon lasso.
pub fn object_contained_in_lasso(obj: &DrawObject, polygon: &[Pos2]) -> bool {
    object_contained_in_lasso_with_context(obj, polygon, None)
}

/// Memeriksa apakah suatu objek secara penuh dilingkupi di dalam batas poligon lasso (Containment Mode)
/// dengan konteks daftar objek seluruh halaman (untuk resolusi konektor diagram).
pub fn object_contained_in_lasso_with_context(
    obj: &DrawObject,
    polygon: &[Pos2],
    all_objects: Option<&[DrawObject]>,
) -> bool {
    let Some((clean_poly, lasso_bounds)) = prepare_lasso_polygon(polygon) else {
        return false;
    };
    object_contained_in_lasso_with_prepared_polygon(obj, &clean_poly, lasso_bounds, all_objects)
}

/// Evaluasi Containment Mode menggunakan poligon lasso yang telah disanitasi dan `lasso_bounds` (Track 4).
pub fn object_contained_in_lasso_with_prepared_polygon(
    obj: &DrawObject,
    polygon: &[Pos2],
    lasso_bounds: Rect,
    all_objects: Option<&[DrawObject]>,
) -> bool {
    if polygon.len() < 3 {
        return false;
    }

    let verts = object_boundary_vertices_with_context(obj, all_objects);
    if verts.is_empty() {
        return false;
    }

    // 1. Broad-phase vertex-bounds culling terhadap lasso_bounds
    let mut vert_min_x = f32::INFINITY;
    let mut vert_max_x = f32::NEG_INFINITY;
    let mut vert_min_y = f32::INFINITY;
    let mut vert_max_y = f32::NEG_INFINITY;
    for v in &verts {
        vert_min_x = vert_min_x.min(v.x);
        vert_max_x = vert_max_x.max(v.x);
        vert_min_y = vert_min_y.min(v.y);
        vert_max_y = vert_max_y.max(v.y);
    }

    if vert_min_x < lasso_bounds.min.x - 0.5
        || vert_max_x > lasso_bounds.max.x + 0.5
        || vert_min_y < lasso_bounds.min.y - 0.5
        || vert_max_y > lasso_bounds.max.y + 0.5
    {
        return false;
    }

    // 2. Evaluasi footprint presisi untuk DrawObject::Stroke (termasuk radius tekanan pena & ujung membulat)
    if let DrawObject::Stroke {
        points,
        width,
        is_highlighter,
        ..
    } = obj
    {
        if points.is_empty() {
            return false;
        }
        let samples = stroke_visual_samples(points);
        const DIAG: f32 = std::f32::consts::FRAC_1_SQRT_2;
        for pt in samples.iter().chain(points.iter()) {
            let p = pt.to_pos2();
            if !point_in_polygon(p, polygon) {
                return false;
            }
            let r = stroke_radius_at_pressure(*width, pt.pressure, *is_highlighter);
            let d = r * DIAG;
            let perimeter = [
                p + Vec2::new(r, 0.0),
                p - Vec2::new(r, 0.0),
                p + Vec2::new(0.0, r),
                p - Vec2::new(0.0, r),
                p + Vec2::new(d, d),
                p + Vec2::new(-d, d),
                p + Vec2::new(d, -d),
                p + Vec2::new(-d, -d),
            ];
            if !perimeter.iter().all(|&c| point_in_polygon(c, polygon)) {
                return false;
            }
            // Pastikan cakram berjari-jari r tidak melampaui segmen batas lasso mana pun
            for j in 0..polygon.len() {
                let l1 = polygon[j];
                let l2 = polygon[(j + 1) % polygon.len()];
                if point_to_segment_dist(p, l1, l2) + 1e-3 < r {
                    return false;
                }
            }
        }
        if samples.len() == 1 {
            return true;
        }

        for i in 1..samples.len() {
            let a = samples[i - 1].to_pos2();
            let b = samples[i].to_pos2();
            let mid = a + (b - a) * 0.5;
            if !point_in_polygon(mid, polygon) {
                return false;
            }
            let ra = stroke_radius_at_pressure(*width, samples[i - 1].pressure, *is_highlighter);
            let rb = stroke_radius_at_pressure(*width, samples[i].pressure, *is_highlighter);
            let ab = b - a;
            let len_sq = ab.length_sq();
            for j in 0..polygon.len() {
                let l1 = polygon[j];
                let l2 = polygon[(j + 1) % polygon.len()];
                if segments_properly_cross(a, b, l1, l2) {
                    return false;
                }
                if len_sq > 1e-6 {
                    let t = ((l1 - a).dot(ab) / len_sq).clamp(0.0, 1.0);
                    let proj = a + ab * t;
                    let r_t = ra + (rb - ra) * t;
                    if l1.distance(proj) + 1e-3 < r_t {
                        return false;
                    }
                }
            }
        }
        return true;
    }

    // 3. Seluruh simpul batas objek harus berada di dalam atau pada batas poligon
    if !verts.iter().all(|&v| point_in_polygon(v, polygon)) {
        return false;
    }

    // 4. Periksa setiap segmen batas objek: titik interior segmen harus berada di dalam poligon
    //    dan tidak boleh ada segmen yang memotong keluar batas poligon secara transversal.
    let is_closed = match obj {
        DrawObject::Stroke { .. } | DrawObject::Connector { .. } => false,
        DrawObject::Shape { shape_type, .. } => {
            !matches!(*shape_type, ShapeKind::Line | ShapeKind::Curve)
        }
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
        let d = p2 - p1;
        for frac in [0.25_f32, 0.5_f32, 0.75_f32] {
            if !point_in_polygon(p1 + d * frac, polygon) {
                return false;
            }
        }
        for j in 0..polygon.len() {
            let l1 = polygon[j];
            let l2 = polygon[(j + 1) % polygon.len()];
            if segments_properly_cross(p1, p2, l1, l2) {
                return false;
            }
        }
    }

    // 5. Untuk objek poligon tertutup, pastikan tidak ada simpul poligon lasso yang menusuk ke dalam interior objek
    if is_closed && verts.len() >= 3 {
        for &lp in polygon {
            if point_strictly_in_polygon(lp, &verts) {
                return false;
            }
        }
    }

    true
}

/// Memilih objek menggunakan mode lasso yang dikonfigurasi.
pub fn object_in_lasso_mode(obj: &DrawObject, polygon: &[Pos2], mode: LassoSelectionMode) -> bool {
    object_in_lasso_mode_with_context(obj, polygon, mode, None)
}

/// Memilih objek menggunakan mode lasso yang dikonfigurasi dengan konteks objek seluruh halaman.
pub fn object_in_lasso_mode_with_context(
    obj: &DrawObject,
    polygon: &[Pos2],
    mode: LassoSelectionMode,
    all_objects: Option<&[DrawObject]>,
) -> bool {
    let Some((clean_poly, lasso_bounds)) = prepare_lasso_polygon(polygon) else {
        return false;
    };
    object_in_lasso_mode_with_prepared_polygon(obj, &clean_poly, lasso_bounds, mode, all_objects)
}

/// Evaluasi seleksi lasso untuk sebuah objek menggunakan poligon yang sudah disanitasi dan `lasso_bounds` (Track 4).
pub fn object_in_lasso_mode_with_prepared_polygon(
    obj: &DrawObject,
    clean_poly: &[Pos2],
    lasso_bounds: Rect,
    mode: LassoSelectionMode,
    all_objects: Option<&[DrawObject]>,
) -> bool {
    match mode {
        LassoSelectionMode::Containment => object_contained_in_lasso_with_prepared_polygon(
            obj,
            clean_poly,
            lasso_bounds,
            all_objects,
        ),
        LassoSelectionMode::Intersection => {
            object_in_lasso_with_prepared_polygon(obj, clean_poly, lasso_bounds, all_objects)
        }
    }
}

/// Mengevaluasi seleksi lasso secara brute-force pada seluruh daftar objek halaman (sebagai oracle validasi & fallback).
/// Mengembalikan indeks objek yang terpilih dalam urutan Z-order dari bawah ke atas.
pub fn select_objects_in_lasso_brute_force(
    objects: &[DrawObject],
    polygon: &[Pos2],
    mode: LassoSelectionMode,
) -> Vec<usize> {
    let Some((clean_poly, lasso_bounds)) = prepare_lasso_polygon(polygon) else {
        return Vec::new();
    };
    objects
        .iter()
        .enumerate()
        .filter_map(|(idx, obj)| {
            object_in_lasso_mode_with_prepared_polygon(
                obj,
                &clean_poly,
                lasso_bounds,
                mode,
                Some(objects),
            )
            .then_some(idx)
        })
        .collect()
}

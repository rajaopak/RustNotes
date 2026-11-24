use crate::model::{DrawObject, Point2D, ShapeKind, StrokePoint};
use eframe::egui::{Pos2, Rect, Vec2};

/// Menghasilkan sampel goresan kanonik (setelah adaptive smoothing dan stabilisasi pressure)
/// yang menyatukan representasi geometri antara rendering (`render_stroke_object`) dan hit-testing (Track 3).
pub fn stroke_visual_samples(points: &[StrokePoint]) -> Vec<StrokePoint> {
    if points.len() <= 2 {
        points
            .iter()
            .map(|p| StrokePoint {
                x: p.x,
                y: p.y,
                pressure: super::tessellation::sanitize_pressure(p.pressure),
            })
            .collect()
    } else {
        super::tessellation::smooth_stroke_points(points, 3)
    }
}

/// Menghasilkan rangkaian titik polyline kanonik untuk `ShapeKind::Curve`
/// dengan memperhitungkan titik kontrol kurva Catmull-Rom/Bézier dan rotasi terhadap pusat bentuk (Track 3).
pub fn curve_shape_polyline(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    rotation: f32,
    curve_points: &[Point2D],
) -> Vec<Pos2> {
    let center = Pos2::new(x + width * 0.5, y + height * 0.5);
    let cos_a = rotation.cos();
    let sin_a = rotation.sin();
    let rot_pt = |p: Pos2| -> Pos2 {
        if rotation.abs() > 1e-5 {
            let dx = p.x - center.x;
            let dy = p.y - center.y;
            Pos2::new(
                center.x + dx * cos_a - dy * sin_a,
                center.y + dx * sin_a + dy * cos_a,
            )
        } else {
            p
        }
    };

    if curve_points.len() >= 2 {
        let beziers = super::curves::fit_catmull_rom_to_bezier(curve_points);
        let mut all_pts = Vec::with_capacity(beziers.len() * 16 + 1);
        for (i, seg) in beziers.iter().enumerate() {
            let sampled = super::curves::sample_cubic_bezier(seg[0], seg[1], seg[2], seg[3], 16);
            let skip = if i == 0 { 0 } else { 1 };
            for p in sampled.into_iter().skip(skip) {
                all_pts.push(rot_pt(p.to_pos2()));
            }
        }
        if all_pts.len() >= 2 {
            return all_pts;
        }
    }

    vec![
        rot_pt(Pos2::new(x, y)),
        rot_pt(Pos2::new(x + width, y + height)),
    ]
}

pub fn text_approx_size(text: &str, font_size: f32) -> (f32, f32) {
    let lines: Vec<&str> = text.lines().collect();
    let line_count = lines.len().max(1) as f32;
    let max_line_len = lines.iter().map(|l| l.chars().count()).max().unwrap_or(1) as f32;
    let safe_fs = if font_size.is_finite() {
        font_size.clamp(4.0, 200.0)
    } else {
        18.0
    };
    let approx_w = (max_line_len * (safe_fs * 0.58)).max(16.0);
    let approx_h = (line_count * (safe_fs * 1.35)).max(12.0);
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
            curve_points,
            ..
        } => {
            let half = *stroke_width / 2.0;
            if *shape_type == ShapeKind::Curve {
                let poly = curve_shape_polyline(*x, *y, *width, *height, *rotation, curve_points);
                let mut min_x = f32::INFINITY;
                let mut min_y = f32::INFINITY;
                let mut max_x = f32::NEG_INFINITY;
                let mut max_y = f32::NEG_INFINITY;
                for p in poly {
                    min_x = min_x.min(p.x);
                    min_y = min_y.min(p.y);
                    max_x = max_x.max(p.x);
                    max_y = max_y.max(p.y);
                }
                if !min_x.is_finite() {
                    Rect::NOTHING
                } else {
                    Rect::from_min_max(
                        Pos2::new(min_x - half, min_y - half),
                        Pos2::new(max_x + half, max_y + half),
                    )
                }
            } else if *shape_type == ShapeKind::Line {
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
            let bb_w = width.abs() * cos_a + height.abs() * sin_a;
            let bb_h = width.abs() * sin_a + height.abs() * cos_a;
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
            let both_free = matches!(
                (start, end),
                (
                    crate::model::ConnectorEnd::Free { .. },
                    crate::model::ConnectorEnd::Free { .. }
                )
            );
            let mut pts = Vec::new();
            if both_free && let Some(poly) = super::connectors::connector_polyline(obj, &[]) {
                for p in poly {
                    pts.push(p.to_pos2());
                }
            }
            if pts.is_empty() {
                if let crate::model::ConnectorEnd::Free { x, y } = start {
                    pts.push(Pos2::new(*x, *y));
                }
                if let crate::model::ConnectorEnd::Free { x, y } = end {
                    pts.push(Pos2::new(*x, *y));
                }
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

/// Menghitung bounding box objek dengan konteks seluruh objek pada halaman,
/// sehingga Connector yang ujungnya terikat (Attached) maupun Orthogonal/Curved memiliki bounds akurat.
pub fn object_bounds_with_context(obj: &DrawObject, all_objects: &[DrawObject]) -> Rect {
    if let DrawObject::Connector {
        start,
        end,
        control_points,
        style,
        ..
    } = obj
    {
        let mut min_x = f32::INFINITY;
        let mut min_y = f32::INFINITY;
        let mut max_x = f32::NEG_INFINITY;
        let mut max_y = f32::NEG_INFINITY;
        if let Some(poly) = super::connectors::connector_polyline(obj, all_objects) {
            for p in poly {
                min_x = min_x.min(p.x);
                min_y = min_y.min(p.y);
                max_x = max_x.max(p.x);
                max_y = max_y.max(p.y);
            }
        } else {
            let ((p_start, _), (p_end, _)) =
                super::connectors::resolve_connector_endpoints(start, end, all_objects);
            min_x = p_start.x.min(p_end.x);
            min_y = p_start.y.min(p_end.y);
            max_x = p_start.x.max(p_end.x);
            max_y = p_start.y.max(p_end.y);
        }
        for cp in control_points {
            min_x = min_x.min(cp.x);
            min_y = min_y.min(cp.y);
            max_x = max_x.max(cp.x);
            max_y = max_y.max(cp.y);
        }
        let half = style.stroke_width * 0.5 + 4.0;
        return Rect::from_min_max(
            Pos2::new(min_x - half, min_y - half),
            Pos2::new(max_x + half, max_y + half),
        );
    }
    object_bounds(obj)
}

pub fn combined_bounds(objects: &[&DrawObject]) -> Option<Rect> {
    let mut combined: Option<Rect> = None;
    for obj in objects {
        let b = object_bounds(obj);
        if b.is_finite() && b.min.x <= b.max.x && b.min.y <= b.max.y {
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

/// Menghitung jarak terdekat antara dua segmen garis `a1-a2` dan `b1-b2` (Track 3).
pub fn segment_to_segment_dist(a1: Pos2, a2: Pos2, b1: Pos2, b2: Pos2) -> f32 {
    if segments_cross(a1, a2, b1, b2) {
        return 0.0;
    }
    point_to_segment_dist(a1, b1, b2)
        .min(point_to_segment_dist(a2, b1, b2))
        .min(point_to_segment_dist(b1, a1, a2))
        .min(point_to_segment_dist(b2, a1, a2))
}

/// Menghitung jarak terdekat dari titik `p` ke garis batas luar poligon tertutup (Track 3).
pub fn point_to_polygon_outline_dist(p: Pos2, polygon: &[Pos2]) -> f32 {
    match polygon.len() {
        0 => f32::INFINITY,
        1 => p.distance(polygon[0]),
        2 => point_to_segment_dist(p, polygon[0], polygon[1]),
        n => {
            let mut min_d = f32::INFINITY;
            for i in 0..n {
                let d = point_to_segment_dist(p, polygon[i], polygon[(i + 1) % n]);
                if d < min_d {
                    min_d = d;
                }
            }
            min_d
        }
    }
}

/// Menghitung jarak terdekat dari titik `p` ke poligon tertutup (`0.0` jika di dalam atau tepat di tepi) (Track 3).
pub fn point_to_polygon_dist(p: Pos2, polygon: &[Pos2]) -> f32 {
    if polygon.len() >= 3 && point_in_polygon(p, polygon) {
        return 0.0;
    }
    point_to_polygon_outline_dist(p, polygon)
}

pub fn shape_vertices(shape_type: ShapeKind, x: f32, y: f32, width: f32, height: f32) -> Vec<Pos2> {
    let p1 = Pos2::new(x, y);
    let p2 = Pos2::new(x + width, y + height);
    let r = Rect::from_two_pos(p1, p2);
    match shape_type {
        ShapeKind::Line | ShapeKind::Curve => vec![p1, p2],
        ShapeKind::Rectangle => vec![
            r.left_top(),
            r.right_top(),
            r.right_bottom(),
            r.left_bottom(),
        ],
        ShapeKind::Ellipse => {
            let center = r.center();
            let rx = (r.width() * 0.5).abs();
            let ry = (r.height() * 0.5).abs();
            let mut pts = Vec::with_capacity(32);
            for i in 0..32 {
                let theta = (i as f32) * (std::f32::consts::TAU / 32.0);
                pts.push(Pos2::new(
                    center.x + rx * theta.cos(),
                    center.y + ry * theta.sin(),
                ));
            }
            pts
        }
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

/// Menyaring, membersihkan, dan memvalidasi rangkaian titik mentah poligon Lasso (Track 4).
/// - Menghapus koordinat non-finite dan simpul berurutan yang berhimpit.
/// - Menyederhanakan titik kolinear mikro secara adaptif tanpa mengubah bentuk atau sudut belokan.
/// - Menolak poligon degenerasi (kurang dari 3 simpul unik atau seluruh simpul segaris/kolinear).
/// - Mengembalikan `(poligon_bersih, bounding_box_poligon)`.
pub fn prepare_lasso_polygon(raw_points: &[Pos2]) -> Option<(Vec<Pos2>, Rect)> {
    if raw_points.len() < 3 {
        return None;
    }
    let mut deduped: Vec<Pos2> = Vec::with_capacity(raw_points.len());
    for &p in raw_points {
        if !p.x.is_finite() || !p.y.is_finite() {
            continue;
        }
        if deduped
            .last()
            .is_none_or(|&prev| (p - prev).length_sq() > 1e-6)
        {
            deduped.push(p);
        }
    }
    while deduped.len() >= 2 && (*deduped.last().unwrap() - deduped[0]).length_sq() <= 1e-6 {
        deduped.pop();
    }
    if deduped.len() < 3 {
        return None;
    }

    // Simplifikasi adaptif non-destruktif untuk jalur sangat padat (> 32 titik):
    // hanya menggabungkan titik interior yang benar-benar kolinear (deviasi <= 0.05 pt) dan searah.
    let cleaned = if deduped.len() > 32 {
        let mut simplified = Vec::with_capacity(deduped.len());
        simplified.push(deduped[0]);
        for i in 1..deduped.len() - 1 {
            let prev = *simplified.last().unwrap();
            let curr = deduped[i];
            let next = deduped[i + 1];
            let v1 = curr - prev;
            let v2 = next - curr;
            let forward = v1.dot(v2) > 0.0;
            let near_collinear = point_to_segment_dist(curr, prev, next) <= 0.05;
            let short_span = (next - prev).length_sq() <= 64.0;
            if !(forward && near_collinear && short_span) {
                simplified.push(curr);
            }
        }
        simplified.push(*deduped.last().unwrap());
        simplified
    } else {
        deduped
    };

    if cleaned.len() < 3 {
        return None;
    }

    // Pastikan poligon tidak degenerasi menjadi garis lurus 1D (mendukung poligon cekung maupun figure-8 self-intersecting)
    let p0 = cleaned[0];
    let mut has_area = false;
    for i in 1..cleaned.len() - 1 {
        let v1 = cleaned[i] - p0;
        let v2 = cleaned[i + 1] - p0;
        if (v1.x * v2.y - v1.y * v2.x).abs() > 1e-4 {
            has_area = true;
            break;
        }
    }
    if !has_area {
        return None;
    }

    let mut min_x = f32::INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    for &p in &cleaned {
        min_x = min_x.min(p.x);
        min_y = min_y.min(p.y);
        max_x = max_x.max(p.x);
        max_y = max_y.max(p.y);
    }
    let bounds = Rect::from_min_max(Pos2::new(min_x, min_y), Pos2::new(max_x, max_y));
    Some((cleaned, bounds))
}

/// Menguji apakah titik `p` berada di dalam atau tepat pada batas poligon `polygon`
/// menggunakan aturan *Even-Odd Fill Rule* yang konsisten untuk poligon konveks, cekung,
/// urutan simpul CW/CCW, maupun poligon berpotongan diri (*self-intersecting*) (Track 3 & Track 4).
pub fn point_in_polygon(p: Pos2, polygon: &[Pos2]) -> bool {
    if polygon.len() < 3 || !p.x.is_finite() || !p.y.is_finite() {
        return false;
    }
    // Tolak poligon degenerasi yang seluruh simpulnya kolinear (luas 2D nol)
    let p0 = polygon[0];
    let mut has_area = false;
    for i in 1..polygon.len() - 1 {
        let v1 = polygon[i] - p0;
        let v2 = polygon[i + 1] - p0;
        if (v1.x * v2.y - v1.y * v2.x).abs() > 1e-4 {
            has_area = true;
            break;
        }
    }
    if !has_area {
        return false;
    }

    const BOUNDARY_EPS: f32 = 1e-4;
    let mut inside = false;
    let mut j = polygon.len() - 1;
    let mut valid_edges = 0usize;
    for i in 0..polygon.len() {
        let pi = polygon[i];
        let pj = polygon[j];
        if (pi - pj).length_sq() < 1e-10 {
            j = i;
            continue;
        }
        valid_edges += 1;
        // Titik tepat pada simpul (vertex) atau sisi (edge) poligon secara deterministik dianggap di dalam
        if point_to_segment_dist(p, pj, pi) <= BOUNDARY_EPS {
            return true;
        }
        let dy = pj.y - pi.y;
        if (pi.y > p.y) != (pj.y > p.y)
            && dy.abs() > 1e-9
            && p.x < (pj.x - pi.x) * (p.y - pi.y) / dy + pi.x
        {
            inside = !inside;
        }
        j = i;
    }
    if valid_edges < 3 {
        return false;
    }
    inside
}

/// Mengembalikan `true` hanya jika titik `p` berada secara ketat di bagian dalam poligon
/// (tidak berada di luar dan tidak menyentuh garis tepi/simpul poligon) (Track 4).
pub fn point_strictly_in_polygon(p: Pos2, polygon: &[Pos2]) -> bool {
    if polygon.len() < 3 {
        return false;
    }
    if point_to_polygon_outline_dist(p, polygon) <= 1e-3 {
        return false;
    }
    point_in_polygon(p, polygon)
}

/// Mengembalikan `true` hanya jika dua segmen garis saling memotong secara transversal
/// di bagian dalam kedua segmen (bukan sekadar menyentuh ujung atau berhimpit kolinear) (Track 4).
pub fn segments_properly_cross(a: Pos2, b: Pos2, c: Pos2, d: Pos2) -> bool {
    const EPS: f32 = 1e-4;
    let ab_min_x = a.x.min(b.x);
    let ab_max_x = a.x.max(b.x);
    let ab_min_y = a.y.min(b.y);
    let ab_max_y = a.y.max(b.y);
    let cd_min_x = c.x.min(d.x);
    let cd_max_x = c.x.max(d.x);
    let cd_min_y = c.y.min(d.y);
    let cd_max_y = c.y.max(d.y);
    if ab_max_x <= cd_min_x || cd_max_x <= ab_min_x || ab_max_y <= cd_min_y || cd_max_y <= ab_min_y
    {
        return false;
    }
    if point_to_segment_dist(a, c, d) <= EPS
        || point_to_segment_dist(b, c, d) <= EPS
        || point_to_segment_dist(c, a, b) <= EPS
        || point_to_segment_dist(d, a, b) <= EPS
    {
        return false;
    }
    let cross = |p1: Pos2, p2: Pos2, p3: Pos2| -> f32 {
        (p2.x - p1.x) * (p3.y - p1.y) - (p2.y - p1.y) * (p3.x - p1.x)
    };
    let d1 = cross(a, b, c);
    let d2 = cross(a, b, d);
    let d3 = cross(c, d, a);
    let d4 = cross(c, d, b);

    ((d1 > EPS && d2 < -EPS) || (d1 < -EPS && d2 > EPS))
        && ((d3 > EPS && d4 < -EPS) || (d3 < -EPS && d4 > EPS))
}

pub fn segments_cross(a: Pos2, b: Pos2, c: Pos2, d: Pos2) -> bool {
    const EPS: f32 = 1e-5;
    // Fast AABB rejection dengan toleransi EPS
    let ab_min_x = a.x.min(b.x) - EPS;
    let ab_max_x = a.x.max(b.x) + EPS;
    let ab_min_y = a.y.min(b.y) - EPS;
    let ab_max_y = a.y.max(b.y) + EPS;
    let cd_min_x = c.x.min(d.x) - EPS;
    let cd_max_x = c.x.max(d.x) + EPS;
    let cd_min_y = c.y.min(d.y) - EPS;
    let cd_max_y = c.y.max(d.y) + EPS;
    if ab_max_x < cd_min_x || cd_max_x < ab_min_x || ab_max_y < cd_min_y || cd_max_y < ab_min_y {
        return false;
    }

    let ab_len_sq = (b - a).length_sq();
    let cd_len_sq = (d - c).length_sq();
    if ab_len_sq <= EPS * EPS && cd_len_sq <= EPS * EPS {
        return a.distance(c) <= EPS;
    }
    if ab_len_sq <= EPS * EPS {
        return point_to_segment_dist(a, c, d) <= EPS;
    }
    if cd_len_sq <= EPS * EPS {
        return point_to_segment_dist(c, a, b) <= EPS;
    }

    // Endpoint touching, T-junction, atau collinear overlapping segments
    if point_to_segment_dist(a, c, d) <= EPS
        || point_to_segment_dist(b, c, d) <= EPS
        || point_to_segment_dist(c, a, b) <= EPS
        || point_to_segment_dist(d, a, b) <= EPS
    {
        return true;
    }

    let cross = |p1: Pos2, p2: Pos2, p3: Pos2| -> f32 {
        (p2.x - p1.x) * (p3.y - p1.y) - (p2.y - p1.y) * (p3.x - p1.x)
    };
    let d1 = cross(a, b, c);
    let d2 = cross(a, b, d);
    let d3 = cross(c, d, a);
    let d4 = cross(c, d, b);

    ((d1 > 0.0 && d2 < 0.0) || (d1 < 0.0 && d2 > 0.0))
        && ((d3 > 0.0 && d4 < 0.0) || (d3 < 0.0 && d4 > 0.0))
}

pub fn screen_to_page_point(screen_pos: Pos2, page_origin: Pos2, zoom: f32) -> Pos2 {
    let rel = screen_pos - page_origin;
    Pos2::new(rel.x / zoom, rel.y / zoom)
}

/// Menghitung simpul batas luar (boundary vertices/corners) dari suatu objek gambar
/// dengan memperhitungkan rotasi dan orientasi sebenarnya di kanvas.
pub fn object_boundary_vertices(obj: &DrawObject) -> Vec<Pos2> {
    object_boundary_vertices_with_context(obj, None)
}

/// Menghitung simpul batas luar dengan konteks daftar objek (diperlukan untuk
/// menyelesaikan posisi titik tempel diagram connector bertipe `Attached`).
pub fn object_boundary_vertices_with_context(
    obj: &DrawObject,
    all_objects: Option<&[DrawObject]>,
) -> Vec<Pos2> {
    match obj {
        DrawObject::Stroke { points, .. } => stroke_visual_samples(points)
            .into_iter()
            .map(|p| p.to_pos2())
            .collect(),
        DrawObject::Shape {
            x,
            y,
            width,
            height,
            rotation,
            shape_type,
            corner_radius,
            curve_points,
            ..
        } => {
            if *shape_type == ShapeKind::Curve {
                curve_shape_polyline(*x, *y, *width, *height, *rotation, curve_points)
            } else if *shape_type == ShapeKind::Line {
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
            } else if *shape_type == ShapeKind::Rectangle && *corner_radius > 0.0 {
                super::curves::rounded_rect_vertices(
                    *x,
                    *y,
                    *width,
                    *height,
                    *corner_radius,
                    *rotation,
                    4,
                )
                .into_iter()
                .map(|p| p.to_pos2())
                .collect()
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
            let both_free = matches!(
                (start, end),
                (
                    crate::model::ConnectorEnd::Free { .. },
                    crate::model::ConnectorEnd::Free { .. }
                )
            );
            if let Some(objects) = all_objects {
                if let Some(poly) = crate::geometry::connectors::connector_polyline(obj, objects) {
                    return poly.into_iter().map(|p| Pos2::new(p.x, p.y)).collect();
                }
            } else if both_free
                && let Some(poly) = crate::geometry::connectors::connector_polyline(obj, &[])
            {
                return poly.into_iter().map(|p| Pos2::new(p.x, p.y)).collect();
            }
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

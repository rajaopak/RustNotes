use super::primitives::point_to_segment_dist;
use crate::model::{ShapeKind, StrokePoint};
use eframe::egui::{Pos2, Vec2};

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

/// Algoritma pencocokan lingkaran kuadrat terkecil (Least-Squares Circle Fitting - Kåsa method).
/// Mengembalikan (center, radius, avg_radial_error, p90_radial_error).
pub fn fit_circle_kasa(points: &[Pos2]) -> Option<(Pos2, f32, f32, f32)> {
    let n = points.len();
    if n < 6 {
        return None;
    }
    let n_f = n as f32;
    let mut mean_x = 0.0f32;
    let mut mean_y = 0.0f32;
    for p in points {
        mean_x += p.x;
        mean_y += p.y;
    }
    mean_x /= n_f;
    mean_y /= n_f;

    let mut s_uu = 0.0f32;
    let mut s_vv = 0.0f32;
    let mut s_uv = 0.0f32;
    let mut s_uuu = 0.0f32;
    let mut s_vvv = 0.0f32;

    for p in points {
        let u = p.x - mean_x;
        let v = p.y - mean_y;
        let u2 = u * u;
        let v2 = v * v;
        let uv = u * v;
        let z = u2 + v2;
        s_uu += u2;
        s_vv += v2;
        s_uv += uv;
        s_uuu += u * z;
        s_vvv += v * z;
    }

    let det = s_uu * s_vv - s_uv * s_uv;
    if det.abs() < 1e-4 {
        return None;
    }

    let uc = (s_vv * s_uuu - s_uv * s_vvv) / (2.0 * det);
    let vc = (s_uu * s_vvv - s_uv * s_uuu) / (2.0 * det);

    let center = Pos2::new(mean_x + uc, mean_y + vc);
    let r_sq = uc * uc + vc * vc + (s_uu + s_vv) / n_f;
    if r_sq <= 0.0 {
        return None;
    }
    let radius = r_sq.sqrt();
    if radius < 4.0 || radius.is_nan() || radius.is_infinite() {
        return None;
    }

    let mut errors: Vec<f32> = points
        .iter()
        .map(|p| {
            let d = (p.x - center.x).hypot(p.y - center.y);
            ((d - radius) / radius).abs()
        })
        .collect();

    let avg_err = errors.iter().sum::<f32>() / n_f;
    errors.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let p90_idx = ((n as f32 * 0.90) as usize).min(n - 1);
    let p90_err = errors[p90_idx];

    Some((center, radius, avg_err, p90_err))
}

fn extract_polygon_corners(
    poly_points: &[Pos2],
    width: f32,
    height: f32,
    diagonal: f32,
) -> Vec<Pos2> {
    let min_dim = width.min(height).max(1.0);
    // Epsilon adaptif yang mempertimbangkan dimensi terkecil dan diagonal tanpa menghapus sudut di sisi sempit
    let epsilon = (min_dim * 0.15).min(diagonal * 0.045).clamp(2.5, 14.0);
    let mut verts = ramer_douglas_peucker(poly_points, epsilon);

    let close_thresh = (min_dim * 0.20).min(diagonal * 0.12).clamp(4.0, 25.0);
    if verts.len() > 1 && (*verts.first().unwrap() - *verts.last().unwrap()).length() < close_thresh
    {
        verts.pop();
    }

    let merge_dist = (min_dim * 0.12).min(diagonal * 0.05).clamp(3.0, 15.0);
    let mut merged: Vec<Pos2> = Vec::with_capacity(verts.len());
    for v in verts {
        if let Some(last) = merged.last_mut()
            && (*last - v).length() < merge_dist
        {
            *last = Pos2::new((last.x + v.x) * 0.5, (last.y + v.y) * 0.5);
            continue;
        }
        merged.push(v);
    }
    if merged.len() > 2
        && (*merged.first().unwrap() - *merged.last().unwrap()).length() < merge_dist
    {
        merged.pop();
    }

    // Pangkas titik-titik yang hampir kolinear (sudut belok < 25 derajat, cos > 0.90)
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
            let perp = Vec2::new(-u1.y, u1.x);
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

    // 3. Deteksi Bentuk Tertutup
    if close_dist / diagonal <= 0.38 {
        let center = Pos2::new((min_x + max_x) * 0.5, (min_y + max_y) * 0.5);

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

        let winding = total_angle.abs();
        if !(4.5..=8.0).contains(&winding) || abs_angle_sum > winding * 1.35 {
            return None;
        }

        // a. Uji Lingkaran (Circle) via Kåsa Least-Squares Fit
        let poly_points: Vec<Pos2> = points.iter().map(|p| p.to_pos2()).collect();
        let aspect = width / height;

        if let Some((circ_center, circ_radius, avg_circ_err, p90_circ_err)) =
            fit_circle_kasa(&poly_points)
        {
            // Jika aspect ratio mendekati 1.0 dan kesalahan radial rendah -> Lingkaran
            if (0.80..=1.25).contains(&aspect) && avg_circ_err < 0.12 && p90_circ_err < 0.20 {
                return Some(RecognizedShape {
                    kind: ShapeKind::Ellipse,
                    x: circ_center.x - circ_radius,
                    y: circ_center.y - circ_radius,
                    width: circ_radius * 2.0,
                    height: circ_radius * 2.0,
                });
            }
        }

        // b. Uji Elips (Ellipse)
        let rx = width * 0.5;
        let ry = height * 0.5;
        let mut ellipse_radial_errors = Vec::with_capacity(points.len());
        let mut quad_counts = [0usize; 4];

        for pt in points {
            let p = pt.to_pos2();
            let dx = p.x - center.x;
            let dy = p.y - center.y;
            let q_idx = match (dx >= 0.0, dy >= 0.0) {
                (true, true) => 0,
                (false, true) => 1,
                (false, false) => 2,
                (true, false) => 3,
            };
            quad_counts[q_idx] += 1;

            let norm_x = dx / rx.max(0.1);
            let norm_y = dy / ry.max(0.1);
            let dist_from_center_norm = (norm_x.powi(2) + norm_y.powi(2)).sqrt();
            let err = (dist_from_center_norm - 1.0).abs();
            ellipse_radial_errors.push(err);
        }

        let min_quad_pts = (points.len() as f32 * 0.08).max(2.0) as usize;
        let all_quads_populated = quad_counts.iter().all(|&c| c >= min_quad_pts);

        if all_quads_populated {
            let avg_ellipse_err: f32 =
                ellipse_radial_errors.iter().sum::<f32>() / ellipse_radial_errors.len() as f32;
            ellipse_radial_errors
                .sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let p90_idx = ((ellipse_radial_errors.len() as f32 * 0.90) as usize)
                .min(ellipse_radial_errors.len() - 1);
            let p90_ellipse_err = ellipse_radial_errors[p90_idx];

            if avg_ellipse_err < 0.12 && p90_ellipse_err < 0.22 {
                return Some(RecognizedShape {
                    kind: ShapeKind::Ellipse,
                    x: min_x,
                    y: min_y,
                    width,
                    height,
                });
            }
        }

        // c. Uji Poligon (Triangle, Parallelogram, Rectangle)
        let corners = extract_polygon_corners(&poly_points, width, height, diagonal);
        let poly_area = polygon_area(&poly_points).abs();
        let bbox_area = width * height;
        let fill_ratio = poly_area / bbox_area.max(1.0);

        if corners.len() == 3 {
            return Some(classify_triangle_corners(
                &corners, min_x, min_y, max_x, max_y,
            ));
        }

        if corners.len() >= 4 && corners.len() <= 6 {
            if fill_ratio < 0.54 {
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

                let l0 = e0.length();
                let l1 = e1.length();
                let l2 = e2.length();
                let l3 = e3.length();

                let min_edge = l0.min(l1).min(l2).min(l3);
                let max_edge = l0.max(l1).max(l2).max(l3);

                if min_edge > 4.0 && max_edge > 10.0 {
                    let u0 = e0 / l0;
                    let u1 = e1 / l1;
                    let u2 = e2 / l2;
                    let u3 = e3 / l3;

                    let parallel_02 = u0.dot(u2).abs();
                    let parallel_13 = u1.dot(u3).abs();

                    let len_ratio_02 = l0.min(l2) / l0.max(l2).max(1e-3);
                    let len_ratio_13 = l1.min(l3) / l1.max(l3).max(1e-3);

                    let c01 = u0.dot(u1).abs();
                    let c12 = u1.dot(u2).abs();
                    let c23 = u2.dot(u3).abs();
                    let c30 = u3.dot(u0).abs();

                    let avg_adj_cos = (c01 + c12 + c23 + c30) * 0.25;
                    let max_adj_cos = c01.max(c12).max(c23).max(c30);

                    let opposite_parallel = parallel_02 >= 0.74 && parallel_13 >= 0.74;
                    let opposite_len_consistent = len_ratio_02 >= 0.45 && len_ratio_13 >= 0.45;

                    // 1. Cek Persegi Panjang (Rectangle): Sudut siku-siku (~90 derajat, cos <= 0.20)
                    if opposite_parallel
                        && opposite_len_consistent
                        && avg_adj_cos <= 0.20
                        && max_adj_cos <= 0.35
                        && fill_ratio >= 0.58
                    {
                        return Some(RecognizedShape {
                            kind: ShapeKind::Rectangle,
                            x: min_x,
                            y: min_y,
                            width,
                            height,
                        });
                    }

                    // 2. Cek Jajar Genjang (Parallelogram): Sisi berlawanan sejajar dan condong nyata (> 8 derajat, avg_cos > 0.14)
                    if opposite_parallel
                        && opposite_len_consistent
                        && (0.14..=0.94).contains(&avg_adj_cos)
                        && fill_ratio >= 0.35
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

                    // 3. Fallback Rectangle murni jika sudut mendekati siku-siku dan rasio luas tinggi
                    if avg_adj_cos <= 0.22 && fill_ratio >= 0.65 {
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
        }
    }

    None
}

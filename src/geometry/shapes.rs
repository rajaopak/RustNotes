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
    if close_dist / diagonal <= 0.35 {
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
        if avg_ellipse_err < 0.14 && max_ellipse_err < 0.28 {
            return Some(RecognizedShape {
                kind: ShapeKind::Ellipse,
                x: min_x,
                y: min_y,
                width,
                height,
            });
        }

        // b. Uji Poligon
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

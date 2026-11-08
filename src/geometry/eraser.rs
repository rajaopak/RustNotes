use crate::model::StrokePoint;
use eframe::egui::Pos2;

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

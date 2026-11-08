use crate::model::StrokePoint;
use eframe::egui::{Color32, Mesh, Pos2, Vec2};

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
    color: Color32,
    page_to_screen: impl Fn(Pos2) -> Pos2,
) -> Mesh {
    tessellate_stroke_ex(points, base_width, color, false, page_to_screen)
}

/// Versi lanjutan tessellator dengan dukungan parameter `is_highlighter`
/// untuk memastikan ketebalan pita seragam dan bebas tumpang tindih (overlap-free).
pub fn tessellate_stroke_ex(
    points: &[StrokePoint],
    base_width: f32,
    color: Color32,
    is_highlighter: bool,
    page_to_screen: impl Fn(Pos2) -> Pos2,
) -> Mesh {
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

    // Filter titik-titik yang terlalu rapat untuk mencegah pita melipat diri (ribbon folding overlap)
    let min_dist_sq = if is_highlighter {
        // Highlighters memiliki pita yang lebar (width ~20-50). Titik yang terlalu rapat
        // (< 0.45 * radius) menyebabkan tepi dalam tikungan melipat ke belakang dan melipatgandakan alpha.
        let r = (base_width / 2.0).max(1.0);
        (r * 0.45).powi(2).max(12.0)
    } else {
        2.0
    };

    let mut filtered: Vec<StrokePoint> = Vec::with_capacity(points.len());
    for pt in points {
        if let Some(last) = filtered.last() {
            let dx = pt.x - last.x;
            let dy = pt.y - last.y;
            if (dx * dx + dy * dy) < min_dist_sq {
                continue;
            }
        }
        filtered.push(pt.clone());
    }
    if let Some(last) = points.last() {
        if let Some(prev) = filtered.last() {
            let dx = last.x - prev.x;
            let dy = last.y - prev.y;
            if (dx * dx + dy * dy) >= min_dist_sq {
                filtered.push(last.clone());
            } else if filtered.len() > 1 {
                // Update titik terakhir agar akurat tanpa menciptakan segmen mikro berjarak 0
                *filtered.last_mut().unwrap() = last.clone();
            }
        } else {
            filtered.push(last.clone());
        }
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

    // Start semicircle cap (hanya untuk pen/pencil: menghadap ke arah luar belakang badan ribbon.
    // Untuk highlighter, ujung rata (chisel cut) bebas overlap agar ketebalan warna 100% seragam tanpa bintik gelap).
    if !is_highlighter {
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
    }

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

    // End semicircle cap (hanya untuk pen/pencil: menghadap ke arah luar depan badan ribbon)
    if !is_highlighter {
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
    }

    mesh
}

fn add_semicircle_cap(
    mesh: &mut Mesh,
    center: Pos2,
    radius: f32,
    outward_dir: Vec2,
    color: Color32,
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
        let pos = center + Vec2::new(angle.cos(), angle.sin()) * radius;
        mesh.colored_vertex(pos, color);
    }

    for i in 0..segments as u32 {
        mesh.add_triangle(center_idx, start_idx + i, start_idx + i + 1);
    }
}

fn add_round_cap(mesh: &mut Mesh, center: Pos2, radius: f32, color: Color32, segments: usize) {
    let center_idx = mesh.vertices.len() as u32;
    mesh.colored_vertex(center, color);

    let start_idx = mesh.vertices.len() as u32;
    for i in 0..=segments {
        let angle = (i as f32 / segments as f32) * std::f32::consts::TAU;
        let pos = center + Vec2::new(angle.cos(), angle.sin()) * radius;
        mesh.colored_vertex(pos, color);
    }

    for i in 0..segments as u32 {
        mesh.add_triangle(center_idx, start_idx + i, start_idx + i + 1);
    }
}

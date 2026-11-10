use super::state::CanvasState;
use eframe::egui::{Color32, Mesh, Painter, Pos2, Vec2};

pub fn smooth_laser_points(pts: &[Pos2]) -> Vec<Pos2> {
    if pts.len() <= 2 {
        return pts.to_vec();
    }
    let mut smoothed = Vec::with_capacity(pts.len() * 4);
    smoothed.push(pts[0]);
    for i in 0..(pts.len() - 1) {
        let p0 = if i == 0 {
            pts[0]
        } else {
            Pos2::new(
                (pts[i - 1].x + pts[i].x) / 2.0,
                (pts[i - 1].y + pts[i].y) / 2.0,
            )
        };
        let p1 = pts[i];
        let p2 = if i + 2 >= pts.len() {
            pts[pts.len() - 1]
        } else {
            Pos2::new(
                (pts[i].x + pts[i + 1].x) / 2.0,
                (pts[i].y + pts[i + 1].y) / 2.0,
            )
        };
        for step in 1..=4 {
            let t = step as f32 / 4.0;
            let one_minus_t = 1.0 - t;
            let x = one_minus_t * one_minus_t * p0.x + 2.0 * one_minus_t * t * p1.x + t * t * p2.x;
            let y = one_minus_t * one_minus_t * p0.y + 2.0 * one_minus_t * t * p1.y + t * t * p2.y;
            smoothed.push(Pos2::new(x, y));
        }
    }
    if let Some(&last) = pts.last()
        && let Some(res_last) = smoothed.last_mut()
    {
        *res_last = last;
    }
    smoothed
}

/// Menghasilkan mesh pita segitiga (triangle strip) kontinu untuk layer laser.
/// Mengeliminasi artifak segmen/kotak berulang (ladder rung) yang muncul jika memakai PathShape polyline.
pub fn tessellate_laser_layer(points: &[Pos2], width: f32, color: Color32) -> Mesh {
    let mut mesh = Mesh::default();
    if points.is_empty() || width <= 0.0 {
        return mesh;
    }

    let r = width / 2.0;

    // Filter titik-titik yang terlalu berhimpit
    let mut filtered: Vec<Pos2> = Vec::with_capacity(points.len());
    for &p in points {
        if let Some(&last) = filtered.last()
            && (p - last).length_sq() < 1.0
        {
            continue;
        }
        filtered.push(p);
    }
    if let Some(&last) = points.last() {
        if filtered
            .last()
            .is_none_or(|&p| (p - last).length_sq() >= 1.0)
        {
            filtered.push(last);
        } else if filtered.len() > 1 {
            *filtered.last_mut().unwrap() = last;
        }
    }

    if filtered.len() < 2 {
        if let Some(&p) = filtered.first() {
            add_round_cap(&mut mesh, p, r, color, 12);
        }
        return mesh;
    }

    let pts = &filtered;

    // Hitung vektor normal rata-rata per titik
    let mut normals: Vec<Vec2> = Vec::with_capacity(pts.len());
    for i in 0..pts.len() {
        let dir = if i == 0 {
            pts[1] - pts[0]
        } else if i == pts.len() - 1 {
            pts[i] - pts[i - 1]
        } else {
            (pts[i + 1] - pts[i - 1]) / 2.0
        };
        let len = dir.length();
        let normal = if len > 1e-4 {
            Vec2::new(-dir.y, dir.x) / len
        } else {
            Vec2::new(0.0, 1.0)
        };
        normals.push(normal);
    }

    // Round cap di awal garis (menghadap ke arah belakang garis)
    let start_dir = (pts[0] - pts[1]).normalized();
    add_semicircle_cap(&mut mesh, pts[0], r, start_dir, color, 8);

    // Badan pita (ribbon): simpul kiri dan kanan menyambung halus tanpa tumpang tindih
    let base_idx = mesh.vertices.len() as u32;
    for i in 0..pts.len() {
        let p = pts[i];
        let n = normals[i];
        let left = p + n * r;
        let right = p - n * r;

        mesh.colored_vertex(left, color);
        mesh.colored_vertex(right, color);
    }

    for i in 0..(pts.len() - 1) as u32 {
        let l1 = base_idx + i * 2;
        let r1 = l1 + 1;
        let l2 = l1 + 2;
        let r2 = l1 + 3;

        mesh.add_triangle(l1, r1, l2);
        mesh.add_triangle(r1, r2, l2);
    }

    // Round cap di akhir garis (menghadap ke arah depan garis)
    let last_idx = pts.len() - 1;
    let end_dir = (pts[last_idx] - pts[last_idx - 1]).normalized();
    add_semicircle_cap(&mut mesh, pts[last_idx], r, end_dir, color, 8);

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

/// Render Efek Laser Pointer (4-layer Neon Glow Ribbon)
pub fn render_laser_trail(painter: &Painter, state: &CanvasState, laser_alpha: f32) {
    if laser_alpha <= 0.0
        || (state.laser_strokes.is_empty() && state.current_laser_stroke.is_empty())
    {
        return;
    }

    let mut all_strokes_to_draw: Vec<&[Pos2]> = Vec::new();
    for stroke in &state.laser_strokes {
        if stroke.len() > 1 {
            all_strokes_to_draw.push(stroke.as_slice());
        }
    }
    if state.current_laser_stroke.len() > 1 {
        all_strokes_to_draw.push(state.current_laser_stroke.as_slice());
    }

    let outer_col = Color32::from_rgba_unmultiplied(255, 30, 45, (40.0 * laser_alpha) as u8);
    let halo_col = Color32::from_rgba_unmultiplied(255, 15, 35, (115.0 * laser_alpha) as u8);
    let core_col = Color32::from_rgba_unmultiplied(255, 0, 20, (235.0 * laser_alpha) as u8);
    let beam_col = Color32::from_rgba_unmultiplied(255, 255, 255, (250.0 * laser_alpha) as u8);

    for stroke in &all_strokes_to_draw {
        let smoothed = smooth_laser_points(stroke);
        if smoothed.len() > 1 {
            // Layer 1: Outer Red Neon Aura / Ambient Glow
            let glow_mesh = tessellate_laser_layer(&smoothed, 20.0, outer_col);
            painter.add(glow_mesh);

            // Layer 2: Mid Neon Halo
            let halo_mesh = tessellate_laser_layer(&smoothed, 11.0, halo_col);
            painter.add(halo_mesh);

            // Layer 3: Solid Vibrant Red Core
            let core_mesh = tessellate_laser_layer(&smoothed, 5.0, core_col);
            painter.add(core_mesh);

            // Layer 4: Hotspot White Laser Beam Center
            let beam_mesh = tessellate_laser_layer(&smoothed, 2.2, beam_col);
            painter.add(beam_mesh);
        }
    }

    // Ujung kepala laser aktif (jika sedang menggambar)
    if state.laser_is_drawing
        && let Some(last_p) = state.current_laser_stroke.last()
    {
        painter.circle_filled(
            *last_p,
            11.0,
            Color32::from_rgba_unmultiplied(255, 40, 50, (90.0 * laser_alpha) as u8),
        );
        painter.circle_filled(
            *last_p,
            5.5,
            Color32::from_rgba_unmultiplied(255, 10, 30, (230.0 * laser_alpha) as u8),
        );
        painter.circle_filled(
            *last_p,
            2.8,
            Color32::from_rgba_unmultiplied(255, 255, 255, (255.0 * laser_alpha) as u8),
        );
    }
}

use super::state::CanvasState;
use crate::model::LaserColorMode;
use eframe::egui::{Color32, Mesh, Painter, Pos2, Vec2};

const RAINBOW_WAVELENGTH_PX: f32 = 240.0;
const RAINBOW_SPEED_DEG_PER_SEC: f32 = 160.0;

/// Mengonversi HSV (`hue_deg` dalam derajat, `sat` & `val` dalam `0.0..=1.0`) menjadi RGB `(u8, u8, u8)`.
pub fn hsv_to_rgb(hue_deg: f32, sat: f32, val: f32) -> (u8, u8, u8) {
    let h = hue_deg.rem_euclid(360.0);
    let s = sat.clamp(0.0, 1.0);
    let v = val.clamp(0.0, 1.0);
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;
    let (r1, g1, b1) = match h {
        h if h < 60.0 => (c, x, 0.0),
        h if h < 120.0 => (x, c, 0.0),
        h if h < 180.0 => (0.0, c, x),
        h if h < 240.0 => (0.0, x, c),
        h if h < 300.0 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    (
        ((r1 + m) * 255.0).round() as u8,
        ((g1 + m) * 255.0).round() as u8,
        ((b1 + m) * 255.0).round() as u8,
    )
}

/// Menghitung warna pelangi neon sepanjang panjang busur goresan dan waktu animasi.
pub fn rainbow_laser_color(arc_length_px: f32, time_secs: f32, alpha: u8) -> Color32 {
    let hue =
        (arc_length_px / RAINBOW_WAVELENGTH_PX) * 360.0 - time_secs * RAINBOW_SPEED_DEG_PER_SEC;
    let (r, g, b) = hsv_to_rgb(hue, 0.95, 1.0);
    Color32::from_rgba_unmultiplied(r, g, b, alpha)
}

pub fn smooth_laser_points(pts: &[Pos2]) -> Vec<Pos2> {
    if pts.len() <= 2 {
        return pts.to_vec();
    }
    let mut relaxed = pts.to_vec();
    if relaxed.len() >= 5 {
        for _pass in 0..2 {
            let snap = relaxed.clone();
            for i in 1..(relaxed.len() - 1) {
                let len_in = (snap[i] - snap[i - 1]).length();
                let len_out = (snap[i + 1] - snap[i]).length();
                if len_in.min(len_out) < 4.0 && len_in.max(len_out) < 10.0 {
                    relaxed[i] = Pos2::new(
                        0.25 * snap[i - 1].x + 0.50 * snap[i].x + 0.25 * snap[i + 1].x,
                        0.25 * snap[i - 1].y + 0.50 * snap[i].y + 0.25 * snap[i + 1].y,
                    );
                }
            }
        }
    }

    let mut smoothed = Vec::with_capacity(relaxed.len() * 6);
    smoothed.push(relaxed[0]);
    for i in 0..(relaxed.len() - 1) {
        let p0 = if i == 0 {
            relaxed[0]
        } else {
            Pos2::new(
                (relaxed[i - 1].x + relaxed[i].x) / 2.0,
                (relaxed[i - 1].y + relaxed[i].y) / 2.0,
            )
        };
        let p1 = relaxed[i];
        let p2 = if i + 2 >= relaxed.len() {
            relaxed[relaxed.len() - 1]
        } else {
            Pos2::new(
                (relaxed[i].x + relaxed[i + 1].x) / 2.0,
                (relaxed[i].y + relaxed[i + 1].y) / 2.0,
            )
        };
        let chord_mid = Pos2::new((p0.x + p2.x) * 0.5, (p0.y + p2.y) * 0.5);
        let sagitta = (p1 - chord_mid).length();
        let steps = (4 + (sagitta * 0.8).round() as usize).clamp(4, 10);
        for step in 1..=steps {
            let t = step as f32 / steps as f32;
            let one_minus_t = 1.0 - t;
            let x = one_minus_t * one_minus_t * p0.x + 2.0 * one_minus_t * t * p1.x + t * t * p2.x;
            let y = one_minus_t * one_minus_t * p0.y + 2.0 * one_minus_t * t * p1.y + t * t * p2.y;
            smoothed.push(Pos2::new(x, y));
        }
    }
    if let Some(&last) = relaxed.last()
        && let Some(res_last) = smoothed.last_mut()
    {
        *res_last = last;
    }
    smoothed
}

/// Menghasilkan mesh pita segitiga (triangle strip) kontinu untuk layer laser dengan warna tetap.
pub fn tessellate_laser_layer(points: &[Pos2], width: f32, color: Color32) -> Mesh {
    tessellate_laser_layer_colored(points, width, |_| color)
}

/// Menghasilkan mesh pita segitiga (triangle strip) kontinu bebas pinch/spike untuk layer laser dengan warna per panjang busur.
pub fn tessellate_laser_layer_colored<F>(points: &[Pos2], width: f32, color_at: F) -> Mesh
where
    F: Fn(f32) -> Color32,
{
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
            add_round_cap(&mut mesh, p, r, color_at(0.0), 18);
        }
        return mesh;
    }

    let pts = &filtered;
    let m = pts.len();

    let mut arc_lengths: Vec<f32> = Vec::with_capacity(m);
    arc_lengths.push(0.0);
    for i in 1..m {
        let prev = arc_lengths[i - 1];
        arc_lengths.push(prev + (pts[i] - pts[i - 1]).length());
    }

    let mut seg_dirs: Vec<Vec2> = Vec::with_capacity(m - 1);
    let mut seg_norms: Vec<Vec2> = Vec::with_capacity(m - 1);
    for i in 0..(m - 1) {
        let d = pts[i + 1] - pts[i];
        let len = d.length();
        let dir = if len > 1e-4 {
            d / len
        } else {
            Vec2::new(1.0, 0.0)
        };
        seg_dirs.push(dir);
        seg_norms.push(Vec2::new(-dir.y, dir.x));
    }

    // Round cap di awal garis (menghadap ke arah belakang garis)
    add_semicircle_cap(&mut mesh, pts[0], r, -seg_dirs[0], color_at(0.0), 14);

    let c0 = color_at(arc_lengths[0]);
    let mut prev_left = pts[0] + seg_norms[0] * r;
    let mut prev_right = pts[0] - seg_norms[0] * r;
    let mut curr_l_idx = mesh.vertices.len() as u32;
    mesh.colored_vertex(prev_left, c0);
    let mut curr_r_idx = mesh.vertices.len() as u32;
    mesh.colored_vertex(prev_right, c0);

    // Bangun pita bebas pinch & spike dengan bisector aman serta Round Join melingkar
    for i in 1..(m - 1) {
        let p = pts[i];
        let col = color_at(arc_lengths[i]);
        let t_in = seg_dirs[i - 1];
        let n_in = seg_norms[i - 1];
        let t_out = seg_dirs[i];
        let n_out = seg_norms[i];

        let cos_turn = t_in.dot(t_out).clamp(-1.0, 1.0);
        let cross = t_in.x * t_out.y - t_in.y * t_out.x;

        if cos_turn >= 0.94 {
            let sum_n = n_in + n_out;
            let bisect = if sum_n.length_sq() > 1e-5 {
                sum_n.normalized()
            } else {
                n_out
            };
            let miter_scale = (1.0 / bisect.dot(n_in).abs().max(0.94)).clamp(1.0, 1.04);
            let eff_r = r * miter_scale;
            let left = p + bisect * eff_r;
            let right = p - bisect * eff_r;
            let l_proj = (left - prev_left).dot(t_in);
            let r_proj = (right - prev_right).dot(t_in);
            if l_proj >= 0.0 && r_proj >= 0.0 {
                let next_l_idx = mesh.vertices.len() as u32;
                mesh.colored_vertex(left, col);
                let next_r_idx = mesh.vertices.len() as u32;
                mesh.colored_vertex(right, col);

                mesh.add_triangle(curr_l_idx, curr_r_idx, next_l_idx);
                mesh.add_triangle(curr_r_idx, next_r_idx, next_l_idx);

                curr_l_idx = next_l_idx;
                curr_r_idx = next_r_idx;
                prev_left = left;
                prev_right = right;
                continue;
            }
        }

        let in_left = p + n_in * r;
        let in_right = p - n_in * r;
        let in_l_idx = mesh.vertices.len() as u32;
        mesh.colored_vertex(in_left, col);
        let in_r_idx = mesh.vertices.len() as u32;
        mesh.colored_vertex(in_right, col);

        mesh.add_triangle(curr_l_idx, curr_r_idx, in_l_idx);
        mesh.add_triangle(curr_r_idx, in_r_idx, in_l_idx);

        let out_left = p + n_out * r;
        let out_right = p - n_out * r;
        let out_l_idx = mesh.vertices.len() as u32;
        mesh.colored_vertex(out_left, col);
        let out_r_idx = mesh.vertices.len() as u32;
        mesh.colored_vertex(out_right, col);

        let center_idx = mesh.vertices.len() as u32;
        mesh.colored_vertex(p, col);

        // Tutup sisi cekung tanpa tumpang tindih alpha pada lapisan neon semi-transparan
        if cross >= 0.0 {
            mesh.add_triangle(center_idx, in_l_idx, out_l_idx);
        } else {
            mesh.add_triangle(center_idx, in_r_idx, out_r_idx);
        }

        let (start_norm, end_norm) = if cross >= 0.0 {
            (-n_in, -n_out)
        } else {
            (n_in, n_out)
        };
        add_round_join_fan_colored(
            &mut mesh, center_idx, p, r, start_norm, end_norm, cross, col,
        );

        curr_l_idx = out_l_idx;
        curr_r_idx = out_r_idx;
        prev_left = out_left;
        prev_right = out_right;
    }

    // Titik akhir pita
    let last_idx = m - 1;
    let end_arc = *arc_lengths.last().unwrap_or(&0.0);
    let c_end = color_at(end_arc);
    let t_last = seg_dirs[last_idx - 1];
    let n_last = seg_norms[last_idx - 1];

    let end_l_idx = mesh.vertices.len() as u32;
    mesh.colored_vertex(pts[last_idx] + n_last * r, c_end);
    let end_r_idx = mesh.vertices.len() as u32;
    mesh.colored_vertex(pts[last_idx] - n_last * r, c_end);

    mesh.add_triangle(curr_l_idx, curr_r_idx, end_l_idx);
    mesh.add_triangle(curr_r_idx, end_r_idx, end_l_idx);

    // Round cap di akhir garis (menghadap ke arah depan garis)
    add_semicircle_cap(&mut mesh, pts[last_idx], r, t_last, c_end, 14);

    mesh
}

#[allow(clippy::too_many_arguments)]
fn add_round_join_fan_colored(
    mesh: &mut Mesh,
    center_idx: u32,
    center: Pos2,
    radius: f32,
    start_norm: Vec2,
    end_norm: Vec2,
    cross: f32,
    color: Color32,
) {
    if radius <= 1e-4 {
        return;
    }
    let a0 = start_norm.y.atan2(start_norm.x);
    let a1 = end_norm.y.atan2(end_norm.x);
    let mut delta = a1 - a0;
    while delta <= -std::f32::consts::PI {
        delta += std::f32::consts::TAU;
    }
    while delta > std::f32::consts::PI {
        delta -= std::f32::consts::TAU;
    }
    if delta.abs() > std::f32::consts::PI - 0.05 {
        delta = if cross >= 0.0 {
            std::f32::consts::PI
        } else {
            -std::f32::consts::PI
        };
    }

    let steps = ((delta.abs() / 0.22).ceil() as usize).clamp(2, 14);
    let start_idx = mesh.vertices.len() as u32;
    for s in 0..=steps {
        let t = s as f32 / steps as f32;
        let angle = a0 + delta * t;
        let pos = center + Vec2::new(angle.cos(), angle.sin()) * radius;
        mesh.colored_vertex(pos, color);
    }
    for s in 0..steps as u32 {
        mesh.add_triangle(center_idx, start_idx + s, start_idx + s + 1);
    }
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

/// Render Efek Laser Pointer (4-layer Neon Glow Ribbon: mode Solid atau Rainbow)
pub fn render_laser_trail(
    painter: &Painter,
    state: &CanvasState,
    laser_alpha: f32,
    time_secs: f32,
) {
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

    let outer_a = (44.0 * laser_alpha) as u8;
    let halo_a = (120.0 * laser_alpha) as u8;
    let core_a = (238.0 * laser_alpha) as u8;
    let beam_col = Color32::from_rgba_unmultiplied(255, 255, 255, (250.0 * laser_alpha) as u8);

    let (solid_r, solid_g, solid_b) = (
        state.laser_color.r(),
        state.laser_color.g(),
        state.laser_color.b(),
    );
    let outer_solid = Color32::from_rgba_unmultiplied(solid_r, solid_g, solid_b, outer_a);
    let halo_solid = Color32::from_rgba_unmultiplied(solid_r, solid_g, solid_b, halo_a);
    let core_solid = Color32::from_rgba_unmultiplied(solid_r, solid_g, solid_b, core_a);

    for stroke in &all_strokes_to_draw {
        let smoothed = smooth_laser_points(stroke);
        if smoothed.len() > 1 {
            match state.laser_color_mode {
                LaserColorMode::Solid => {
                    // Layer 1: Outer Neon Aura / Ambient Glow
                    painter.add(tessellate_laser_layer(&smoothed, 20.0, outer_solid));
                    // Layer 2: Mid Neon Halo
                    painter.add(tessellate_laser_layer(&smoothed, 11.0, halo_solid));
                    // Layer 3: Solid Vibrant Core
                    painter.add(tessellate_laser_layer(&smoothed, 5.0, core_solid));
                }
                LaserColorMode::Rainbow => {
                    // Layer 1..3: Rainbow Spectrum Aura, Halo & Core
                    painter.add(tessellate_laser_layer_colored(&smoothed, 20.0, |arc| {
                        rainbow_laser_color(arc, time_secs, outer_a)
                    }));
                    painter.add(tessellate_laser_layer_colored(&smoothed, 11.0, |arc| {
                        rainbow_laser_color(arc, time_secs, halo_a)
                    }));
                    painter.add(tessellate_laser_layer_colored(&smoothed, 5.0, |arc| {
                        rainbow_laser_color(arc, time_secs, core_a)
                    }));
                }
            }

            // Layer 4: Hotspot White Laser Beam Center
            painter.add(tessellate_laser_layer(&smoothed, 2.2, beam_col));
        }
    }

    // Ujung kepala laser aktif (jika sedang menggambar)
    if state.laser_is_drawing
        && let Some(last_p) = state.current_laser_stroke.last()
    {
        let (hr, hg, hb) = match state.laser_color_mode {
            LaserColorMode::Solid => (solid_r, solid_g, solid_b),
            LaserColorMode::Rainbow => {
                let total_arc: f32 = state
                    .current_laser_stroke
                    .windows(2)
                    .map(|w| (w[1] - w[0]).length())
                    .sum();
                let c = rainbow_laser_color(total_arc, time_secs, 255);
                (c.r(), c.g(), c.b())
            }
        };
        painter.circle_filled(
            *last_p,
            11.0,
            Color32::from_rgba_unmultiplied(hr, hg, hb, (90.0 * laser_alpha) as u8),
        );
        painter.circle_filled(
            *last_p,
            5.5,
            Color32::from_rgba_unmultiplied(hr, hg, hb, (230.0 * laser_alpha) as u8),
        );
        painter.circle_filled(
            *last_p,
            2.8,
            Color32::from_rgba_unmultiplied(255, 255, 255, (255.0 * laser_alpha) as u8),
        );
    }
}

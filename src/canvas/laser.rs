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

/// Profil gradien radial Gaussian 7-stop untuk pendaran laser neon bebas piksel:
/// `(radius_px, white_mix, alpha_255)`.
const NEON_PROFILE: [(f32, f32, f32); 7] = [
    (0.0, 1.0, 250.0),
    (1.05, 1.0, 250.0),
    (2.0, 0.28, 245.0),
    (3.2, 0.0, 238.0),
    (5.6, 0.0, 115.0),
    (9.2, 0.0, 36.0),
    (14.0, 0.0, 0.0),
];

#[inline]
fn neon_stop_color(rgb: (u8, u8, u8), white_mix: f32, alpha_255: f32, laser_alpha: f32) -> Color32 {
    let a = (alpha_255 * laser_alpha).round().clamp(0.0, 255.0) as u8;
    if a == 0 {
        return Color32::TRANSPARENT;
    }
    let m = white_mix.clamp(0.0, 1.0);
    let r = (rgb.0 as f32 + (255.0 - rgb.0 as f32) * m)
        .round()
        .clamp(0.0, 255.0) as u8;
    let g = (rgb.1 as f32 + (255.0 - rgb.1 as f32) * m)
        .round()
        .clamp(0.0, 255.0) as u8;
    let b = (rgb.2 as f32 + (255.0 - rgb.2 as f32) * m)
        .round()
        .clamp(0.0, 255.0) as u8;
    Color32::from_rgba_unmultiplied(r, g, b, a)
}

#[inline]
fn push_strip_quad(mesh: &mut Mesh, i0: u32, o0: u32, i1: u32, o1: u32) {
    mesh.add_triangle(i0, o0, i1);
    mesh.add_triangle(o0, o1, i1);
}

#[derive(Clone, Copy)]
struct RadialBand {
    r_in: f32,
    r_out: f32,
    c_in: Color32,
    c_out: Color32,
}

fn add_ring_semicircle(
    mesh: &mut Mesh,
    center: Pos2,
    band: RadialBand,
    outward_dir: Vec2,
    segments: usize,
) {
    if outward_dir.length_sq() < 1e-6 || band.r_out <= 1e-4 {
        return;
    }
    let base_angle = outward_dir.y.atan2(outward_dir.x);
    if band.r_in <= 1e-4 {
        let center_idx = mesh.vertices.len() as u32;
        mesh.colored_vertex(center, band.c_in);
        let start_idx = mesh.vertices.len() as u32;
        for i in 0..=segments {
            let off =
                -std::f32::consts::FRAC_PI_2 + (i as f32 / segments as f32) * std::f32::consts::PI;
            let a = base_angle + off;
            mesh.colored_vertex(
                center + Vec2::new(a.cos(), a.sin()) * band.r_out,
                band.c_out,
            );
        }
        for i in 0..segments as u32 {
            mesh.add_triangle(center_idx, start_idx + i, start_idx + i + 1);
        }
        return;
    }

    let in_start = mesh.vertices.len() as u32;
    for i in 0..=segments {
        let off =
            -std::f32::consts::FRAC_PI_2 + (i as f32 / segments as f32) * std::f32::consts::PI;
        let a = base_angle + off;
        let dir = Vec2::new(a.cos(), a.sin());
        mesh.colored_vertex(center + dir * band.r_in, band.c_in);
    }
    let out_start = mesh.vertices.len() as u32;
    for i in 0..=segments {
        let off =
            -std::f32::consts::FRAC_PI_2 + (i as f32 / segments as f32) * std::f32::consts::PI;
        let a = base_angle + off;
        let dir = Vec2::new(a.cos(), a.sin());
        mesh.colored_vertex(center + dir * band.r_out, band.c_out);
    }
    for i in 0..segments as u32 {
        push_strip_quad(
            mesh,
            in_start + i,
            out_start + i,
            in_start + i + 1,
            out_start + i + 1,
        );
    }
}

fn add_ring_join_arc(
    mesh: &mut Mesh,
    center: Pos2,
    band: RadialBand,
    start_norm: Vec2,
    end_norm: Vec2,
    cross: f32,
) {
    if band.r_out <= 1e-4 {
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

    let steps = ((delta.abs() / 0.20).ceil() as usize).clamp(2, 16);
    if band.r_in <= 1e-4 {
        let center_idx = mesh.vertices.len() as u32;
        mesh.colored_vertex(center, band.c_in);
        let out_start = mesh.vertices.len() as u32;
        for s in 0..=steps {
            let a = a0 + delta * (s as f32 / steps as f32);
            mesh.colored_vertex(
                center + Vec2::new(a.cos(), a.sin()) * band.r_out,
                band.c_out,
            );
        }
        for s in 0..steps as u32 {
            mesh.add_triangle(center_idx, out_start + s, out_start + s + 1);
        }
        return;
    }

    let in_start = mesh.vertices.len() as u32;
    for s in 0..=steps {
        let a = a0 + delta * (s as f32 / steps as f32);
        mesh.colored_vertex(center + Vec2::new(a.cos(), a.sin()) * band.r_in, band.c_in);
    }
    let out_start = mesh.vertices.len() as u32;
    for s in 0..=steps {
        let a = a0 + delta * (s as f32 / steps as f32);
        mesh.colored_vertex(
            center + Vec2::new(a.cos(), a.sin()) * band.r_out,
            band.c_out,
        );
    }
    for s in 0..steps as u32 {
        push_strip_quad(
            mesh,
            in_start + s,
            out_start + s,
            in_start + s + 1,
            out_start + s + 1,
        );
    }
}

/// Menambahkan cakram pendaran radial Gaussian mulus pada satu titik (dipakai kepala laser aktif & tap).
pub fn add_radial_neon_disk(
    mesh: &mut Mesh,
    center: Pos2,
    width_scale: f32,
    laser_alpha: f32,
    rgb: (u8, u8, u8),
    segments: usize,
) {
    let s = width_scale.max(0.2);
    for band in (0..NEON_PROFILE.len() - 1).rev() {
        let (d0, mix0, a0) = NEON_PROFILE[band];
        let (d1, mix1, a1) = NEON_PROFILE[band + 1];
        let r_in = d0 * s;
        let r_out = d1 * s;
        let c_in = neon_stop_color(rgb, mix0, a0, laser_alpha);
        let c_out = neon_stop_color(rgb, mix1, a1, laser_alpha);

        if r_in <= 1e-4 {
            let center_idx = mesh.vertices.len() as u32;
            mesh.colored_vertex(center, c_in);
            let out_start = mesh.vertices.len() as u32;
            for i in 0..=segments {
                let ang = (i as f32 / segments as f32) * std::f32::consts::TAU;
                mesh.colored_vertex(center + Vec2::new(ang.cos(), ang.sin()) * r_out, c_out);
            }
            for i in 0..segments as u32 {
                mesh.add_triangle(center_idx, out_start + i, out_start + i + 1);
            }
        } else {
            let in_start = mesh.vertices.len() as u32;
            for i in 0..=segments {
                let ang = (i as f32 / segments as f32) * std::f32::consts::TAU;
                mesh.colored_vertex(center + Vec2::new(ang.cos(), ang.sin()) * r_in, c_in);
            }
            let out_start = mesh.vertices.len() as u32;
            for i in 0..=segments {
                let ang = (i as f32 / segments as f32) * std::f32::consts::TAU;
                mesh.colored_vertex(center + Vec2::new(ang.cos(), ang.sin()) * r_out, c_out);
            }
            for i in 0..segments as u32 {
                push_strip_quad(
                    mesh,
                    in_start + i,
                    out_start + i,
                    in_start + i + 1,
                    out_start + i + 1,
                );
            }
        }
    }
}

/// Menghasilkan satu mesh neon Gaussian kontinu multi-stop tanpa batas piksel (mendukung mode Solid & Rainbow).
pub fn tessellate_laser_neon_beam<F>(
    points: &[Pos2],
    width_scale: f32,
    laser_alpha: f32,
    color_at: F,
) -> Mesh
where
    F: Fn(f32) -> (u8, u8, u8),
{
    let mut mesh = Mesh::default();
    if points.is_empty() || laser_alpha <= 0.0 || width_scale <= 0.0 {
        return mesh;
    }

    let mut filtered: Vec<Pos2> = Vec::with_capacity(points.len());
    for &p in points {
        if let Some(&last) = filtered.last()
            && (p - last).length_sq() < 0.5625
        {
            continue;
        }
        filtered.push(p);
    }
    if let Some(&last) = points.last() {
        if filtered
            .last()
            .is_none_or(|&p| (p - last).length_sq() >= 0.5625)
        {
            filtered.push(last);
        } else if filtered.len() > 1 {
            *filtered.last_mut().unwrap() = last;
        }
    }

    if filtered.len() < 2 {
        if let Some(&p) = filtered.first() {
            add_radial_neon_disk(&mut mesh, p, width_scale, laser_alpha, color_at(0.0), 22);
        }
        return mesh;
    }

    let pts = &filtered;
    let m = pts.len();
    let mut arc_lengths = Vec::with_capacity(m);
    arc_lengths.push(0.0_f32);
    for i in 1..m {
        arc_lengths.push(arc_lengths[i - 1] + (pts[i] - pts[i - 1]).length());
    }

    let mut seg_dirs = Vec::with_capacity(m - 1);
    let mut seg_norms = Vec::with_capacity(m - 1);
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

    let s = width_scale.max(0.2);
    let rgbs: Vec<(u8, u8, u8)> = arc_lengths.iter().map(|&arc| color_at(arc)).collect();

    // Render dari pita terluar (transparan) menuju inti putih terdalam agar inti terang selalu bersih di atas tikungan tajam
    for band in (0..NEON_PROFILE.len() - 1).rev() {
        let (d0, mix0, a0) = NEON_PROFILE[band];
        let (d1, mix1, a1) = NEON_PROFILE[band + 1];
        let r_in = d0 * s;
        let r_out = d1 * s;

        let c_in_0 = neon_stop_color(rgbs[0], mix0, a0, laser_alpha);
        let c_out_0 = neon_stop_color(rgbs[0], mix1, a1, laser_alpha);
        add_ring_semicircle(
            &mut mesh,
            pts[0],
            RadialBand {
                r_in,
                r_out,
                c_in: c_in_0,
                c_out: c_out_0,
            },
            -seg_dirs[0],
            18,
        );

        let n0 = seg_norms[0];
        let mut prev_l_out = pts[0] + n0 * r_out;
        let mut prev_r_out = pts[0] - n0 * r_out;
        let mut cur_li = mesh.vertices.len() as u32;
        mesh.colored_vertex(pts[0] + n0 * r_in, c_in_0);
        let mut cur_lo = mesh.vertices.len() as u32;
        mesh.colored_vertex(prev_l_out, c_out_0);
        let mut cur_ri = mesh.vertices.len() as u32;
        mesh.colored_vertex(pts[0] - n0 * r_in, c_in_0);
        let mut cur_ro = mesh.vertices.len() as u32;
        mesh.colored_vertex(prev_r_out, c_out_0);

        for i in 1..(m - 1) {
            let p = pts[i];
            let c_in = neon_stop_color(rgbs[i], mix0, a0, laser_alpha);
            let c_out = neon_stop_color(rgbs[i], mix1, a1, laser_alpha);
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
                let miter = (1.0 / bisect.dot(n_in).abs().max(0.94)).clamp(1.0, 1.04);
                let l_out = p + bisect * (r_out * miter);
                let r_out_pos = p - bisect * (r_out * miter);
                if (l_out - prev_l_out).dot(t_in) >= 0.0
                    && (r_out_pos - prev_r_out).dot(t_in) >= 0.0
                {
                    let n_li = mesh.vertices.len() as u32;
                    mesh.colored_vertex(p + bisect * (r_in * miter), c_in);
                    let n_lo = mesh.vertices.len() as u32;
                    mesh.colored_vertex(l_out, c_out);
                    let n_ri = mesh.vertices.len() as u32;
                    mesh.colored_vertex(p - bisect * (r_in * miter), c_in);
                    let n_ro = mesh.vertices.len() as u32;
                    mesh.colored_vertex(r_out_pos, c_out);

                    push_strip_quad(&mut mesh, cur_li, cur_lo, n_li, n_lo);
                    push_strip_quad(&mut mesh, cur_ri, cur_ro, n_ri, n_ro);

                    cur_li = n_li;
                    cur_lo = n_lo;
                    cur_ri = n_ri;
                    cur_ro = n_ro;
                    prev_l_out = l_out;
                    prev_r_out = r_out_pos;
                    continue;
                }
            }

            let in_li = mesh.vertices.len() as u32;
            mesh.colored_vertex(p + n_in * r_in, c_in);
            let in_lo = mesh.vertices.len() as u32;
            mesh.colored_vertex(p + n_in * r_out, c_out);
            let in_ri = mesh.vertices.len() as u32;
            mesh.colored_vertex(p - n_in * r_in, c_in);
            let in_ro = mesh.vertices.len() as u32;
            mesh.colored_vertex(p - n_in * r_out, c_out);

            push_strip_quad(&mut mesh, cur_li, cur_lo, in_li, in_lo);
            push_strip_quad(&mut mesh, cur_ri, cur_ro, in_ri, in_ro);

            let (start_norm, end_norm) = if cross >= 0.0 {
                (-n_in, -n_out)
            } else {
                (n_in, n_out)
            };
            add_ring_join_arc(
                &mut mesh,
                p,
                RadialBand {
                    r_in,
                    r_out,
                    c_in,
                    c_out,
                },
                start_norm,
                end_norm,
                cross,
            );

            let out_l_out = p + n_out * r_out;
            let out_r_out = p - n_out * r_out;
            cur_li = mesh.vertices.len() as u32;
            mesh.colored_vertex(p + n_out * r_in, c_in);
            cur_lo = mesh.vertices.len() as u32;
            mesh.colored_vertex(out_l_out, c_out);
            cur_ri = mesh.vertices.len() as u32;
            mesh.colored_vertex(p - n_out * r_in, c_in);
            cur_ro = mesh.vertices.len() as u32;
            mesh.colored_vertex(out_r_out, c_out);
            prev_l_out = out_l_out;
            prev_r_out = out_r_out;
        }

        let last_idx = m - 1;
        let c_in_last = neon_stop_color(rgbs[last_idx], mix0, a0, laser_alpha);
        let c_out_last = neon_stop_color(rgbs[last_idx], mix1, a1, laser_alpha);
        let t_last = seg_dirs[last_idx - 1];
        let n_last = seg_norms[last_idx - 1];

        let end_li = mesh.vertices.len() as u32;
        mesh.colored_vertex(pts[last_idx] + n_last * r_in, c_in_last);
        let end_lo = mesh.vertices.len() as u32;
        mesh.colored_vertex(pts[last_idx] + n_last * r_out, c_out_last);
        let end_ri = mesh.vertices.len() as u32;
        mesh.colored_vertex(pts[last_idx] - n_last * r_in, c_in_last);
        let end_ro = mesh.vertices.len() as u32;
        mesh.colored_vertex(pts[last_idx] - n_last * r_out, c_out_last);

        push_strip_quad(&mut mesh, cur_li, cur_lo, end_li, end_lo);
        push_strip_quad(&mut mesh, cur_ri, cur_ro, end_ri, end_ro);

        add_ring_semicircle(
            &mut mesh,
            pts[last_idx],
            RadialBand {
                r_in,
                r_out,
                c_in: c_in_last,
                c_out: c_out_last,
            },
            t_last,
            18,
        );
    }

    mesh
}

/// Render Efek Laser Pointer (Gaussian Neon Glow Kontinu: mode Solid atau Rainbow)
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

    let solid_rgb = (
        state.laser_color.r(),
        state.laser_color.g(),
        state.laser_color.b(),
    );

    for stroke in &all_strokes_to_draw {
        let smoothed = smooth_laser_points(stroke);
        if smoothed.len() > 1 {
            let mesh = match state.laser_color_mode {
                LaserColorMode::Solid => {
                    tessellate_laser_neon_beam(&smoothed, 1.0, laser_alpha, |_| solid_rgb)
                }
                LaserColorMode::Rainbow => {
                    tessellate_laser_neon_beam(&smoothed, 1.0, laser_alpha, |arc| {
                        let c = rainbow_laser_color(arc, time_secs, 255);
                        (c.r(), c.g(), c.b())
                    })
                }
            };
            painter.add(mesh);
        }
    }

    // Ujung kepala laser aktif dengan gradasi radial halus
    if state.laser_is_drawing
        && let Some(last_p) = state.current_laser_stroke.last()
    {
        let head_rgb = match state.laser_color_mode {
            LaserColorMode::Solid => solid_rgb,
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
        let mut head_mesh = Mesh::default();
        add_radial_neon_disk(&mut head_mesh, *last_p, 1.05, laser_alpha, head_rgb, 24);
        painter.add(head_mesh);
    }
}

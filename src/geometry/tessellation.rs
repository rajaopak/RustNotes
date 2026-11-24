use crate::model::{StrokePoint, StrokeStyle};
use eframe::egui::{Color32, Mesh, Pos2, Vec2};

/// Menjamin nilai pressure selalu berada dalam rentang valid `[0.05, 1.0]`
/// dan menangani nilai tidak valid (`NaN`, `Infinity`, atau negatif) secara aman.
#[inline]
pub fn sanitize_pressure(pressure: f32) -> f32 {
    if !pressure.is_finite() {
        0.5
    } else {
        pressure.clamp(0.05, 1.0)
    }
}

/// Menghaluskan kanal pressure secara adaptif (edge-preserving filter):
/// meredam lonjakan noise 1-sampel tanpa menghilangkan perubahan tekanan yang nyata
/// maupun gradasi kenaikan/penurunan tekanan pena.
pub fn stabilize_stroke_pressures(points: &[StrokePoint]) -> Vec<f32> {
    let n = points.len();
    if n == 0 {
        return Vec::new();
    }
    let raw: Vec<f32> = points
        .iter()
        .map(|p| sanitize_pressure(p.pressure))
        .collect();
    if n <= 2 {
        return raw;
    }

    let mut out = Vec::with_capacity(n);
    out.push(raw[0]);
    for i in 1..(n - 1) {
        let p_prev = raw[i - 1];
        let p_curr = raw[i];
        let p_next = raw[i + 1];
        let d1 = p_curr - p_prev;
        let d2 = p_next - p_curr;

        // Jika terjadi osilasi bolak-balik (spike tunggal) dengan amplitudo kecil/menengah, redam noise
        if d1 * d2 < 0.0 && d1.abs().min(d2.abs()) < 0.25 {
            out.push((0.25 * p_prev + 0.50 * p_curr + 0.25 * p_next).clamp(0.05, 1.0));
        } else if d1.abs() >= 0.20 || d2.abs() >= 0.20 {
            // Pertahankan perubahan pressure tajam yang disengaja
            out.push(p_curr);
        } else {
            // Transisi bertahap yang halus
            out.push((0.15 * p_prev + 0.70 * p_curr + 0.15 * p_next).clamp(0.05, 1.0));
        }
    }
    out.push(raw[n - 1]);
    out
}

/// Menghaluskan titik goresan mentah menggunakan kurva Quadratic Bézier adaptif yang
/// mempertahankan sudut tajam (corner-preserving), bebas overshoot (convex-hull property),
/// mempertahankan goresan pendek, serta menjaga titik awal dan akhir tetap eksak.
pub fn smooth_stroke_points(points: &[StrokePoint], steps_per_segment: usize) -> Vec<StrokePoint> {
    if points.is_empty() {
        return Vec::new();
    }
    if points.len() == 1 {
        let mut pt = points[0].clone();
        pt.pressure = sanitize_pressure(pt.pressure);
        return vec![pt];
    }
    if points.len() == 2 || steps_per_segment <= 1 {
        return points
            .iter()
            .map(|p| StrokePoint {
                x: p.x,
                y: p.y,
                pressure: sanitize_pressure(p.pressure),
            })
            .collect();
    }

    let pressures = stabilize_stroke_pressures(points);

    // Bersihkan titik-titik berurutan yang tepat berhimpit sambil mempertahankan titik awal & akhir
    let mut clean: Vec<StrokePoint> = Vec::with_capacity(points.len());
    for (idx, pt) in points.iter().enumerate() {
        let p = StrokePoint {
            x: pt.x,
            y: pt.y,
            pressure: pressures[idx],
        };
        if let Some(last) = clean.last_mut() {
            let dist_sq = (p.to_pos2() - last.to_pos2()).length_sq();
            if dist_sq <= 1e-8 && idx + 1 < points.len() {
                if (p.pressure - last.pressure).abs() > 1e-3 {
                    last.pressure = (last.pressure + p.pressure) * 0.5;
                }
                continue;
            }
        }
        clean.push(p);
    }

    if clean.len() <= 2 {
        return clean;
    }

    // Hilangkan noise tangga piksel 1px mouse pada segmen mikro sambil menjaga sudut sengaja pada garis panjang.
    if clean.len() >= 5 {
        for _pass in 0..2 {
            let prev_snapshot = clean.clone();
            for i in 1..(clean.len() - 1) {
                let prev = prev_snapshot[i - 1].to_pos2();
                let curr = prev_snapshot[i].to_pos2();
                let next = prev_snapshot[i + 1].to_pos2();
                let len_in = (curr - prev).length();
                let len_out = (next - curr).length();
                if len_in.min(len_out) < 3.0 && len_in.max(len_out) < 7.0 {
                    clean[i].x = 0.25 * prev.x + 0.50 * curr.x + 0.25 * next.x;
                    clean[i].y = 0.25 * prev.y + 0.50 * curr.y + 0.25 * next.y;
                }
            }
        }
    }

    let n = clean.len();
    let base_steps = steps_per_segment.clamp(2, 8);
    let mut result = Vec::with_capacity(n * base_steps);
    result.push(clean[0].clone());

    // Hitung faktor pembulatan sudut (corner rounding factor) pada setiap titik interior.
    // Sudut sangat tajam (cos_turn <= -0.15, belokan > ~98 derajat) dipertahankan eksak (alpha = 0).
    // Sudut sedang dibatasi radius pembulatannya agar segmen lurus panjang tidak terpotong jauh dari sudut.
    const MAX_CORNER_ROUND_DIST: f32 = 8.0;
    let mut entry_pts = vec![clean[0].clone(); n];
    let mut exit_pts = vec![clean[0].clone(); n];
    let mut is_sharp_corner = vec![false; n];

    exit_pts[0] = clean[0].clone();
    entry_pts[n - 1] = clean[n - 1].clone();

    for i in 1..(n - 1) {
        let prev = clean[i - 1].to_pos2();
        let curr = clean[i].to_pos2();
        let next = clean[i + 1].to_pos2();

        let v_in = curr - prev;
        let v_out = next - curr;
        let len_in = v_in.length();
        let len_out = v_out.length();

        if len_in <= 1e-5 || len_out <= 1e-5 {
            entry_pts[i] = clean[i].clone();
            exit_pts[i] = clean[i].clone();
            is_sharp_corner[i] = true;
            continue;
        }

        let cos_turn = (v_in / len_in).dot(v_out / len_out).clamp(-1.0, 1.0);
        if cos_turn <= -0.15 && (len_in.min(len_out) >= 2.5 || n <= 4) {
            // Sudut lancip / balik arah tajam yang disengaja: pertahankan puncak sudut secara eksak
            entry_pts[i] = clean[i].clone();
            exit_pts[i] = clean[i].clone();
            is_sharp_corner[i] = true;
        } else if cos_turn < 0.45 {
            // Sudut belok menengah (63..98 derajat): batasi jangkauan pembulatan di sekitar sudut
            let t_norm = ((cos_turn - (-0.15)) / 0.60).clamp(0.0, 1.0);
            let alpha = 0.08 + 0.42 * (t_norm * t_norm);
            let d_in = (len_in * alpha)
                .min(MAX_CORNER_ROUND_DIST)
                .min(len_in * 0.5);
            let d_out = (len_out * alpha)
                .min(MAX_CORNER_ROUND_DIST)
                .min(len_out * 0.5);
            let t_in = (d_in / len_in).clamp(0.0, 0.5);
            let t_out = (d_out / len_out).clamp(0.0, 0.5);

            entry_pts[i] = StrokePoint {
                x: curr.x - v_in.x * t_in,
                y: curr.y - v_in.y * t_in,
                pressure: clean[i].pressure + (clean[i - 1].pressure - clean[i].pressure) * t_in,
            };
            exit_pts[i] = StrokePoint {
                x: curr.x + v_out.x * t_out,
                y: curr.y + v_out.y * t_out,
                pressure: clean[i].pressure + (clean[i + 1].pressure - clean[i].pressure) * t_out,
            };
        } else {
            // Kurva halus: gunakan midpoint Quadratic Bézier standar (C1 continuous)
            let t_in = if i == 1 { 1.0 } else { 0.5 };
            let t_out = if i + 2 == n { 1.0 } else { 0.5 };
            entry_pts[i] = StrokePoint {
                x: curr.x - v_in.x * t_in,
                y: curr.y - v_in.y * t_in,
                pressure: clean[i].pressure + (clean[i - 1].pressure - clean[i].pressure) * t_in,
            };
            exit_pts[i] = StrokePoint {
                x: curr.x + v_out.x * t_out,
                y: curr.y + v_out.y * t_out,
                pressure: clean[i].pressure + (clean[i + 1].pressure - clean[i].pressure) * t_out,
            };
        }
    }

    for i in 1..(n - 1) {
        let prev_exit = &exit_pts[i - 1];
        let p0 = &entry_pts[i];
        let p1 = &clean[i];
        let p2 = &exit_pts[i];

        // Jika terdapat segmen lurus penghubung antara exit_pts[i-1] dan entry_pts[i], tambahkan titik masuknya
        let gap_sq = (p0.to_pos2() - prev_exit.to_pos2()).length_sq();
        if gap_sq > 1e-5
            && let Some(last_out) = result.last()
            && (p0.to_pos2() - last_out.to_pos2()).length_sq() > 1e-6
        {
            result.push(p0.clone());
        }

        if is_sharp_corner[i] {
            if let Some(last_out) = result.last()
                && (p1.to_pos2() - last_out.to_pos2()).length_sq() > 1e-6
            {
                result.push(p1.clone());
            }
            continue;
        }

        // Hitung kelengkungan lokal (sagitta) untuk menentukan jumlah subdivisi secara adaptif
        let sagitta =
            super::primitives::point_to_segment_dist(p1.to_pos2(), p0.to_pos2(), p2.to_pos2());
        let span_len =
            (p1.to_pos2() - p0.to_pos2()).length() + (p2.to_pos2() - p1.to_pos2()).length();
        let dp = (p0.pressure - p1.pressure)
            .abs()
            .max((p2.pressure - p1.pressure).abs());

        let local_steps = if sagitta < 0.02 && dp < 0.02 && span_len < 2.0 {
            base_steps.min(2)
        } else {
            let extra = (sagitta * 1.1).round() as usize;
            (base_steps + extra).clamp(base_steps, 12)
        };

        for step in 1..=local_steps {
            let t = step as f32 / local_steps as f32;
            let one_minus_t = 1.0 - t;
            // Quadratic Bézier: B(t) = (1-t)^2 * p0 + 2*(1-t)*t * p1 + t^2 * p2
            // Selalu berada di dalam convex hull (p0, p1, p2) sehingga bebas overshoot
            let x = one_minus_t * one_minus_t * p0.x + 2.0 * one_minus_t * t * p1.x + t * t * p2.x;
            let y = one_minus_t * one_minus_t * p0.y + 2.0 * one_minus_t * t * p1.y + t * t * p2.y;
            let pressure = (one_minus_t * one_minus_t * p0.pressure
                + 2.0 * one_minus_t * t * p1.pressure
                + t * t * p2.pressure)
                .clamp(0.05, 1.0);

            result.push(StrokePoint { x, y, pressure });
        }
    }

    // Pastikan titik paling akhir sama persis dengan titik input terakhir
    if let Some(last) = clean.last() {
        if let Some(res_last) = result.last_mut() {
            if (last.to_pos2() - res_last.to_pos2()).length_sq() > 1e-6 {
                result.push(last.clone());
            } else {
                res_last.x = last.x;
                res_last.y = last.y;
                res_last.pressure = last.pressure;
            }
        } else {
            result.push(last.clone());
        }
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
    tessellate_stroke_styled(
        points,
        base_width,
        color,
        false,
        StrokeStyle::Solid,
        page_to_screen,
    )
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
    tessellate_stroke_styled(
        points,
        base_width,
        color,
        is_highlighter,
        StrokeStyle::Solid,
        page_to_screen,
    )
}

/// Menghitung radius goresan berdasarkan `base_width` dan `pressure` secara aman.
#[inline]
pub fn stroke_radius_at_pressure(base_width: f32, pressure: f32, is_highlighter: bool) -> f32 {
    let safe_w = if base_width.is_finite() {
        base_width.max(0.5)
    } else {
        1.0
    };
    if is_highlighter {
        (safe_w / 2.0).max(1.0)
    } else {
        let safe_p = sanitize_pressure(pressure);
        (safe_w * safe_p / 2.0).max(0.5)
    }
}

/// Tessellator komprehensif yang mendukung gaya garis (Solid, Dashed, Dash-Dot, Dotted)
/// dengan komputasi jarak kumulatif (cumulative path distance) agar pola dash konsisten di seluruh kurva.
pub fn tessellate_stroke_styled(
    points: &[StrokePoint],
    base_width: f32,
    color: Color32,
    is_highlighter: bool,
    style: StrokeStyle,
    page_to_screen: impl Fn(Pos2) -> Pos2,
) -> Mesh {
    tessellate_stroke_styled_aa(
        points,
        base_width,
        color,
        is_highlighter,
        style,
        0.0,
        page_to_screen,
    )
}

/// Versi tessellator dengan *per-edge anti-aliasing feathering* (`aa_feather` dalam satuan koordinat target/layar)
/// agar tepi mesh mentah tampil sehalus kurva vektor tanpa efek bergerigi/pixelated.
pub fn tessellate_stroke_styled_aa(
    points: &[StrokePoint],
    base_width: f32,
    color: Color32,
    is_highlighter: bool,
    style: StrokeStyle,
    aa_feather: f32,
    page_to_screen: impl Fn(Pos2) -> Pos2,
) -> Mesh {
    let mut mesh = Mesh::default();
    if points.is_empty() {
        return mesh;
    }
    let aa_feather = if aa_feather.is_finite() {
        aa_feather.max(0.0)
    } else {
        0.0
    };

    // Dukungan goresan 1 titik (Single-point stroke / tap)
    if points.len() == 1 {
        let center = page_to_screen(points[0].to_pos2());
        let r = stroke_radius_at_pressure(base_width, points[0].pressure, is_highlighter);
        let segs = if aa_feather > 1e-4 { 20 } else { 16 };
        add_round_cap_aa(&mut mesh, center, r, color, aa_feather, segs);
        return mesh;
    }

    // Hitung jarak kumulatif di sepanjang kurva titik goresan
    let mut dists = Vec::with_capacity(points.len());
    dists.push(0.0);
    for i in 1..points.len() {
        let seg_len = (points[i].to_pos2() - points[i - 1].to_pos2()).length();
        let safe_len = if seg_len.is_finite() { seg_len } else { 0.0 };
        dists.push(dists[i - 1] + safe_len);
    }
    let total_len = *dists.last().unwrap_or(&0.0);
    if total_len < 1e-4 {
        let max_p = points
            .iter()
            .map(|p| sanitize_pressure(p.pressure))
            .fold(0.05_f32, f32::max);
        let center = page_to_screen(points[0].to_pos2());
        let r = stroke_radius_at_pressure(base_width, max_p, is_highlighter);
        let segs = if aa_feather > 1e-4 { 20 } else { 16 };
        add_round_cap_aa(&mut mesh, center, r, color, aa_feather, segs);
        return mesh;
    }

    let dot_segs = if aa_feather > 1e-4 { 16 } else { 12 };
    match style {
        StrokeStyle::Solid => {
            tessellate_single_ribbon_aa(
                points,
                base_width,
                color,
                is_highlighter,
                aa_feather,
                &page_to_screen,
                &mut mesh,
            );
        }
        StrokeStyle::Dotted => {
            let dot_spacing = (base_width * 2.5).max(7.0);
            let mut s = 0.0;
            while s <= total_len {
                let pt = sample_point_at_dist(points, &dists, s);
                let center = page_to_screen(pt.to_pos2());
                let r = stroke_radius_at_pressure(base_width, pt.pressure, is_highlighter).max(0.8);
                add_round_cap_aa(&mut mesh, center, r, color, aa_feather, dot_segs);
                s += dot_spacing;
            }
        }
        StrokeStyle::Dashed => {
            let dash_len = (base_width * 3.5).max(10.0);
            let gap_len = (base_width * 2.0).max(6.0);
            let cycle = dash_len + gap_len;
            let mut curr = 0.0;
            while curr < total_len {
                let s_start = curr;
                let s_end = (curr + dash_len).min(total_len);
                let sub_pts = slice_stroke_by_dist(points, &dists, s_start, s_end);
                if sub_pts.len() == 1 {
                    let center = page_to_screen(sub_pts[0].to_pos2());
                    let r =
                        stroke_radius_at_pressure(base_width, sub_pts[0].pressure, is_highlighter)
                            .max(1.0);
                    add_round_cap_aa(&mut mesh, center, r, color, aa_feather, dot_segs);
                } else if sub_pts.len() >= 2 {
                    tessellate_single_ribbon_aa(
                        &sub_pts,
                        base_width,
                        color,
                        is_highlighter,
                        aa_feather,
                        &page_to_screen,
                        &mut mesh,
                    );
                }
                curr += cycle;
            }
        }
        StrokeStyle::DashDot => {
            let dash_len = (base_width * 3.5).max(10.0);
            let gap1 = (base_width * 1.8).max(5.0);
            let dot_len = (base_width * 0.8).max(3.0);
            let gap2 = (base_width * 1.8).max(5.0);
            let cycle = dash_len + gap1 + dot_len + gap2;
            let mut curr = 0.0;
            while curr < total_len {
                // Dash
                let s_dash_start = curr;
                let s_dash_end = (curr + dash_len).min(total_len);
                let dash_pts = slice_stroke_by_dist(points, &dists, s_dash_start, s_dash_end);
                if dash_pts.len() == 1 {
                    let center = page_to_screen(dash_pts[0].to_pos2());
                    let r =
                        stroke_radius_at_pressure(base_width, dash_pts[0].pressure, is_highlighter)
                            .max(1.0);
                    add_round_cap_aa(&mut mesh, center, r, color, aa_feather, dot_segs);
                } else if dash_pts.len() >= 2 {
                    tessellate_single_ribbon_aa(
                        &dash_pts,
                        base_width,
                        color,
                        is_highlighter,
                        aa_feather,
                        &page_to_screen,
                        &mut mesh,
                    );
                }

                // Dot
                let s_dot_mid = curr + dash_len + gap1 + dot_len * 0.5;
                if s_dot_mid <= total_len {
                    let dot_pt = sample_point_at_dist(points, &dists, s_dot_mid);
                    let center = page_to_screen(dot_pt.to_pos2());
                    let r = stroke_radius_at_pressure(base_width, dot_pt.pressure, is_highlighter)
                        .max(0.8);
                    add_round_cap_aa(&mut mesh, center, r, color, aa_feather, dot_segs);
                }
                curr += cycle;
            }
        }
    }

    mesh
}

/// Bagian terpisah dari goresan bergaya (`Solid`, `Dashed`, `DashDot`, `Dotted`)
/// dalam ruang koordinat input sehingga tessellator maupun rasterizer menghasilkan pola identik.
#[derive(Clone, Debug, PartialEq)]
pub enum StrokeStylePart {
    Polyline(Vec<StrokePoint>),
    Dot(StrokePoint),
}

/// Memecah rangkaian titik goresan menjadi segmen-segmen garis (`Polyline`) dan titik (`Dot`)
/// sesuai `style` dan `base_width` (dalam satuan ruang koordinat titik input).
pub fn decompose_styled_stroke(
    points: &[StrokePoint],
    base_width: f32,
    style: StrokeStyle,
) -> Vec<StrokeStylePart> {
    if points.is_empty() {
        return Vec::new();
    }
    if points.len() == 1 {
        let mut pt = points[0].clone();
        pt.pressure = sanitize_pressure(pt.pressure);
        return vec![StrokeStylePart::Dot(pt)];
    }

    let mut dists = Vec::with_capacity(points.len());
    dists.push(0.0);
    for i in 1..points.len() {
        let seg_len = (points[i].to_pos2() - points[i - 1].to_pos2()).length();
        let safe_len = if seg_len.is_finite() { seg_len } else { 0.0 };
        dists.push(dists[i - 1] + safe_len);
    }
    let total_len = *dists.last().unwrap_or(&0.0);
    if total_len < 1e-4 {
        let max_p = points
            .iter()
            .map(|p| sanitize_pressure(p.pressure))
            .fold(0.05_f32, f32::max);
        let mut pt = points[0].clone();
        pt.pressure = max_p;
        return vec![StrokeStylePart::Dot(pt)];
    }

    let safe_w = if base_width.is_finite() {
        base_width.max(0.5)
    } else {
        1.0
    };

    let mut parts = Vec::new();
    match style {
        StrokeStyle::Solid => {
            parts.push(StrokeStylePart::Polyline(points.to_vec()));
        }
        StrokeStyle::Dotted => {
            let dot_spacing = (safe_w * 2.5).max(7.0);
            let mut s = 0.0;
            while s <= total_len {
                parts.push(StrokeStylePart::Dot(sample_point_at_dist(
                    points, &dists, s,
                )));
                s += dot_spacing;
            }
        }
        StrokeStyle::Dashed => {
            let dash_len = (safe_w * 3.5).max(10.0);
            let gap_len = (safe_w * 2.0).max(6.0);
            let cycle = dash_len + gap_len;
            let mut curr = 0.0;
            while curr < total_len {
                let s_start = curr;
                let s_end = (curr + dash_len).min(total_len);
                let sub_pts = slice_stroke_by_dist(points, &dists, s_start, s_end);
                if sub_pts.len() == 1 {
                    parts.push(StrokeStylePart::Dot(sub_pts.into_iter().next().unwrap()));
                } else if sub_pts.len() >= 2 {
                    parts.push(StrokeStylePart::Polyline(sub_pts));
                }
                curr += cycle;
            }
        }
        StrokeStyle::DashDot => {
            let dash_len = (safe_w * 3.5).max(10.0);
            let gap1 = (safe_w * 1.8).max(5.0);
            let dot_len = (safe_w * 0.8).max(3.0);
            let gap2 = (safe_w * 1.8).max(5.0);
            let cycle = dash_len + gap1 + dot_len + gap2;
            let mut curr = 0.0;
            while curr < total_len {
                let s_dash_start = curr;
                let s_dash_end = (curr + dash_len).min(total_len);
                let dash_pts = slice_stroke_by_dist(points, &dists, s_dash_start, s_dash_end);
                if dash_pts.len() == 1 {
                    parts.push(StrokeStylePart::Dot(dash_pts.into_iter().next().unwrap()));
                } else if dash_pts.len() >= 2 {
                    parts.push(StrokeStylePart::Polyline(dash_pts));
                }

                let s_dot_mid = curr + dash_len + gap1 + dot_len * 0.5;
                if s_dot_mid <= total_len {
                    parts.push(StrokeStylePart::Dot(sample_point_at_dist(
                        points, &dists, s_dot_mid,
                    )));
                }
                curr += cycle;
            }
        }
    }
    parts
}

fn sample_point_at_dist(points: &[StrokePoint], dists: &[f32], target_d: f32) -> StrokePoint {
    if points.is_empty() {
        return StrokePoint {
            x: 0.0,
            y: 0.0,
            pressure: 1.0,
        };
    }
    if points.len() == 1 || target_d <= 0.0 {
        let mut pt = points[0].clone();
        pt.pressure = sanitize_pressure(pt.pressure);
        return pt;
    }
    let total_len = *dists.last().unwrap_or(&0.0);
    if target_d >= total_len {
        let mut pt = points.last().unwrap().clone();
        pt.pressure = sanitize_pressure(pt.pressure);
        return pt;
    }
    let idx = match dists.binary_search_by(|d| {
        d.partial_cmp(&target_d)
            .unwrap_or(std::cmp::Ordering::Equal)
    }) {
        Ok(i) => i,
        Err(i) => i,
    };
    let i = idx.clamp(1, points.len() - 1);
    let d0 = dists[i - 1];
    let d1 = dists[i];
    let seg_len = d1 - d0;
    let t = if seg_len > 1e-5 {
        ((target_d - d0) / seg_len).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let p0 = &points[i - 1];
    let p1 = &points[i];
    let pr0 = sanitize_pressure(p0.pressure);
    let pr1 = sanitize_pressure(p1.pressure);
    StrokePoint {
        x: p0.x + (p1.x - p0.x) * t,
        y: p0.y + (p1.y - p0.y) * t,
        pressure: pr0 + (pr1 - pr0) * t,
    }
}

fn slice_stroke_by_dist(
    points: &[StrokePoint],
    dists: &[f32],
    s_start: f32,
    s_end: f32,
) -> Vec<StrokePoint> {
    if s_start >= s_end {
        return Vec::new();
    }
    let mut slice = Vec::new();
    slice.push(sample_point_at_dist(points, dists, s_start));
    for i in 0..points.len() {
        if dists[i] > s_start + 1e-3 && dists[i] < s_end - 1e-3 {
            let mut pt = points[i].clone();
            pt.pressure = sanitize_pressure(pt.pressure);
            slice.push(pt);
        }
    }
    slice.push(sample_point_at_dist(points, dists, s_end));
    slice
}

/// Penyaringan adaptif untuk data rendering goresan (derived rendering data).
/// Mempertahankan titik awal, titik akhir, detail goresan pendek, perubahan arah (sudut/belokan),
/// kelengkungan jalur, dan perubahan pressure yang signifikan, sambil menyederhanakan titik
/// kolinear yang berhimpit agar mesh tetap bersih dan bebas lipatan (ribbon folding).
pub fn filter_stroke_points_adaptive(
    points: &[StrokePoint],
    page_base_width: f32,
    is_highlighter: bool,
) -> Vec<StrokePoint> {
    if points.is_empty() {
        return Vec::new();
    }
    if points.len() == 1 {
        let mut pt = points[0].clone();
        pt.pressure = sanitize_pressure(pt.pressure);
        return vec![pt];
    }
    if points.len() == 2 {
        let p0 = StrokePoint {
            x: points[0].x,
            y: points[0].y,
            pressure: sanitize_pressure(points[0].pressure),
        };
        let p1 = StrokePoint {
            x: points[1].x,
            y: points[1].y,
            pressure: sanitize_pressure(points[1].pressure),
        };
        let dist_sq = (p1.to_pos2() - p0.to_pos2()).length_sq();
        if dist_sq <= 1e-8 {
            let mut single = p0;
            single.pressure = single.pressure.max(p1.pressure);
            return vec![single];
        }
        return vec![p0, p1];
    }

    let mut total_len = 0.0_f32;
    for i in 1..points.len() {
        let seg = (points[i].to_pos2() - points[i - 1].to_pos2()).length();
        if seg.is_finite() {
            total_len += seg;
        }
    }

    let eff_w = if page_base_width.is_finite() {
        page_base_width.max(0.5)
    } else {
        1.0
    };
    let min_collinear_dist = if is_highlighter {
        if total_len < eff_w {
            (total_len * 0.20).clamp(0.15, 1.2)
        } else {
            (eff_w * 0.18).clamp(0.5, 2.5)
        }
    } else if total_len < (eff_w * 2.0).max(4.0) {
        0.04
    } else {
        (eff_w * 0.08).clamp(0.08, 0.35)
    };
    let min_collinear_dist_sq = min_collinear_dist * min_collinear_dist;
    let curvature_tol = (eff_w * 0.02).clamp(0.015, 0.10);

    let mut filtered: Vec<StrokePoint> = Vec::with_capacity(points.len());
    filtered.push(StrokePoint {
        x: points[0].x,
        y: points[0].y,
        pressure: sanitize_pressure(points[0].pressure),
    });

    let last_idx = points.len() - 1;
    for i in 1..last_idx {
        let curr = StrokePoint {
            x: points[i].x,
            y: points[i].y,
            pressure: sanitize_pressure(points[i].pressure),
        };
        let prev = filtered.last().unwrap();
        let v_in = curr.to_pos2() - prev.to_pos2();
        let dist_sq = v_in.length_sq();
        let dp = (curr.pressure - prev.pressure).abs();

        // Titik yang tepat berhimpit secara numerik
        if dist_sq <= 1e-6 {
            if !is_highlighter && dp >= 0.01 {
                filtered.last_mut().unwrap().pressure = curr.pressure;
            }
            continue;
        }

        if dist_sq >= min_collinear_dist_sq {
            filtered.push(curr);
            continue;
        }

        // Pertahankan jika terjadi perubahan tekanan (pressure) yang signifikan
        if !is_highlighter && dp >= 0.025 {
            filtered.push(curr);
            continue;
        }

        // Pertahankan jika terdapat perubahan arah (sudut) atau kelengkungan jalur yang penting
        let next = &points[i + 1];
        let v_out = next.to_pos2() - curr.to_pos2();
        let out_len_sq = v_out.length_sq();
        if out_len_sq > 1e-6 {
            let in_dir = v_in / dist_sq.sqrt();
            let out_dir = v_out / out_len_sq.sqrt();
            let cos_turn = in_dir.dot(out_dir);
            let turn_threshold = if is_highlighter { 0.75 } else { 0.965 };
            if cos_turn < turn_threshold {
                filtered.push(curr);
                continue;
            }

            if !is_highlighter {
                let chord_dev = super::primitives::point_to_segment_dist(
                    curr.to_pos2(),
                    prev.to_pos2(),
                    next.to_pos2(),
                );
                if chord_dev >= curvature_tol {
                    filtered.push(curr);
                    continue;
                }
            }
        }
    }

    // Selalu pertahankan titik akhir secara akurat
    if let Some(raw_last) = points.last() {
        let last = StrokePoint {
            x: raw_last.x,
            y: raw_last.y,
            pressure: sanitize_pressure(raw_last.pressure),
        };
        if let Some(prev) = filtered.last() {
            let dist_sq = (last.to_pos2() - prev.to_pos2()).length_sq();
            if dist_sq > 1e-6 {
                filtered.push(last);
            } else if filtered.len() > 1 {
                *filtered.last_mut().unwrap() = last;
            } else if !is_highlighter {
                filtered[0].pressure = filtered[0].pressure.max(last.pressure);
            }
        } else {
            filtered.push(last);
        }
    }

    filtered
}

#[inline]
fn aa_radii(radius: f32, aa_feather: f32) -> (f32, f32) {
    if aa_feather <= 1e-4 {
        return (radius, radius);
    }
    let half_f = (aa_feather * 0.5).min(radius * 0.65);
    ((radius - half_f).max(0.0), radius + half_f)
}

#[inline]
fn push_quad(indices: &mut Vec<u32>, a: u32, b: u32, c: u32, d: u32) {
    indices.extend_from_slice(&[a, b, c, b, d, c]);
}

fn tessellate_single_ribbon_aa(
    points: &[StrokePoint],
    base_width: f32,
    color: Color32,
    is_highlighter: bool,
    aa_feather: f32,
    page_to_screen: &impl Fn(Pos2) -> Pos2,
    mesh: &mut Mesh,
) {
    if points.is_empty() {
        return;
    }
    if points.len() == 1 {
        let coord_scale = (page_to_screen(Pos2::new(1.0, 0.0)) - page_to_screen(Pos2::ZERO))
            .length()
            .max(1e-4);
        let page_base_width = base_width / coord_scale;
        let center = page_to_screen(points[0].to_pos2());
        let r = stroke_radius_at_pressure(page_base_width, points[0].pressure, is_highlighter)
            * coord_scale;
        let segs = if aa_feather > 1e-4 { 20 } else { 16 };
        add_round_cap_aa(mesh, center, r, color, aa_feather, segs);
        return;
    }

    let coord_scale = (page_to_screen(Pos2::new(1.0, 0.0)) - page_to_screen(Pos2::ZERO))
        .length()
        .max(1e-4);
    let page_base_width = base_width / coord_scale;

    let filtered = filter_stroke_points_adaptive(points, page_base_width, is_highlighter);
    if filtered.len() == 1 {
        let center = page_to_screen(filtered[0].to_pos2());
        let r = stroke_radius_at_pressure(page_base_width, filtered[0].pressure, is_highlighter)
            * coord_scale;
        let segs = if aa_feather > 1e-4 { 20 } else { 16 };
        add_round_cap_aa(mesh, center, r, color, aa_feather, segs);
        return;
    }
    let pts_to_use = &filtered[..];

    let mut screen_pts = Vec::with_capacity(pts_to_use.len());
    let mut radii = Vec::with_capacity(pts_to_use.len());
    for pt in pts_to_use {
        screen_pts.push(page_to_screen(pt.to_pos2()));
        let r =
            stroke_radius_at_pressure(page_base_width, pt.pressure, is_highlighter) * coord_scale;
        radii.push(r);
    }

    let m = screen_pts.len();
    let mut seg_dirs = Vec::with_capacity(m - 1);
    let mut seg_norms = Vec::with_capacity(m - 1);
    for i in 0..(m - 1) {
        let d = screen_pts[i + 1] - screen_pts[i];
        let len = d.length();
        let dir = if len > 1e-5 {
            d / len
        } else {
            Vec2::new(1.0, 0.0)
        };
        let norm = Vec2::new(-dir.y, dir.x);
        seg_dirs.push(dir);
        seg_norms.push(norm);
    }

    let has_aa = aa_feather > 1e-4;
    let mut aa_indices: Vec<u32> = if has_aa {
        Vec::with_capacity(m * 16)
    } else {
        Vec::new()
    };
    let mut core_indices: Vec<u32> = Vec::with_capacity(m * 12);

    // Start semicircle cap (untuk Pen, Pencil, dan Highlighter agar ujung membulat seperti kapsul).
    // Hitung segmen berdasarkan ketebalan dalam koordinat halaman agar tetap 100% zoom-invariant.
    let cap_segments = ((page_base_width * 0.45).ceil() as usize).clamp(12, 24);
    let is_opaque = !is_highlighter && color.a() == 255;
    let start_outward = -seg_dirs[0];
    add_semicircle_cap_aa(
        mesh,
        &mut aa_indices,
        &mut core_indices,
        screen_pts[0],
        radii[0],
        start_outward,
        color,
        aa_feather,
        cap_segments,
    );

    // Titik awal pita (i = 0)
    let p0 = screen_pts[0];
    let n0 = seg_norms[0];
    let (r0_core, r0_outer) = aa_radii(radii[0], aa_feather);
    let mut prev_left = p0 + n0 * r0_core;
    let mut prev_right = p0 - n0 * r0_core;
    let mut curr_l_idx = mesh.vertices.len() as u32;
    mesh.colored_vertex(prev_left, color);
    let mut curr_r_idx = mesh.vertices.len() as u32;
    mesh.colored_vertex(prev_right, color);

    let (mut curr_l_outer_idx, mut curr_r_outer_idx) = if has_aa {
        let l_out = mesh.vertices.len() as u32;
        mesh.colored_vertex(p0 + n0 * r0_outer, Color32::TRANSPARENT);
        let r_out = mesh.vertices.len() as u32;
        mesh.colored_vertex(p0 - n0 * r0_outer, Color32::TRANSPARENT);
        (l_out, r_out)
    } else {
        (0, 0)
    };

    // Bangun simpul pita sepanjang titik interior bebas spike (tanpa ekstrusi tangensial keluar radius).
    for i in 1..(m - 1) {
        let p = screen_pts[i];
        let r = radii[i];
        let (r_core, r_outer) = aa_radii(r, aa_feather);
        let t_in = seg_dirs[i - 1];
        let n_in = seg_norms[i - 1];
        let t_out = seg_dirs[i];
        let n_out = seg_norms[i];

        let cos_turn = t_in.dot(t_out).clamp(-1.0, 1.0);
        let cross = t_in.x * t_out.y - t_in.y * t_out.x;

        if is_opaque {
            if cos_turn >= 0.998 {
                let sum_n = n_in + n_out;
                let bisect = if sum_n.length_sq() > 1e-5 {
                    sum_n.normalized()
                } else {
                    n_out
                };
                let left = p + bisect * r_core;
                let right = p - bisect * r_core;

                let next_l_idx = mesh.vertices.len() as u32;
                mesh.colored_vertex(left, color);
                let next_r_idx = mesh.vertices.len() as u32;
                mesh.colored_vertex(right, color);
                push_quad(
                    &mut core_indices,
                    curr_l_idx,
                    curr_r_idx,
                    next_l_idx,
                    next_r_idx,
                );

                if has_aa {
                    let next_l_outer = mesh.vertices.len() as u32;
                    mesh.colored_vertex(p + bisect * r_outer, Color32::TRANSPARENT);
                    let next_r_outer = mesh.vertices.len() as u32;
                    mesh.colored_vertex(p - bisect * r_outer, Color32::TRANSPARENT);
                    push_quad(
                        &mut aa_indices,
                        curr_l_idx,
                        curr_l_outer_idx,
                        next_l_idx,
                        next_l_outer,
                    );
                    push_quad(
                        &mut aa_indices,
                        curr_r_idx,
                        curr_r_outer_idx,
                        next_r_idx,
                        next_r_outer,
                    );
                    curr_l_outer_idx = next_l_outer;
                    curr_r_outer_idx = next_r_outer;
                }

                curr_l_idx = next_l_idx;
                curr_r_idx = next_r_idx;
                prev_left = left;
                prev_right = right;
                continue;
            }
        } else if cos_turn >= 0.95 {
            let sum_n = n_in + n_out;
            let bisect = if sum_n.length_sq() > 1e-5 {
                sum_n.normalized()
            } else {
                n_out
            };
            let miter_scale = (1.0 / bisect.dot(n_in).abs().max(0.95)).clamp(1.0, 1.03);
            let eff_r = r_core * miter_scale;
            let left = p + bisect * eff_r;
            let right = p - bisect * eff_r;
            let l_proj = (left - prev_left).dot(t_in);
            let r_proj = (right - prev_right).dot(t_in);
            if l_proj >= 0.0 && r_proj >= 0.0 {
                let next_l_idx = mesh.vertices.len() as u32;
                mesh.colored_vertex(left, color);
                let next_r_idx = mesh.vertices.len() as u32;
                mesh.colored_vertex(right, color);
                push_quad(
                    &mut core_indices,
                    curr_l_idx,
                    curr_r_idx,
                    next_l_idx,
                    next_r_idx,
                );

                if has_aa {
                    let eff_outer = r_outer * miter_scale;
                    let next_l_outer = mesh.vertices.len() as u32;
                    mesh.colored_vertex(p + bisect * eff_outer, Color32::TRANSPARENT);
                    let next_r_outer = mesh.vertices.len() as u32;
                    mesh.colored_vertex(p - bisect * eff_outer, Color32::TRANSPARENT);
                    push_quad(
                        &mut aa_indices,
                        curr_l_idx,
                        curr_l_outer_idx,
                        next_l_idx,
                        next_l_outer,
                    );
                    push_quad(
                        &mut aa_indices,
                        curr_r_idx,
                        curr_r_outer_idx,
                        next_r_idx,
                        next_r_outer,
                    );
                    curr_l_outer_idx = next_l_outer;
                    curr_r_outer_idx = next_r_outer;
                }

                curr_l_idx = next_l_idx;
                curr_r_idx = next_r_idx;
                prev_left = left;
                prev_right = right;
                continue;
            }
        }

        // Sambungan tegak lurus + kipas Round Join melingkar pada belokan atau pelipatan dalam:
        // seluruh vertex berada persis pada jarak <= r dari p sehingga spike secara matematis mustahil terjadi.
        let in_left = p + n_in * r_core;
        let in_right = p - n_in * r_core;
        let in_l_idx = mesh.vertices.len() as u32;
        mesh.colored_vertex(in_left, color);
        let in_r_idx = mesh.vertices.len() as u32;
        mesh.colored_vertex(in_right, color);
        push_quad(
            &mut core_indices,
            curr_l_idx,
            curr_r_idx,
            in_l_idx,
            in_r_idx,
        );

        if has_aa {
            let in_l_outer = mesh.vertices.len() as u32;
            mesh.colored_vertex(p + n_in * r_outer, Color32::TRANSPARENT);
            let in_r_outer = mesh.vertices.len() as u32;
            mesh.colored_vertex(p - n_in * r_outer, Color32::TRANSPARENT);
            push_quad(
                &mut aa_indices,
                curr_l_idx,
                curr_l_outer_idx,
                in_l_idx,
                in_l_outer,
            );
            push_quad(
                &mut aa_indices,
                curr_r_idx,
                curr_r_outer_idx,
                in_r_idx,
                in_r_outer,
            );
        }

        let out_left = p + n_out * r_core;
        let out_right = p - n_out * r_core;
        let out_l_idx = mesh.vertices.len() as u32;
        mesh.colored_vertex(out_left, color);
        let out_r_idx = mesh.vertices.len() as u32;
        mesh.colored_vertex(out_right, color);

        let (out_l_outer, out_r_outer) = if has_aa {
            let l_out = mesh.vertices.len() as u32;
            mesh.colored_vertex(p + n_out * r_outer, Color32::TRANSPARENT);
            let r_out = mesh.vertices.len() as u32;
            mesh.colored_vertex(p - n_out * r_outer, Color32::TRANSPARENT);
            (l_out, r_out)
        } else {
            (0, 0)
        };

        let center_idx = mesh.vertices.len() as u32;
        mesh.colored_vertex(p, color);

        if !is_opaque {
            if cross >= 0.0 {
                core_indices.extend_from_slice(&[center_idx, in_l_idx, out_l_idx]);
            } else {
                core_indices.extend_from_slice(&[center_idx, in_r_idx, out_r_idx]);
            }
        } else {
            core_indices.extend_from_slice(&[
                center_idx, in_l_idx, out_l_idx, center_idx, in_r_idx, out_r_idx,
            ]);
        }

        let (start_norm, end_norm) = if cross >= 0.0 {
            (-n_in, -n_out)
        } else {
            (n_in, n_out)
        };
        add_round_join_fan_aa(
            mesh,
            &mut aa_indices,
            &mut core_indices,
            center_idx,
            p,
            r,
            start_norm,
            end_norm,
            cross,
            color,
            aa_feather,
        );

        curr_l_idx = out_l_idx;
        curr_r_idx = out_r_idx;
        curr_l_outer_idx = out_l_outer;
        curr_r_outer_idx = out_r_outer;
        prev_left = out_left;
        prev_right = out_right;
    }

    // Titik akhir pita (i = m - 1)
    let last_idx = m - 1;
    let p_last = screen_pts[last_idx];
    let r_last = radii[last_idx];
    let (r_last_core, r_last_outer) = aa_radii(r_last, aa_feather);
    let t_last = seg_dirs[last_idx - 1];
    let n_last = seg_norms[last_idx - 1];

    let left_last = p_last + n_last * r_last_core;
    let right_last = p_last - n_last * r_last_core;

    let end_l_idx = mesh.vertices.len() as u32;
    mesh.colored_vertex(left_last, color);
    let end_r_idx = mesh.vertices.len() as u32;
    mesh.colored_vertex(right_last, color);
    push_quad(
        &mut core_indices,
        curr_l_idx,
        curr_r_idx,
        end_l_idx,
        end_r_idx,
    );

    if has_aa {
        let end_l_outer = mesh.vertices.len() as u32;
        mesh.colored_vertex(p_last + n_last * r_last_outer, Color32::TRANSPARENT);
        let end_r_outer = mesh.vertices.len() as u32;
        mesh.colored_vertex(p_last - n_last * r_last_outer, Color32::TRANSPARENT);
        push_quad(
            &mut aa_indices,
            curr_l_idx,
            curr_l_outer_idx,
            end_l_idx,
            end_l_outer,
        );
        push_quad(
            &mut aa_indices,
            curr_r_idx,
            curr_r_outer_idx,
            end_r_idx,
            end_r_outer,
        );
    }

    // End semicircle cap
    add_semicircle_cap_aa(
        mesh,
        &mut aa_indices,
        &mut core_indices,
        p_last,
        r_last,
        t_last,
        color,
        aa_feather,
        cap_segments,
    );

    mesh.indices.extend_from_slice(&aa_indices);
    mesh.indices.extend_from_slice(&core_indices);
}

/// Menambahkan kipas busur melingkar (Round Join) pada sudut tajam untuk mencegah ujung bergerigi atau lonjakan miter.
#[allow(clippy::too_many_arguments)]
fn add_round_join_fan_aa(
    mesh: &mut Mesh,
    aa_indices: &mut Vec<u32>,
    core_indices: &mut Vec<u32>,
    center_idx: u32,
    center: Pos2,
    radius: f32,
    start_norm: Vec2,
    end_norm: Vec2,
    cross: f32,
    color: Color32,
    aa_feather: f32,
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
    // Pada belokan hampir 180 derajat (hairpin), pastikan arah sapuan mengikuti tanda cross product
    if delta.abs() > std::f32::consts::PI - 0.05 {
        delta = if cross >= 0.0 {
            std::f32::consts::PI
        } else {
            -std::f32::consts::PI
        };
    }

    let (r_core, r_outer) = aa_radii(radius, aa_feather);
    let has_aa = aa_feather > 1e-4;
    let steps = ((delta.abs() / 0.18).ceil() as usize).clamp(2, 16);
    let start_idx = mesh.vertices.len() as u32;
    for s in 0..=steps {
        let t = s as f32 / steps as f32;
        let angle = a0 + delta * t;
        let pos = center + Vec2::new(angle.cos(), angle.sin()) * r_core;
        mesh.colored_vertex(pos, color);
    }
    for s in 0..steps as u32 {
        core_indices.extend_from_slice(&[center_idx, start_idx + s, start_idx + s + 1]);
    }

    if has_aa {
        let outer_start = mesh.vertices.len() as u32;
        for s in 0..=steps {
            let t = s as f32 / steps as f32;
            let angle = a0 + delta * t;
            let pos = center + Vec2::new(angle.cos(), angle.sin()) * r_outer;
            mesh.colored_vertex(pos, Color32::TRANSPARENT);
        }
        for s in 0..steps as u32 {
            push_quad(
                aa_indices,
                start_idx + s,
                outer_start + s,
                start_idx + s + 1,
                outer_start + s + 1,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn add_semicircle_cap_aa(
    mesh: &mut Mesh,
    aa_indices: &mut Vec<u32>,
    core_indices: &mut Vec<u32>,
    center: Pos2,
    radius: f32,
    outward_dir: Vec2,
    color: Color32,
    aa_feather: f32,
    segments: usize,
) {
    if outward_dir.length_sq() < 1e-6 {
        return;
    }
    let (r_core, r_outer) = aa_radii(radius, aa_feather);
    let has_aa = aa_feather > 1e-4;
    let center_idx = mesh.vertices.len() as u32;
    mesh.colored_vertex(center, color);

    let base_angle = outward_dir.y.atan2(outward_dir.x);
    let start_idx = mesh.vertices.len() as u32;

    for i in 0..=segments {
        let offset_angle =
            -std::f32::consts::FRAC_PI_2 + (i as f32 / segments as f32) * std::f32::consts::PI;
        let angle = base_angle + offset_angle;
        let pos = center + Vec2::new(angle.cos(), angle.sin()) * r_core;
        mesh.colored_vertex(pos, color);
    }

    for i in 0..segments as u32 {
        core_indices.extend_from_slice(&[center_idx, start_idx + i, start_idx + i + 1]);
    }

    if has_aa {
        let outer_start = mesh.vertices.len() as u32;
        for i in 0..=segments {
            let offset_angle =
                -std::f32::consts::FRAC_PI_2 + (i as f32 / segments as f32) * std::f32::consts::PI;
            let angle = base_angle + offset_angle;
            let pos = center + Vec2::new(angle.cos(), angle.sin()) * r_outer;
            mesh.colored_vertex(pos, Color32::TRANSPARENT);
        }
        for i in 0..segments as u32 {
            push_quad(
                aa_indices,
                start_idx + i,
                outer_start + i,
                start_idx + i + 1,
                outer_start + i + 1,
            );
        }
    }
}

fn add_round_cap_aa(
    mesh: &mut Mesh,
    center: Pos2,
    radius: f32,
    color: Color32,
    aa_feather: f32,
    segments: usize,
) {
    let (r_core, r_outer) = aa_radii(radius, aa_feather);
    let has_aa = aa_feather > 1e-4;
    let mut aa_indices = Vec::new();
    let mut core_indices = Vec::new();

    let center_idx = mesh.vertices.len() as u32;
    mesh.colored_vertex(center, color);

    let start_idx = mesh.vertices.len() as u32;
    for i in 0..=segments {
        let angle = (i as f32 / segments as f32) * std::f32::consts::TAU;
        let pos = center + Vec2::new(angle.cos(), angle.sin()) * r_core;
        mesh.colored_vertex(pos, color);
    }

    for i in 0..segments as u32 {
        core_indices.extend_from_slice(&[center_idx, start_idx + i, start_idx + i + 1]);
    }

    if has_aa {
        let outer_start = mesh.vertices.len() as u32;
        for i in 0..=segments {
            let angle = (i as f32 / segments as f32) * std::f32::consts::TAU;
            let pos = center + Vec2::new(angle.cos(), angle.sin()) * r_outer;
            mesh.colored_vertex(pos, Color32::TRANSPARENT);
        }
        for i in 0..segments as u32 {
            push_quad(
                &mut aa_indices,
                start_idx + i,
                outer_start + i,
                start_idx + i + 1,
                outer_start + i + 1,
            );
        }
    }

    mesh.indices.extend_from_slice(&aa_indices);
    mesh.indices.extend_from_slice(&core_indices);
}

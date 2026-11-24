use super::tessellation::{
    StrokeStylePart, decompose_styled_stroke, smooth_stroke_points, stroke_radius_at_pressure,
};
use crate::model::{ColorData, HIGHLIGHTER_ALPHA, StrokePoint, StrokeStyle};
use eframe::egui::{Color32, Pos2};
use std::collections::HashMap;

/// Mengonversi nilai opacity desimal (`0.0..=1.0`) menjadi alpha 8-bit (`20..=255`).
#[inline]
pub fn highlighter_opacity_to_alpha(opacity: f32) -> u8 {
    if opacity.is_finite() {
        (opacity * 255.0).round().clamp(20.0, 255.0) as u8
    } else {
        HIGHLIGHTER_ALPHA
    }
}

/// Menentukan alpha efektif sebuah goresan highlighter secara konsisten di kanvas dan ekspor.
/// Bila `override_opacity` diberikan (pengaturan aktif pada kanvas), nilainya diprioritaskan;
/// jika tidak, gunakan alpha tersimpan apabila valid (`1..=254`) dan fallback ke `HIGHLIGHTER_ALPHA`.
#[inline]
pub fn resolve_highlighter_alpha(stored_alpha: u8, override_opacity: Option<f32>) -> u8 {
    if let Some(op) = override_opacity {
        highlighter_opacity_to_alpha(op)
    } else if stored_alpha > 0 && stored_alpha < 255 {
        stored_alpha
    } else {
        HIGHLIGHTER_ALPHA
    }
}

/// Membentuk `ColorData` unmultiplied untuk goresan highlighter yang baru di-commit,
/// mencegah premultiplikasi ganda saat warna dibaca kembali oleh renderer maupun exporter.
#[inline]
pub fn highlighter_stroke_color_data(stroke_color: Color32, highlighter_opacity: f32) -> ColorData {
    let [r, g, b, _] = stroke_color.to_srgba_unmultiplied();
    ColorData {
        r,
        g,
        b,
        a: highlighter_opacity_to_alpha(highlighter_opacity),
    }
}

/// Deskriptor masukan sebuah goresan highlighter untuk pengelompokan dan rasterisasi coverage mask.
#[derive(Clone, Copy, Debug)]
pub struct HighlighterStrokeInput<'a> {
    pub points: &'a [StrokePoint],
    pub rgb: (u8, u8, u8),
    pub width: f32,
    pub alpha: u8,
    pub style: StrokeStyle,
}

/// Sekelompok goresan highlighter dengan warna RGB yang sama untuk dikompositkan dalam satu pass.
#[derive(Clone, Debug)]
pub struct HighlighterColorGroup<'a> {
    pub rgb: (u8, u8, u8),
    pub strokes: Vec<HighlighterStrokeInput<'a>>,
}

impl<'a> HighlighterColorGroup<'a> {
    /// Mengembalikan nilai alpha maksimum di dalam grup warna ini (fallback ke `HIGHLIGHTER_ALPHA`).
    pub fn max_alpha(&self) -> u8 {
        self.strokes
            .iter()
            .map(|s| s.alpha)
            .filter(|&a| a > 0)
            .max()
            .unwrap_or(HIGHLIGHTER_ALPHA)
    }
}

/// Mengelompokkan goresan highlighter berdasarkan RGB sambil mempertahankan urutan kemunculan pertama
/// yang deterministik sehingga urutan tumpukan antarwarna konsisten antara preview, kanvas, dan ekspor.
pub fn group_highlighters_by_rgb<'a>(
    strokes: impl IntoIterator<Item = HighlighterStrokeInput<'a>>,
) -> Vec<HighlighterColorGroup<'a>> {
    let mut groups: Vec<HighlighterColorGroup<'a>> = Vec::new();
    let mut group_indices: HashMap<(u8, u8, u8), usize> = HashMap::new();

    for stroke in strokes {
        if stroke.points.is_empty() {
            continue;
        }
        if let Some(&idx) = group_indices.get(&stroke.rgb) {
            groups[idx].strokes.push(stroke);
        } else {
            let idx = groups.len();
            group_indices.insert(stroke.rgb, idx);
            groups.push(HighlighterColorGroup {
                rgb: stroke.rgb,
                strokes: vec![stroke],
            });
        }
    }

    groups
}

/// Transformasi koordinat ruang halaman (`page_pos`) ke piksel coverage mask (`0..mask_w`, `0..mask_h`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MaskTransform {
    pub origin_x: f32,
    pub origin_y: f32,
    pub scale: f32,
}

impl MaskTransform {
    #[inline]
    pub fn map_pos(&self, p: Pos2) -> Pos2 {
        Pos2::new(
            (p.x - self.origin_x) * self.scale,
            (p.y - self.origin_y) * self.scale,
        )
    }

    #[inline]
    pub fn map_pt(&self, pt: &StrokePoint) -> Pos2 {
        Pos2::new(
            (pt.x - self.origin_x) * self.scale,
            (pt.y - self.origin_y) * self.scale,
        )
    }
}

/// Merasterisasi seluruh goresan highlighter dalam satu grup warna ke dalam `mask` menggunakan
/// operator maksimum (`max`) per piksel sehingga tumpang tindih tidak pernah mengakumulasi alpha.
pub fn rasterize_highlighter_group_mask(
    mask: &mut [u8],
    mask_w: usize,
    mask_h: usize,
    group: &HighlighterColorGroup<'_>,
    transform: MaskTransform,
    smoothing_steps: usize,
) {
    if mask_w == 0 || mask_h == 0 || !transform.scale.is_finite() || transform.scale <= 0.0 {
        return;
    }
    if mask.len() < mask_w.saturating_mul(mask_h) {
        return;
    }

    for stroke in &group.strokes {
        if stroke.points.is_empty() || stroke.alpha == 0 {
            continue;
        }
        let page_r = stroke_radius_at_pressure(stroke.width, 1.0, true);
        let r_px = (page_r * transform.scale).max(0.6);
        let smoothed = smooth_stroke_points(stroke.points, smoothing_steps);
        let parts = decompose_styled_stroke(&smoothed, stroke.width, stroke.style);

        for part in parts {
            match part {
                StrokeStylePart::Dot(pt) => {
                    let center = transform.map_pt(&pt);
                    rasterize_circle_max(mask, mask_w, mask_h, center, r_px, stroke.alpha);
                }
                StrokeStylePart::Polyline(pts) => {
                    if pts.len() == 1 {
                        let center = transform.map_pt(&pts[0]);
                        rasterize_circle_max(mask, mask_w, mask_h, center, r_px, stroke.alpha);
                    } else {
                        for w in pts.windows(2) {
                            let a = transform.map_pt(&w[0]);
                            let b = transform.map_pt(&w[1]);
                            rasterize_capsule_segment_max(
                                mask,
                                mask_w,
                                mask_h,
                                a,
                                b,
                                r_px,
                                stroke.alpha,
                            );
                        }
                    }
                }
            }
        }
    }
}

fn rasterize_circle_max(
    mask: &mut [u8],
    mask_w: usize,
    mask_h: usize,
    center: Pos2,
    r_px: f32,
    alpha: u8,
) {
    if !center.x.is_finite() || !center.y.is_finite() {
        return;
    }
    let outer_r = r_px + 0.5;
    let max_x_bound = mask_w as f32 - 1.0;
    let max_y_bound = mask_h as f32 - 1.0;
    let px_min_f = (center.x - outer_r).floor();
    let px_max_f = (center.x + outer_r).ceil();
    let py_min_f = (center.y - outer_r).floor();
    let py_max_f = (center.y + outer_r).ceil();
    if px_max_f < 0.0 || py_max_f < 0.0 || px_min_f > max_x_bound || py_min_f > max_y_bound {
        return;
    }

    let px_min = px_min_f.max(0.0) as usize;
    let px_max = px_max_f.min(max_x_bound) as usize;
    let py_min = py_min_f.max(0.0) as usize;
    let py_max = py_max_f.min(max_y_bound) as usize;

    let outer_r_sq = outer_r * outer_r;
    let inner_r = (r_px - 0.5).max(0.0);
    let inner_r_sq = inner_r * inner_r;
    let alpha_f = alpha as f32;

    for py in py_min..=py_max {
        let row_offset = py * mask_w;
        let dy = (py as f32 + 0.5) - center.y;
        let dy_sq = dy * dy;
        for px in px_min..=px_max {
            let dx = (px as f32 + 0.5) - center.x;
            let d_sq = dx * dx + dy_sq;
            if d_sq < outer_r_sq {
                let eff_a = if d_sq <= inner_r_sq {
                    alpha
                } else {
                    let cov = (outer_r - d_sq.sqrt()).clamp(0.0, 1.0);
                    (alpha_f * cov).round() as u8
                };
                let slot = &mut mask[row_offset + px];
                if eff_a > *slot {
                    *slot = eff_a;
                }
            }
        }
    }
}

fn rasterize_capsule_segment_max(
    mask: &mut [u8],
    mask_w: usize,
    mask_h: usize,
    a: Pos2,
    b: Pos2,
    r_px: f32,
    alpha: u8,
) {
    if !a.x.is_finite() || !a.y.is_finite() || !b.x.is_finite() || !b.y.is_finite() {
        return;
    }
    let ab = b - a;
    let ab_len_sq = ab.length_sq();
    if ab_len_sq < 1e-6 {
        rasterize_circle_max(
            mask,
            mask_w,
            mask_h,
            Pos2::new((a.x + b.x) * 0.5, (a.y + b.y) * 0.5),
            r_px,
            alpha,
        );
        return;
    }

    let outer_r = r_px + 0.5;
    let max_x_bound = mask_w as f32 - 1.0;
    let max_y_bound = mask_h as f32 - 1.0;
    let px_min_f = (a.x.min(b.x) - outer_r).floor();
    let px_max_f = (a.x.max(b.x) + outer_r).ceil();
    let py_min_f = (a.y.min(b.y) - outer_r).floor();
    let py_max_f = (a.y.max(b.y) + outer_r).ceil();
    if px_max_f < 0.0 || py_max_f < 0.0 || px_min_f > max_x_bound || py_min_f > max_y_bound {
        return;
    }

    let px_min = px_min_f.max(0.0) as usize;
    let px_max = px_max_f.min(max_x_bound) as usize;
    let py_min = py_min_f.max(0.0) as usize;
    let py_max = py_max_f.min(max_y_bound) as usize;

    let inv_ab_len_sq = 1.0 / ab_len_sq;
    let outer_r_sq = outer_r * outer_r;
    let inner_r = (r_px - 0.5).max(0.0);
    let inner_r_sq = inner_r * inner_r;
    let alpha_f = alpha as f32;

    for py in py_min..=py_max {
        let row_offset = py * mask_w;
        let py_c = py as f32 + 0.5;
        let rel_y = py_c - a.y;
        for px in px_min..=px_max {
            let px_c = px as f32 + 0.5;
            let rel_x = px_c - a.x;
            let t = ((rel_x * ab.x + rel_y * ab.y) * inv_ab_len_sq).clamp(0.0, 1.0);
            let dx = px_c - (a.x + ab.x * t);
            let dy = py_c - (a.y + ab.y * t);
            let d_sq = dx * dx + dy * dy;
            if d_sq < outer_r_sq {
                let eff_a = if d_sq <= inner_r_sq {
                    alpha
                } else {
                    let cov = (outer_r - d_sq.sqrt()).clamp(0.0, 1.0);
                    (alpha_f * cov).round() as u8
                };
                let slot = &mut mask[row_offset + px];
                if eff_a > *slot {
                    *slot = eff_a;
                }
            }
        }
    }
}

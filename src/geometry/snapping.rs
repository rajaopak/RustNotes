use crate::model::Point2D;
use eframe::egui::{Rect, Vec2};

/// Orientasi garis panduan magnetik (Smart Alignment Guide).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuideOrientation {
    /// Garis horizontal (membentang sepanjang sumbu X pada ketinggian Y tertentu)
    Horizontal,
    /// Garis vertikal (membentang sepanjang sumbu Y pada posisi X tertentu)
    Vertical,
}

/// Garis panduan visual penyelarasan magnetik antar-objek (Smart Guides).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AlignmentGuide {
    pub orientation: GuideOrientation,
    /// Koordinat garis: nilai Y untuk garis Horizontal, nilai X untuk garis Vertical.
    pub position: f32,
    /// Batas awal rentang garis panduan (X_min jika Horizontal, Y_min jika Vertical).
    pub min_span: f32,
    /// Batas akhir rentang garis panduan (X_max jika Horizontal, Y_max jika Vertical).
    pub max_span: f32,
}

/// Menarik nilai skalar ke kelipatan grid terdekat jika dalam jarak `threshold`.
/// Mengembalikan `(snapped_value, is_snapped)`.
pub fn snap_value_to_grid(val: f32, grid_size: f32, threshold: f32) -> (f32, bool) {
    if grid_size <= 0.0 {
        return (val, false);
    }
    let nearest = (val / grid_size).round() * grid_size;
    let dist = (val - nearest).abs();
    if dist <= threshold {
        (nearest, true)
    } else {
        (val, false)
    }
}

/// Menarik koordinat 2D ke perpotongan kisi (*grid intersection*) terdekat.
pub fn snap_point_to_grid(p: Point2D, grid_size: f32, threshold: f32) -> (Point2D, bool) {
    let (sx, snapped_x) = snap_value_to_grid(p.x, grid_size, threshold);
    let (sy, snapped_y) = snap_value_to_grid(p.y, grid_size, threshold);
    (Point2D::new(sx, sy), snapped_x || snapped_y)
}

/// Mencari penyesuaian delta `(dx, dy)` dan garis pemandu visual penyelarasan (*smart alignment guides*)
/// saat sebuah kotak seleksi `moving_rect` digeser mendekati sekumpulan bentuk lain `target_rects`.
pub fn find_alignment_guides(
    moving_rect: Rect,
    target_rects: &[Rect],
    threshold: f32,
) -> (Vec2, Vec<AlignmentGuide>) {
    let mut best_dx = 0.0_f32;
    let mut min_x_dist = f32::MAX;
    let mut x_guide: Option<AlignmentGuide> = None;

    let mut best_dy = 0.0_f32;
    let mut min_y_dist = f32::MAX;
    let mut y_guide: Option<AlignmentGuide> = None;

    let moving_xs = [
        moving_rect.left(),
        moving_rect.center().x,
        moving_rect.right(),
    ];
    let moving_ys = [
        moving_rect.top(),
        moving_rect.center().y,
        moving_rect.bottom(),
    ];

    for &target in target_rects {
        let target_xs = [target.left(), target.center().x, target.right()];
        let target_ys = [target.top(), target.center().y, target.bottom()];

        // 1. Evaluasi Penyelarasan Sumbu X (Garis Pemandu Vertikal)
        for &mx in &moving_xs {
            for &tx in &target_xs {
                let diff = tx - mx;
                let dist = diff.abs();
                if dist <= threshold && dist < min_x_dist {
                    min_x_dist = dist;
                    best_dx = diff;
                    let min_y = moving_rect.top().min(target.top()) - 10.0;
                    let max_y = moving_rect.bottom().max(target.bottom()) + 10.0;
                    x_guide = Some(AlignmentGuide {
                        orientation: GuideOrientation::Vertical,
                        position: tx,
                        min_span: min_y,
                        max_span: max_y,
                    });
                }
            }
        }

        // 2. Evaluasi Penyelarasan Sumbu Y (Garis Pemandu Horizontal)
        for &my in &moving_ys {
            for &ty in &target_ys {
                let diff = ty - my;
                let dist = diff.abs();
                if dist <= threshold && dist < min_y_dist {
                    min_y_dist = dist;
                    best_dy = diff;
                    let min_x = moving_rect.left().min(target.left()) - 10.0;
                    let max_x = moving_rect.right().max(target.right()) + 10.0;
                    y_guide = Some(AlignmentGuide {
                        orientation: GuideOrientation::Horizontal,
                        position: ty,
                        min_span: min_x,
                        max_span: max_x,
                    });
                }
            }
        }
    }

    let mut guides = Vec::new();
    if let Some(xg) = x_guide {
        guides.push(xg);
    }
    if let Some(yg) = y_guide {
        guides.push(yg);
    }

    (Vec2::new(best_dx, best_dy), guides)
}

use super::primitives::object_bounds_with_context;
use crate::model::DrawObject;
use eframe::egui::{Pos2, Rect};
use std::collections::HashMap;

const MAX_CELLS_PER_OBJECT: usize = 4096;
const MAX_CELL_COORD: f32 = 100_000.0;

/// 2D Uniform Grid Spatial Index for O(1) / O(k) hit testing on dense pages.
#[derive(Clone, Debug)]
pub struct SpatialGrid {
    cell_size: f32,
    cells: HashMap<(i32, i32), Vec<usize>>,
    overflow_indices: Vec<usize>,
}

impl Default for SpatialGrid {
    fn default() -> Self {
        Self {
            cell_size: 100.0,
            cells: HashMap::new(),
            overflow_indices: Vec::new(),
        }
    }
}

impl SpatialGrid {
    pub fn new(cell_size: f32) -> Self {
        Self {
            cell_size: cell_size.max(20.0),
            cells: HashMap::new(),
            overflow_indices: Vec::new(),
        }
    }

    #[inline]
    fn cell_coord(&self, v: f32) -> i32 {
        let cs = self.cell_size.max(20.0);
        let q = v / cs;
        if !q.is_finite() {
            return if v.is_sign_negative() {
                -(MAX_CELL_COORD as i32)
            } else {
                MAX_CELL_COORD as i32
            };
        }
        q.clamp(-MAX_CELL_COORD, MAX_CELL_COORD).floor() as i32
    }

    pub fn rebuild(&mut self, objects: &[DrawObject]) {
        self.cells.clear();
        self.overflow_indices.clear();
        for (idx, obj) in objects.iter().enumerate() {
            let b = object_bounds_with_context(obj, objects);
            if !b.is_finite() || b == Rect::NOTHING || b.min.x > b.max.x || b.min.y > b.max.y {
                continue;
            }
            let pad = match obj {
                DrawObject::Stroke { width, .. } => (*width * 0.25).max(2.0),
                DrawObject::Connector { .. } => 14.0,
                _ => 2.0,
            };
            let eb = b.expand(pad);
            if !eb.is_finite() {
                self.overflow_indices.push(idx);
                continue;
            }
            let min_cx = self.cell_coord(eb.left());
            let max_cx = self.cell_coord(eb.right());
            let min_cy = self.cell_coord(eb.top());
            let max_cy = self.cell_coord(eb.bottom());

            let span_x = (max_cx as i64) - (min_cx as i64) + 1;
            let span_y = (max_cy as i64) - (min_cy as i64) + 1;
            if span_x <= 0
                || span_y <= 0
                || span_x.saturating_mul(span_y) > MAX_CELLS_PER_OBJECT as i64
            {
                self.overflow_indices.push(idx);
                continue;
            }

            for cx in min_cx..=max_cx {
                for cy in min_cy..=max_cy {
                    self.cells.entry((cx, cy)).or_default().push(idx);
                }
            }
        }
    }

    pub fn query_point(&self, p: Pos2, tolerance: f32) -> Vec<usize> {
        if !p.x.is_finite() || !p.y.is_finite() || !tolerance.is_finite() || tolerance < 0.0 {
            return Vec::new();
        }
        let tol = tolerance.max(0.0);
        let min_cx = self.cell_coord(p.x - tol);
        let max_cx = self.cell_coord(p.x + tol);
        let min_cy = self.cell_coord(p.y - tol);
        let max_cy = self.cell_coord(p.y + tol);

        self.collect_cell_range(min_cx, max_cx, min_cy, max_cy)
    }

    pub fn query_rect(&self, r: Rect) -> Vec<usize> {
        if !r.is_finite() || r == Rect::NOTHING || r.min.x > r.max.x || r.min.y > r.max.y {
            return Vec::new();
        }
        let min_cx = self.cell_coord(r.left());
        let max_cx = self.cell_coord(r.right());
        let min_cy = self.cell_coord(r.top());
        let max_cy = self.cell_coord(r.bottom());

        self.collect_cell_range(min_cx, max_cx, min_cy, max_cy)
    }

    fn collect_cell_range(&self, min_cx: i32, max_cx: i32, min_cy: i32, max_cy: i32) -> Vec<usize> {
        let span_x = (max_cx as i64) - (min_cx as i64) + 1;
        let span_y = (max_cy as i64) - (min_cy as i64) + 1;
        let mut candidates = Vec::new();
        if span_x <= 0 || span_y <= 0 {
            return candidates;
        }
        if span_x.saturating_mul(span_y) > MAX_CELLS_PER_OBJECT as i64 {
            for list in self.cells.values() {
                candidates.extend_from_slice(list);
            }
        } else {
            for cx in min_cx..=max_cx {
                for cy in min_cy..=max_cy {
                    if let Some(list) = self.cells.get(&(cx, cy)) {
                        candidates.extend_from_slice(list);
                    }
                }
            }
        }
        candidates.extend_from_slice(&self.overflow_indices);
        candidates.sort_unstable();
        candidates.dedup();
        candidates
    }

    /// Mengembalikan seluruh indeks objek yang lolos narrow-phase `hit_test_object_with_context`
    /// dalam urutan Z-order dari bawah ke atas (ascending index).
    pub fn hit_test_all(&self, objects: &[DrawObject], p: Pos2, tolerance: f32) -> Vec<usize> {
        let candidates = self.query_point(p, tolerance);
        candidates
            .into_iter()
            .filter(|&idx| {
                objects.get(idx).is_some_and(|obj| {
                    super::hit_test::hit_test_object_with_context(obj, p, tolerance, Some(objects))
                })
            })
            .collect()
    }

    /// Mengembalikan indeks dan referensi objek paling atas (Z-order tertinggi) yang terkena hit-test.
    pub fn hit_test_topmost<'a>(
        &self,
        objects: &'a [DrawObject],
        p: Pos2,
        tolerance: f32,
    ) -> Option<(usize, &'a DrawObject)> {
        let candidates = self.query_point(p, tolerance);
        for idx in candidates.into_iter().rev() {
            if let Some(obj) = objects.get(idx)
                && super::hit_test::hit_test_object_with_context(obj, p, tolerance, Some(objects))
            {
                return Some((idx, obj));
            }
        }
        None
    }

    /// Menyeleksi seluruh objek yang memenuhi `mode` (Containment atau Intersection) terhadap poligon lasso
    /// dengan menyaring kandidat menggunakan `SpatialGrid::query_rect` dilanjutkan evaluasi geometri presisi (Track 4).
    /// Mengembalikan indeks objek terpilih dalam urutan Z-order dari bawah ke atas.
    pub fn select_in_lasso(
        &self,
        objects: &[DrawObject],
        polygon: &[Pos2],
        mode: crate::model::LassoSelectionMode,
    ) -> Vec<usize> {
        let Some((clean_poly, lasso_bounds)) = super::primitives::prepare_lasso_polygon(polygon)
        else {
            return Vec::new();
        };
        let candidates = self.query_rect(lasso_bounds);
        candidates
            .into_iter()
            .filter(|&idx| {
                objects.get(idx).is_some_and(|obj| {
                    super::hit_test::object_in_lasso_mode_with_prepared_polygon(
                        obj,
                        &clean_poly,
                        lasso_bounds,
                        mode,
                        Some(objects),
                    )
                })
            })
            .collect()
    }
}

use super::primitives::object_bounds_with_context;
use crate::model::DrawObject;
use eframe::egui::{Pos2, Rect};
use std::collections::HashMap;

/// 2D Uniform Grid Spatial Index for O(1) / O(k) hit testing on dense pages.
#[derive(Clone, Debug)]
pub struct SpatialGrid {
    cell_size: f32,
    cells: HashMap<(i32, i32), Vec<usize>>,
}

impl Default for SpatialGrid {
    fn default() -> Self {
        Self {
            cell_size: 100.0,
            cells: HashMap::new(),
        }
    }
}

impl SpatialGrid {
    pub fn new(cell_size: f32) -> Self {
        Self {
            cell_size: cell_size.max(20.0),
            cells: HashMap::new(),
        }
    }

    pub fn rebuild(&mut self, objects: &[DrawObject]) {
        self.cells.clear();
        for (idx, obj) in objects.iter().enumerate() {
            let b = object_bounds_with_context(obj, objects);
            if !b.is_finite() || b.min.x > b.max.x || b.min.y > b.max.y {
                continue;
            }
            let pad = match obj {
                DrawObject::Stroke { width, .. } => (*width * 0.25).max(2.0),
                DrawObject::Connector { .. } => 14.0,
                _ => 2.0,
            };
            let eb = b.expand(pad);
            let min_cx = (eb.left() / self.cell_size).floor() as i32;
            let max_cx = (eb.right() / self.cell_size).floor() as i32;
            let min_cy = (eb.top() / self.cell_size).floor() as i32;
            let max_cy = (eb.bottom() / self.cell_size).floor() as i32;

            for cx in min_cx..=max_cx {
                for cy in min_cy..=max_cy {
                    self.cells.entry((cx, cy)).or_default().push(idx);
                }
            }
        }
    }

    pub fn query_point(&self, p: Pos2, tolerance: f32) -> Vec<usize> {
        let tol = tolerance.max(0.0);
        let min_cx = ((p.x - tol) / self.cell_size).floor() as i32;
        let max_cx = ((p.x + tol) / self.cell_size).floor() as i32;
        let min_cy = ((p.y - tol) / self.cell_size).floor() as i32;
        let max_cy = ((p.y + tol) / self.cell_size).floor() as i32;

        let mut candidates = Vec::new();
        for cx in min_cx..=max_cx {
            for cy in min_cy..=max_cy {
                if let Some(list) = self.cells.get(&(cx, cy)) {
                    candidates.extend_from_slice(list);
                }
            }
        }
        candidates.sort_unstable();
        candidates.dedup();
        candidates
    }

    pub fn query_rect(&self, r: Rect) -> Vec<usize> {
        let min_cx = (r.left() / self.cell_size).floor() as i32;
        let max_cx = (r.right() / self.cell_size).floor() as i32;
        let min_cy = (r.top() / self.cell_size).floor() as i32;
        let max_cy = (r.bottom() / self.cell_size).floor() as i32;

        let mut candidates = Vec::new();
        for cx in min_cx..=max_cx {
            for cy in min_cy..=max_cy {
                if let Some(list) = self.cells.get(&(cx, cy)) {
                    candidates.extend_from_slice(list);
                }
            }
        }
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
}

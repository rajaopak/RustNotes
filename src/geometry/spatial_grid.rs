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
            if !b.is_finite() || b.width() <= 0.0 || b.height() <= 0.0 {
                continue;
            }
            let min_cx = (b.left() / self.cell_size).floor() as i32;
            let max_cx = (b.right() / self.cell_size).floor() as i32;
            let min_cy = (b.top() / self.cell_size).floor() as i32;
            let max_cy = (b.bottom() / self.cell_size).floor() as i32;

            for cx in min_cx..=max_cx {
                for cy in min_cy..=max_cy {
                    self.cells.entry((cx, cy)).or_default().push(idx);
                }
            }
        }
    }

    pub fn query_point(&self, p: Pos2, tolerance: f32) -> Vec<usize> {
        let min_cx = ((p.x - tolerance) / self.cell_size).floor() as i32;
        let max_cx = ((p.x + tolerance) / self.cell_size).floor() as i32;
        let min_cy = ((p.y - tolerance) / self.cell_size).floor() as i32;
        let max_cy = ((p.y + tolerance) / self.cell_size).floor() as i32;

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
}

use super::types::{ActiveTool, EraserMode, LaserTrailPoint, ResizeCorner};
use crate::geometry::{
    SpatialGrid, combined_bounds, distance_to_object, hit_test_object, split_stroke_by_eraser,
};
use crate::model::{DrawObject, Page, StrokePoint, StrokeStyle};
use eframe::egui::{self, Color32, Mesh, Pos2, Rect, TextureHandle, Vec2};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;
use uuid::Uuid;

pub struct CanvasState {
    pub current_tool: ActiveTool,
    pub stroke_color: Color32,
    pub stroke_width: f32,
    pub stroke_style: StrokeStyle,
    pub zoom: f32,
    pub pan_offset: Vec2,
    pub smoothing_steps: usize,

    // Undo / Redo stacks: page_id -> Vec<Arc<Vec<DrawObject>>> (Snapshot structural sharing)
    pub undo_stacks: HashMap<String, Vec<Arc<Vec<DrawObject>>>>,
    pub redo_stacks: HashMap<String, Vec<Arc<Vec<DrawObject>>>>,
    pub undo_page_order: Vec<String>,
    pub redo_page_order: Vec<String>,

    // Active stroke / shape draft
    pub active_stroke_page_id: Option<String>,
    pub draft_stroke: Option<Vec<StrokePoint>>,
    pub draft_shape_start: Option<Pos2>,
    pub draft_shape_current: Option<Pos2>,
    pub last_stroke_move_time: Option<Instant>,
    pub recognized_draft_shape: Option<crate::geometry::RecognizedShape>,
    pub snap_initial_shape: Option<crate::geometry::RecognizedShape>,
    pub snap_flash_time: Option<Instant>,
    pub auto_shape_enabled: bool,

    // Lasso selection
    pub lasso_points: Vec<Pos2>,
    pub selected_ids: Vec<String>,
    pub is_dragging_selection: bool,
    pub drag_start_pos: Option<Pos2>,
    pub resize_corner: Option<ResizeCorner>,

    // Laser pointer (glowing neon multi-stroke trail with idle decay)
    pub laser_strokes: Vec<Vec<Pos2>>,
    pub current_laser_stroke: Vec<Pos2>,
    pub laser_last_active: Instant,
    pub laser_is_drawing: bool,
    pub laser_points: Vec<LaserTrailPoint>,

    // Performance optimizations: Mesh caching & Spatial Index
    pub mesh_cache: HashMap<(String, u32), Mesh>,
    pub spatial_grid: SpatialGrid,
    pub spatial_grid_page_id: Option<String>,

    // Fase 3: Texture cache & Active notebook ID for asset lookup
    pub texture_cache: HashMap<(String, String), TextureHandle>,
    pub failed_texture_loads: HashSet<(String, String)>,
    pub active_notebook_id: Option<String>,

    // Fase 4: Object clipboard
    pub object_clipboard: Vec<DrawObject>,
    pub app_clipboard_timestamp: Option<Instant>,
    pub request_paste_image: Option<(String, Pos2)>,
    pub hovered_page_id: Option<String>,
    pub hovered_page_pos: Option<Pos2>,
    pub last_interacted_page_id: Option<String>,

    // Fase 8.2: Eraser mode & undo tracking per drag gesture
    pub eraser_mode: EraserMode,
    pub eraser_undo_pushed: bool,
    pub last_whole_erase_pos: Option<Pos2>,

    // Fase 8.3: Zoom Window state
    pub show_zoom_window: bool,
    pub zoom_target_rect: Rect,
    pub zoom_draft_stroke: Option<Vec<StrokePoint>>,
    pub zoom_wrote_in_advance_zone: bool,

    // Fase 8.5: Palm Rejection state
    pub palm_rejection_enabled: bool,

    // Task 1 & Settings: Pressure Sensitivity
    pub pressure_sensitivity_enabled: bool,
    pub pressure_min: f32,
    pub pressure_multiplier: f32,

    // Inline text editing
    pub editing_text_id: Option<String>,
    pub focus_text_edit: bool,
    #[allow(dead_code)]
    pub text_buffer: String,

    // Fase G7: Connector interactive creation & snapping state
    pub draft_connector_start: Option<crate::model::ConnectorEnd>,
    pub draft_connector_current: Option<crate::model::Point2D>,
    pub connector_snap_candidate:
        Option<(String, crate::model::ConnectionSite, crate::model::Point2D)>,

    // Fase G11: Grid & Ruler Snapping, Smart Alignment Guides
    pub snap_to_grid: bool,
    pub smart_guides: bool,
    pub grid_spacing: f32,
    pub active_alignment_guides: Vec<crate::geometry::AlignmentGuide>,
}

impl Default for CanvasState {
    fn default() -> Self {
        Self {
            current_tool: ActiveTool::Pen,
            stroke_color: Color32::from_rgb(25, 27, 32),
            stroke_width: 3.0,
            stroke_style: StrokeStyle::Solid,
            zoom: 1.0,
            pan_offset: Vec2::ZERO,
            smoothing_steps: 3,
            undo_stacks: HashMap::new(),
            redo_stacks: HashMap::new(),
            undo_page_order: Vec::new(),
            redo_page_order: Vec::new(),
            active_stroke_page_id: None,
            draft_stroke: None,
            draft_shape_start: None,
            draft_shape_current: None,
            last_stroke_move_time: None,
            recognized_draft_shape: None,
            snap_initial_shape: None,
            snap_flash_time: None,
            auto_shape_enabled: true,
            lasso_points: Vec::new(),
            selected_ids: Vec::new(),
            is_dragging_selection: false,
            drag_start_pos: None,
            resize_corner: None,
            laser_strokes: Vec::new(),
            current_laser_stroke: Vec::new(),
            laser_last_active: Instant::now(),
            laser_is_drawing: false,
            laser_points: Vec::new(),
            mesh_cache: HashMap::new(),
            spatial_grid: SpatialGrid::new(100.0),
            spatial_grid_page_id: None,
            texture_cache: HashMap::new(),
            failed_texture_loads: HashSet::new(),
            active_notebook_id: None,
            object_clipboard: Vec::new(),
            app_clipboard_timestamp: None,
            request_paste_image: None,
            hovered_page_id: None,
            hovered_page_pos: None,
            last_interacted_page_id: None,
            eraser_mode: EraserMode::Partial,
            eraser_undo_pushed: false,
            last_whole_erase_pos: None,
            show_zoom_window: false,
            zoom_target_rect: Rect::from_min_size(Pos2::new(40.0, 90.0), Vec2::new(220.0, 80.0)),
            zoom_draft_stroke: None,
            zoom_wrote_in_advance_zone: false,
            palm_rejection_enabled: true,
            pressure_sensitivity_enabled: true,
            pressure_min: 0.2,
            pressure_multiplier: 1.0,
            editing_text_id: None,
            focus_text_edit: false,
            text_buffer: String::new(),
            draft_connector_start: None,
            draft_connector_current: None,
            connector_snap_candidate: None,
            snap_to_grid: false,
            smart_guides: true,
            grid_spacing: 30.0,
            active_alignment_guides: Vec::new(),
        }
    }
}

impl CanvasState {
    pub const MAX_UNDO_DEPTH: usize = 200;

    pub fn effective_pressure(&self, raw: f32) -> f32 {
        if !self.pressure_sensitivity_enabled {
            return 1.0;
        }
        (self.pressure_min + (1.0 - self.pressure_min) * (raw * self.pressure_multiplier))
            .clamp(0.05, 1.0)
    }

    pub fn get_or_load_texture(
        &mut self,
        ctx: &egui::Context,
        asset_id: &str,
    ) -> Option<&TextureHandle> {
        let nb_id = self.active_notebook_id.clone()?;
        self.get_or_load_texture_for_notebook(ctx, &nb_id, asset_id)
    }

    pub fn get_or_load_texture_for_notebook(
        &mut self,
        ctx: &egui::Context,
        notebook_id: &str,
        asset_id: &str,
    ) -> Option<&TextureHandle> {
        let key = (notebook_id.to_string(), asset_id.to_string());
        if self.failed_texture_loads.contains(&key) {
            return None;
        }

        if !self.texture_cache.contains_key(&key) {
            // Cap maximum textures in memory (200 handles) to prevent GPU memory leak
            const MAX_TEXTURE_CACHE_SIZE: usize = 200;
            if self.texture_cache.len() >= MAX_TEXTURE_CACHE_SIZE {
                let active = self.active_notebook_id.as_deref().unwrap_or("");
                let keys_to_remove: Vec<_> = self
                    .texture_cache
                    .keys()
                    .filter(|(nb, _)| nb != active)
                    .take(20)
                    .cloned()
                    .collect();
                for k in keys_to_remove {
                    self.texture_cache.remove(&k);
                }
                if self.texture_cache.len() >= MAX_TEXTURE_CACHE_SIZE {
                    let any_keys: Vec<_> = self.texture_cache.keys().take(10).cloned().collect();
                    for k in any_keys {
                        self.texture_cache.remove(&k);
                    }
                }
            }

            match crate::storage::load_asset_from_notebook(
                &crate::storage::app_storage_dir(),
                notebook_id,
                asset_id,
            ) {
                Ok(bytes) => match image::load_from_memory(&bytes) {
                    Ok(dyn_img) => {
                        let rgba = dyn_img.to_rgba8();
                        let size = [rgba.width() as usize, rgba.height() as usize];
                        let color_image =
                            egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
                        let handle = ctx.load_texture(
                            format!("{notebook_id}:{asset_id}"),
                            color_image,
                            Default::default(),
                        );
                        self.texture_cache.insert(key.clone(), handle);
                    }
                    Err(_) => {
                        self.failed_texture_loads.insert(key.clone());
                    }
                },
                Err(_) => {
                    self.failed_texture_loads.insert(key.clone());
                }
            }
        }
        self.texture_cache.get(&key)
    }

    pub fn get_texture(&self, asset_id: &str) -> Option<&TextureHandle> {
        let nb_id = self.active_notebook_id.as_deref()?;
        self.get_texture_for_notebook(nb_id, asset_id)
    }

    pub fn get_texture_for_notebook(
        &self,
        notebook_id: &str,
        asset_id: &str,
    ) -> Option<&TextureHandle> {
        let key = (notebook_id.to_string(), asset_id.to_string());
        self.texture_cache.get(&key)
    }

    pub fn invalidate_mesh_for_selection(&mut self) {
        if self.selected_ids.is_empty() {
            return;
        }
        self.mesh_cache
            .retain(|(k, _), _| !self.selected_ids.contains(k));
    }

    pub fn invalidate_mesh_for_id(&mut self, id: &str) {
        self.mesh_cache.retain(|(k, _), _| k != id);
    }

    // Fase 4: Clipboard & Selection helpers
    pub fn copy_selected(&mut self, objects: &[DrawObject]) {
        self.object_clipboard = objects
            .iter()
            .filter(|o| self.selected_ids.contains(&o.id().to_string()))
            .cloned()
            .collect();
        if !self.object_clipboard.is_empty() {
            self.app_clipboard_timestamp = Some(Instant::now());
        }
    }

    pub fn cut_selected(&mut self, page: &mut Page) {
        self.copy_selected(&page.objects);
        self.delete_selected(page);
    }

    pub fn delete_selected(&mut self, page: &mut Page) {
        if self.selected_ids.is_empty() {
            return;
        }
        let ids_to_delete: Vec<String> = page
            .objects
            .iter()
            .filter(|o| self.selected_ids.contains(&o.id().to_string()) && !o.is_locked())
            .map(|o| o.id().to_string())
            .collect();
        if ids_to_delete.is_empty() {
            return;
        }
        self.push_undo(&page.id, &page.objects);
        // Lepaskan connector yang terikat pada bentuk yang akan dihapus (Asumsi A5 / Task 7.5)
        crate::geometry::detach_connectors_from_shapes(&mut page.objects, &ids_to_delete);
        for id in &ids_to_delete {
            self.mesh_cache.retain(|(k, _), _| k != id);
        }
        page.objects
            .retain(|o| !ids_to_delete.contains(&o.id().to_string()));
        self.selected_ids.retain(|id| !ids_to_delete.contains(id));
        self.spatial_grid.rebuild(&page.objects);
    }

    pub fn duplicate_selected(&mut self, page: &mut Page) {
        if self.selected_ids.is_empty() {
            return;
        }
        self.push_undo(&page.id, &page.objects);
        let mut new_ids = Vec::new();
        let mut duplicated = Vec::new();
        let mut id_map = std::collections::HashMap::new();

        for obj in &page.objects {
            if self.selected_ids.contains(&obj.id().to_string()) {
                let mut clone = obj.clone();
                let old_id = obj.id().to_string();
                let new_id = Uuid::new_v4().to_string();
                id_map.insert(old_id, new_id.clone());
                crate::geometry::translate_object(&mut clone, 20.0, 20.0);
                match &mut clone {
                    DrawObject::Stroke { id, .. }
                    | DrawObject::Shape { id, .. }
                    | DrawObject::Text { id, .. }
                    | DrawObject::StickyNote { id, .. }
                    | DrawObject::Image { id, .. }
                    | DrawObject::Connector { id, .. } => *id = new_id.clone(),
                }
                new_ids.push(new_id);
                duplicated.push(clone);
            }
        }

        // Remap connector endpoints (Task 7.5)
        crate::geometry::remap_connector_endpoints_on_copy(&mut duplicated, &id_map, &page.objects);

        page.objects.extend(duplicated);
        self.selected_ids = new_ids;
        self.spatial_grid.rebuild(&page.objects);
    }

    pub fn paste_clipboard(&mut self, page: &mut Page, target_pos: Option<Pos2>) {
        if self.object_clipboard.is_empty() {
            return;
        }
        self.push_undo(&page.id, &page.objects);
        let mut new_ids = Vec::new();
        let mut pasted = self.object_clipboard.clone();
        let mut id_map = std::collections::HashMap::new();

        let offset = if let Some(target) = target_pos {
            let b = combined_bounds(&pasted.iter().collect::<Vec<_>>()).unwrap_or(Rect::NOTHING);
            target - b.center()
        } else {
            Vec2::new(20.0, 20.0)
        };

        for obj in &mut pasted {
            let old_id = obj.id().to_string();
            let new_id = Uuid::new_v4().to_string();
            id_map.insert(old_id, new_id.clone());
            crate::geometry::translate_object(obj, offset.x, offset.y);
            match obj {
                DrawObject::Stroke { id, .. }
                | DrawObject::Shape { id, .. }
                | DrawObject::Text { id, .. }
                | DrawObject::StickyNote { id, .. }
                | DrawObject::Image { id, .. }
                | DrawObject::Connector { id, .. } => *id = new_id.clone(),
            }
            new_ids.push(new_id);
        }

        // Remap connector endpoints (Task 7.5)
        crate::geometry::remap_connector_endpoints_on_copy(
            &mut pasted,
            &id_map,
            &self.object_clipboard,
        );

        page.objects.extend(pasted);
        self.selected_ids = new_ids;
        self.spatial_grid.rebuild(&page.objects);
    }

    /// Fase 8.2: Menghapus atau memotong objek pada titik `page_p` sesuai `self.eraser_mode`.
    /// Mengembalikan `true` jika ada objek yang terpotong atau terhapus.
    pub fn erase_at(&mut self, page: &mut Page, page_p: Pos2, radius: f32) -> bool {
        if self.spatial_grid_page_id.as_deref() != Some(&page.id) {
            self.spatial_grid.rebuild(&page.objects);
            self.spatial_grid_page_id = Some(page.id.clone());
        }
        let candidate_indices = self.spatial_grid.query_point(page_p, radius);
        if candidate_indices.is_empty() {
            return false;
        }

        match self.eraser_mode {
            EraserMode::WholeStroke => {
                // Cegah penghapusan beruntun di titik persimpangan/sambungan yang sama akibat gerakan mikro (1-2px)
                if let Some(prev_pos) = self.last_whole_erase_pos
                    && (page_p - prev_pos).length() < 9.0
                {
                    return false;
                }

                // Gunakan toleransi presisi ketat dan pilih HANYA 1 stroke terdekat di bawah ujung penghapus
                let precision_tol = radius.min(4.5);
                let mut best_match: Option<(usize, f32)> = None;

                for &idx in &candidate_indices {
                    if idx < page.objects.len()
                        && !page.objects[idx].is_locked()
                        && let Some(dist) = distance_to_object(&page.objects[idx], page_p)
                        && dist <= precision_tol
                    {
                        match best_match {
                            None => best_match = Some((idx, dist)),
                            Some((best_idx, best_dist)) => {
                                if dist + 0.25 < best_dist
                                    || ((dist - best_dist).abs() <= 0.25 && idx > best_idx)
                                {
                                    best_match = Some((idx, dist));
                                }
                            }
                        }
                    }
                }

                if let Some((target_idx, _)) = best_match {
                    if !self.eraser_undo_pushed {
                        self.push_undo(&page.id, &page.objects);
                        self.eraser_undo_pushed = true;
                    }
                    let rem_id = page.objects[target_idx].id().to_string();
                    crate::geometry::detach_connectors_from_shapes(
                        &mut page.objects,
                        std::slice::from_ref(&rem_id),
                    );
                    self.mesh_cache.retain(|(k, _), _| k != &rem_id);
                    page.objects.remove(target_idx);
                    self.spatial_grid.rebuild(&page.objects);
                    self.last_whole_erase_pos = Some(page_p);
                    return true;
                }
                false
            }
            EraserMode::Partial => {
                let mut modifications: HashMap<usize, Option<Vec<Vec<StrokePoint>>>> =
                    HashMap::new();

                for &idx in &candidate_indices {
                    if idx >= page.objects.len() {
                        continue;
                    }
                    match &page.objects[idx] {
                        DrawObject::Stroke { points, width, .. } => {
                            let eff_radius = radius + width * 0.5;
                            if let Some(segments) =
                                split_stroke_by_eraser(points, page_p, eff_radius)
                            {
                                modifications.insert(idx, Some(segments));
                            }
                        }
                        other => {
                            if !other.is_locked() && hit_test_object(other, page_p, radius) {
                                modifications.insert(idx, None);
                            }
                        }
                    }
                }

                if modifications.is_empty() {
                    return false;
                }

                if !self.eraser_undo_pushed {
                    self.push_undo(&page.id, &page.objects);
                    self.eraser_undo_pushed = true;
                }

                let mut deleted_shape_ids = Vec::new();
                let mut new_objects = Vec::with_capacity(page.objects.len() + modifications.len());
                for (idx, obj) in page.objects.drain(..).enumerate() {
                    if let Some(mod_entry) = modifications.remove(&idx) {
                        let old_id = obj.id().to_string();
                        self.mesh_cache.retain(|(k, _), _| k != &old_id);
                        if let DrawObject::Shape { id, .. } = &obj {
                            deleted_shape_ids.push(id.clone());
                        }

                        if let (
                            Some(segments),
                            DrawObject::Stroke {
                                color,
                                width,
                                is_highlighter,
                                style,
                                ..
                            },
                        ) = (mod_entry, obj)
                        {
                            for seg in segments {
                                new_objects.push(DrawObject::Stroke {
                                    id: Uuid::new_v4().to_string(),
                                    points: seg,
                                    color: color.clone(),
                                    width,
                                    is_highlighter,
                                    style,
                                });
                            }
                        }
                    } else {
                        new_objects.push(obj);
                    }
                }

                if !deleted_shape_ids.is_empty() {
                    crate::geometry::detach_connectors_from_shapes(
                        &mut new_objects,
                        &deleted_shape_ids,
                    );
                }

                page.objects = new_objects;
                self.spatial_grid.rebuild(&page.objects);
                true
            }
        }
    }

    pub fn advance_zoom_target(&mut self, page_width: f32, page_height: f32) {
        let w = self.zoom_target_rect.width();
        let h = self.zoom_target_rect.height();
        let step_x = w * 0.65;
        let left_margin = 30.0_f32.min((page_width - w).max(0.0));
        let max_x = (page_width - w - 15.0).max(left_margin);

        let mut new_min = self.zoom_target_rect.min;
        if new_min.x + step_x <= max_x {
            new_min.x += step_x;
        } else {
            new_min.x = left_margin;
            new_min.y = (new_min.y + h * 0.80).min((page_height - h - 15.0).max(0.0));
        }
        self.zoom_target_rect = Rect::from_min_size(new_min, Vec2::new(w, h));
    }

    pub fn newline_zoom_target(&mut self, page_width: f32, page_height: f32) {
        let w = self.zoom_target_rect.width();
        let h = self.zoom_target_rect.height();
        let left_margin = 30.0_f32.min((page_width - w).max(0.0));
        let new_y = (self.zoom_target_rect.min.y + h * 0.80).min((page_height - h - 15.0).max(0.0));
        self.zoom_target_rect = Rect::from_min_size(Pos2::new(left_margin, new_y), Vec2::new(w, h));
    }

    pub fn is_selected_locked(&self, page: &Page) -> bool {
        if self.selected_ids.is_empty() {
            return false;
        }
        page.objects
            .iter()
            .any(|o| self.selected_ids.contains(&o.id().to_string()) && o.is_locked())
    }

    pub fn toggle_lock_selected(&mut self, page: &mut Page) -> bool {
        if self.selected_ids.is_empty() {
            return false;
        }
        let lockable_count = page
            .objects
            .iter()
            .filter(|o| {
                self.selected_ids.contains(&o.id().to_string())
                    && matches!(o, DrawObject::Shape { .. } | DrawObject::Connector { .. })
            })
            .count();
        if lockable_count == 0 {
            return false;
        }

        let all_locked = page.objects.iter().all(|o| {
            if self.selected_ids.contains(&o.id().to_string())
                && matches!(o, DrawObject::Shape { .. } | DrawObject::Connector { .. })
            {
                o.is_locked()
            } else {
                true
            }
        });

        self.push_undo(&page.id, &page.objects);
        let new_locked = !all_locked;
        for obj in &mut page.objects {
            if self.selected_ids.contains(&obj.id().to_string()) {
                obj.set_locked(new_locked);
            }
        }
        true
    }

    pub fn flip_selected_horizontal(&mut self, page: &mut Page) {
        let selected: Vec<&DrawObject> = page
            .objects
            .iter()
            .filter(|o| self.selected_ids.contains(&o.id().to_string()) && !o.is_locked())
            .collect();
        if let Some(b) = combined_bounds(&selected) {
            self.push_undo(&page.id, &page.objects);
            let center_x = b.center().x;
            for obj in &mut page.objects {
                if self.selected_ids.contains(&obj.id().to_string()) && !obj.is_locked() {
                    crate::geometry::flip_object_horizontal(obj, center_x);
                    let id = obj.id().to_string();
                    self.mesh_cache.retain(|(k, _), _| k != &id);
                }
            }
            self.spatial_grid.rebuild(&page.objects);
        }
    }

    pub fn flip_selected_vertical(&mut self, page: &mut Page) {
        let selected: Vec<&DrawObject> = page
            .objects
            .iter()
            .filter(|o| self.selected_ids.contains(&o.id().to_string()) && !o.is_locked())
            .collect();
        if let Some(b) = combined_bounds(&selected) {
            self.push_undo(&page.id, &page.objects);
            let center_y = b.center().y;
            for obj in &mut page.objects {
                if self.selected_ids.contains(&obj.id().to_string()) && !obj.is_locked() {
                    crate::geometry::flip_object_vertical(obj, center_y);
                    let id = obj.id().to_string();
                    self.mesh_cache.retain(|(k, _), _| k != &id);
                }
            }
            self.spatial_grid.rebuild(&page.objects);
        }
    }

    pub fn bring_selected_to_front(&mut self, page: &mut Page) {
        if self.selected_ids.is_empty() {
            return;
        }
        self.push_undo(&page.id, &page.objects);
        let mut selected = Vec::new();
        let mut unselected = Vec::new();
        for obj in page.objects.drain(..) {
            if self.selected_ids.contains(&obj.id().to_string()) {
                selected.push(obj);
            } else {
                unselected.push(obj);
            }
        }
        unselected.extend(selected);
        page.objects = unselected;
    }

    pub fn send_selected_to_back(&mut self, page: &mut Page) {
        if self.selected_ids.is_empty() {
            return;
        }
        self.push_undo(&page.id, &page.objects);
        let mut selected = Vec::new();
        let mut unselected = Vec::new();
        for obj in page.objects.drain(..) {
            if self.selected_ids.contains(&obj.id().to_string()) {
                selected.push(obj);
            } else {
                unselected.push(obj);
            }
        }
        selected.extend(unselected);
        page.objects = selected;
    }

    pub fn push_undo(&mut self, page_id: &str, objects: &[DrawObject]) {
        let stack = self.undo_stacks.entry(page_id.to_string()).or_default();
        if let Some(top) = stack.last()
            && top.as_slice() == objects
        {
            return;
        }
        let snapshot = Arc::new(objects.to_vec());
        stack.push(snapshot);
        if stack.len() > Self::MAX_UNDO_DEPTH {
            stack.remove(0);
        }
        self.undo_page_order.push(page_id.to_string());
        if self.undo_page_order.len() > Self::MAX_UNDO_DEPTH * 2 {
            self.undo_page_order.remove(0);
        }
        self.redo_stacks
            .entry(page_id.to_string())
            .or_default()
            .clear();
        self.redo_page_order.retain(|id| id != page_id);
    }

    pub fn clean_redundant_top_undo(&mut self, page_id: &str, objects: &[DrawObject]) {
        if let Some(stack) = self.undo_stacks.get_mut(page_id)
            && let Some(top) = stack.last()
            && top.as_slice() == objects
        {
            stack.pop();
            if let Some(pos) = self.undo_page_order.iter().rposition(|id| id == page_id) {
                self.undo_page_order.remove(pos);
            }
        }
    }

    pub fn last_undo_page_id(&self) -> Option<&str> {
        for id in self.undo_page_order.iter().rev() {
            if self.can_undo(id) {
                return Some(id.as_str());
            }
        }
        None
    }

    pub fn last_redo_page_id(&self) -> Option<&str> {
        for id in self.redo_page_order.iter().rev() {
            if self.can_redo(id) {
                return Some(id.as_str());
            }
        }
        None
    }

    pub fn can_undo(&self, page_id: &str) -> bool {
        self.undo_stacks.get(page_id).is_some_and(|s| !s.is_empty())
    }

    pub fn can_redo(&self, page_id: &str) -> bool {
        self.redo_stacks.get(page_id).is_some_and(|s| !s.is_empty())
    }

    pub fn undo(&mut self, page: &mut Page) {
        if let Some(stack) = self.undo_stacks.get_mut(&page.id)
            && let Some(previous) = stack.pop()
        {
            let redo_stack = self.redo_stacks.entry(page.id.clone()).or_default();
            let current_snapshot = Arc::new(page.objects.clone());
            redo_stack.push(current_snapshot);
            if redo_stack.len() > Self::MAX_UNDO_DEPTH {
                redo_stack.remove(0);
            }
            if let Some(pos) = self.undo_page_order.iter().rposition(|id| id == &page.id) {
                self.undo_page_order.remove(pos);
            }
            self.redo_page_order.push(page.id.clone());
            if self.redo_page_order.len() > Self::MAX_UNDO_DEPTH * 2 {
                self.redo_page_order.remove(0);
            }

            page.objects = Arc::unwrap_or_clone(previous);
            self.selected_ids.clear();
            self.mesh_cache.clear();
            self.spatial_grid.rebuild(&page.objects);
        }
    }

    pub fn redo(&mut self, page: &mut Page) {
        if let Some(stack) = self.redo_stacks.get_mut(&page.id)
            && let Some(next) = stack.pop()
        {
            let undo_stack = self.undo_stacks.entry(page.id.clone()).or_default();
            let current_snapshot = Arc::new(page.objects.clone());
            undo_stack.push(current_snapshot);
            if undo_stack.len() > Self::MAX_UNDO_DEPTH {
                undo_stack.remove(0);
            }
            if let Some(pos) = self.redo_page_order.iter().rposition(|id| id == &page.id) {
                self.redo_page_order.remove(pos);
            }
            self.undo_page_order.push(page.id.clone());
            if self.undo_page_order.len() > Self::MAX_UNDO_DEPTH * 2 {
                self.undo_page_order.remove(0);
            }

            page.objects = Arc::unwrap_or_clone(next);
            self.selected_ids.clear();
            self.mesh_cache.clear();
            self.spatial_grid.rebuild(&page.objects);
        }
    }
}

use crate::geometry::{
    SpatialGrid, combined_bounds, distance_to_object, hit_test_object, object_bounds,
    object_in_lasso, recognize_handwriting_from_strokes, recognize_shape, scale_object,
    should_reject_palm_input, smooth_stroke_points, split_stroke_by_eraser, tessellate_stroke_ex,
    translate_object,
};
use crate::model::{DrawObject, Page, PageTemplate, ShapeKind, StrokePoint};
use eframe::egui::{
    self, Color32, Key, Mesh, Painter, PointerButton, Pos2, Rect, Response, Sense, Stroke, Ui, Vec2,
};
use std::time::Instant;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ActiveTool {
    Pen,
    Pencil,
    Highlighter,
    Eraser,
    Lasso,
    Shape(ShapeKind),
    Text,
    StickyNote,
    Laser,
    Hand,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EraserMode {
    Partial,
    WholeStroke,
}

#[derive(Clone, Debug)]
pub struct LaserTrailPoint {
    pub pos: Pos2,
    pub time: Instant,
}

pub struct CanvasState {
    pub current_tool: ActiveTool,
    pub stroke_color: Color32,
    pub stroke_width: f32,
    pub zoom: f32,
    pub pan_offset: Vec2,
    pub smoothing_steps: usize,

    // Undo / Redo stacks: page_id -> Vec<Vec<Arc<DrawObject>>> (Structural sharing)
    pub undo_stacks: std::collections::HashMap<String, Vec<Vec<std::sync::Arc<DrawObject>>>>,
    pub redo_stacks: std::collections::HashMap<String, Vec<Vec<std::sync::Arc<DrawObject>>>>,
    pub undo_page_order: Vec<String>,
    pub redo_page_order: Vec<String>,

    // Active stroke / shape draft
    pub active_stroke_page_id: Option<String>,
    pub draft_stroke: Option<Vec<StrokePoint>>,
    pub draft_shape_start: Option<Pos2>,
    pub draft_shape_current: Option<Pos2>,
    pub last_stroke_move_time: Option<Instant>,
    pub recognized_draft_shape: Option<crate::geometry::RecognizedShape>,

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
    pub mesh_cache: std::collections::HashMap<(String, u32), Mesh>,
    pub spatial_grid: SpatialGrid,
    pub spatial_grid_page_id: Option<String>,

    // Fase 3: Texture cache & Active notebook ID for asset lookup
    pub texture_cache: std::collections::HashMap<String, egui::TextureHandle>,
    pub active_notebook_id: Option<String>,

    // Fase 4: Object clipboard
    pub object_clipboard: Vec<DrawObject>,
    pub app_clipboard_timestamp: Option<Instant>,
    pub request_paste_image: Option<Pos2>,

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

    // Inline text editing
    pub editing_text_id: Option<String>,
    pub focus_text_edit: bool,
    #[allow(dead_code)]
    pub text_buffer: String,
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[allow(dead_code)]
pub enum ResizeCorner {
    NorthWest,
    NorthEast,
    SouthEast,
    SouthWest,
    Rotation,
    LineStart,
    LineEnd,
}

impl Default for CanvasState {
    fn default() -> Self {
        Self {
            current_tool: ActiveTool::Pen,
            stroke_color: Color32::from_rgb(25, 27, 32),
            stroke_width: 3.0,
            zoom: 1.0,
            pan_offset: Vec2::ZERO,
            smoothing_steps: 3,
            undo_stacks: std::collections::HashMap::new(),
            redo_stacks: std::collections::HashMap::new(),
            undo_page_order: Vec::new(),
            redo_page_order: Vec::new(),
            active_stroke_page_id: None,
            draft_stroke: None,
            draft_shape_start: None,
            draft_shape_current: None,
            last_stroke_move_time: None,
            recognized_draft_shape: None,
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
            mesh_cache: std::collections::HashMap::new(),
            spatial_grid: SpatialGrid::new(100.0),
            spatial_grid_page_id: None,
            texture_cache: std::collections::HashMap::new(),
            active_notebook_id: None,
            object_clipboard: Vec::new(),
            app_clipboard_timestamp: None,
            request_paste_image: None,
            eraser_mode: EraserMode::Partial,
            eraser_undo_pushed: false,
            last_whole_erase_pos: None,
            show_zoom_window: false,
            zoom_target_rect: Rect::from_min_size(Pos2::new(40.0, 90.0), Vec2::new(220.0, 80.0)),
            zoom_draft_stroke: None,
            zoom_wrote_in_advance_zone: false,
            palm_rejection_enabled: true,
            editing_text_id: None,
            focus_text_edit: false,
            text_buffer: String::new(),
        }
    }
}

impl CanvasState {
    pub const MAX_UNDO_DEPTH: usize = 200;

    pub fn get_or_load_texture(
        &mut self,
        ctx: &egui::Context,
        asset_id: &str,
    ) -> Option<&egui::TextureHandle> {
        if !self.texture_cache.contains_key(asset_id)
            && let Some(nb_id) = &self.active_notebook_id
            && let Ok(bytes) = crate::storage::load_asset_from_notebook(
                &crate::storage::app_storage_dir(),
                nb_id,
                asset_id,
            )
            && let Ok(dyn_img) = image::load_from_memory(&bytes)
        {
            let rgba = dyn_img.to_rgba8();
            let size = [rgba.width() as usize, rgba.height() as usize];
            let color_image = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
            let handle = ctx.load_texture(asset_id, color_image, Default::default());
            self.texture_cache.insert(asset_id.to_string(), handle);
        }
        self.texture_cache.get(asset_id)
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
        self.push_undo(&page.id, &page.objects);
        for id in &self.selected_ids {
            self.mesh_cache.retain(|(k, _), _| k != id);
        }
        page.objects
            .retain(|o| !self.selected_ids.contains(&o.id().to_string()));
        self.selected_ids.clear();
        self.spatial_grid.rebuild(&page.objects);
    }

    pub fn duplicate_selected(&mut self, page: &mut Page) {
        if self.selected_ids.is_empty() {
            return;
        }
        self.push_undo(&page.id, &page.objects);
        let mut new_ids = Vec::new();
        let mut duplicated = Vec::new();
        for obj in &page.objects {
            if self.selected_ids.contains(&obj.id().to_string()) {
                let mut clone = obj.clone();
                let new_id = uuid::Uuid::new_v4().to_string();
                crate::geometry::translate_object(&mut clone, 20.0, 20.0);
                match &mut clone {
                    DrawObject::Stroke { id, .. }
                    | DrawObject::Shape { id, .. }
                    | DrawObject::Text { id, .. }
                    | DrawObject::StickyNote { id, .. }
                    | DrawObject::Image { id, .. } => *id = new_id.clone(),
                }
                new_ids.push(new_id);
                duplicated.push(clone);
            }
        }
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

        let offset = if let Some(target) = target_pos {
            let b = combined_bounds(&pasted.iter().collect::<Vec<_>>()).unwrap_or(Rect::NOTHING);
            target - b.center()
        } else {
            Vec2::new(20.0, 20.0)
        };

        for obj in &mut pasted {
            let new_id = uuid::Uuid::new_v4().to_string();
            crate::geometry::translate_object(obj, offset.x, offset.y);
            match obj {
                DrawObject::Stroke { id, .. }
                | DrawObject::Shape { id, .. }
                | DrawObject::Text { id, .. }
                | DrawObject::StickyNote { id, .. }
                | DrawObject::Image { id, .. } => *id = new_id.clone(),
            }
            new_ids.push(new_id);
        }

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
                        && let Some(dist) = distance_to_object(&page.objects[idx], page_p)
                        && dist <= precision_tol
                    {
                        match best_match {
                            None => best_match = Some((idx, dist)),
                            Some((best_idx, best_dist)) => {
                                // Pilih objek yang jaraknya paling dekat; jika seri (< 0.25pt), pilih stroke teratas (terbaru)
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
                    self.mesh_cache.retain(|(k, _), _| k != &rem_id);
                    page.objects.remove(target_idx);
                    self.spatial_grid.rebuild(&page.objects);
                    self.last_whole_erase_pos = Some(page_p);
                    return true;
                }
                false
            }
            EraserMode::Partial => {
                // Peta index objek -> Option<Vec<Vec<StrokePoint>>>
                // (Some(segs) untuk Stroke yang terpotong, None untuk objek non-Stroke yang harus dihapus)
                let mut modifications: std::collections::HashMap<
                    usize,
                    Option<Vec<Vec<StrokePoint>>>,
                > = std::collections::HashMap::new();

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
                            if hit_test_object(other, page_p, radius) {
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

                let mut new_objects = Vec::with_capacity(page.objects.len() + modifications.len());
                for (idx, obj) in page.objects.drain(..).enumerate() {
                    if let Some(mod_entry) = modifications.remove(&idx) {
                        let old_id = obj.id().to_string();
                        self.mesh_cache.retain(|(k, _), _| k != &old_id);

                        if let (
                            Some(segments),
                            DrawObject::Stroke {
                                color,
                                width,
                                is_highlighter,
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
                                });
                            }
                        }
                    } else {
                        new_objects.push(obj);
                    }
                }

                page.objects = new_objects;
                self.spatial_grid.rebuild(&page.objects);
                true
            }
        }
    }

    /// Fase 8.3: Geser kotak target Zoom Window ke kanan (auto-advance),
    /// atau pindah ke baris baru di sisi kiri jika sudah mendekati batas kanan halaman.
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

    /// Fase 8.3: Pindah kotak target Zoom Window ke awal baris berikutnya.
    pub fn newline_zoom_target(&mut self, page_width: f32, page_height: f32) {
        let w = self.zoom_target_rect.width();
        let h = self.zoom_target_rect.height();
        let left_margin = 30.0_f32.min((page_width - w).max(0.0));
        let new_y = (self.zoom_target_rect.min.y + h * 0.80).min((page_height - h - 15.0).max(0.0));
        self.zoom_target_rect = Rect::from_min_size(Pos2::new(left_margin, new_y), Vec2::new(w, h));
    }

    pub fn flip_selected_horizontal(&mut self, page: &mut Page) {
        let selected: Vec<&DrawObject> = page
            .objects
            .iter()
            .filter(|o| self.selected_ids.contains(&o.id().to_string()))
            .collect();
        if let Some(b) = combined_bounds(&selected) {
            self.push_undo(&page.id, &page.objects);
            let center_x = b.center().x;
            for obj in &mut page.objects {
                if self.selected_ids.contains(&obj.id().to_string()) {
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
            .filter(|o| self.selected_ids.contains(&o.id().to_string()))
            .collect();
        if let Some(b) = combined_bounds(&selected) {
            self.push_undo(&page.id, &page.objects);
            let center_y = b.center().y;
            for obj in &mut page.objects {
                if self.selected_ids.contains(&obj.id().to_string()) {
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
        // Cegah penambahan snapshot identik berturut-turut tanpa perubahan objek
        if let Some(top) = stack.last()
            && top.len() == objects.len()
            && top.iter().zip(objects.iter()).all(|(a, b)| **a == *b)
        {
            return;
        }
        let arc_list: Vec<std::sync::Arc<DrawObject>> =
            objects.iter().cloned().map(std::sync::Arc::new).collect();
        stack.push(arc_list);
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
            && top.len() == objects.len()
            && top.iter().zip(objects.iter()).all(|(a, b)| **a == *b)
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
            let current_arcs: Vec<std::sync::Arc<DrawObject>> = page
                .objects
                .iter()
                .cloned()
                .map(std::sync::Arc::new)
                .collect();
            redo_stack.push(current_arcs);
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

            page.objects = previous
                .into_iter()
                .map(std::sync::Arc::unwrap_or_clone)
                .collect();
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
            let current_arcs: Vec<std::sync::Arc<DrawObject>> = page
                .objects
                .iter()
                .cloned()
                .map(std::sync::Arc::new)
                .collect();
            undo_stack.push(current_arcs);
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

            page.objects = next
                .into_iter()
                .map(std::sync::Arc::unwrap_or_clone)
                .collect();
            self.selected_ids.clear();
            self.mesh_cache.clear();
            self.spatial_grid.rebuild(&page.objects);
        }
    }
}

/// Transformasikan `Mesh` goresan yang di-cache dalam koordinat lokal halaman (`0..width`, `0..height`)
/// ke koordinat layar terkini berdasarkan `origin` kertas (`rect.min`) dan faktor `zoom`.
/// Dengan cara ini, goresan selalu menempel presisi di atas kertas saat di-scroll maupun di-zoom.
pub fn transform_page_mesh_to_screen(page_mesh: &Mesh, origin: Pos2, zoom: f32) -> Mesh {
    let mut screen_mesh = page_mesh.clone();
    for v in &mut screen_mesh.vertices {
        v.pos = Pos2::new(origin.x + v.pos.x * zoom, origin.y + v.pos.y * zoom);
    }
    screen_mesh
}

pub fn render_page_canvas(ui: &mut Ui, page: &mut Page, state: &mut CanvasState) -> Response {
    let desired_size = Vec2::new(page.width * state.zoom, page.height * state.zoom);
    let (rect, response) = ui.allocate_exact_size(desired_size, Sense::click_and_drag());
    // Fase 8.9: Metadata Aksesibilitas (AccessKit) untuk kanvas halaman
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Other, true, "Kanvas Catatan RustNotes")
    });
    let painter = ui.painter_at(rect);

    let zoom = state.zoom;
    let origin = rect.min;
    let screen_clip_rect = ui.clip_rect();

    // Transform from screen coordinates to page coordinates
    let screen_to_page = move |screen_pos: Pos2| -> Pos2 {
        let rel = screen_pos - origin;
        Pos2::new(rel.x / zoom, rel.y / zoom)
    };

    let page_to_screen =
        move |page_pos: Pos2| -> Pos2 { origin + Vec2::new(page_pos.x * zoom, page_pos.y * zoom) };

    // Pre-cache textures for background and image objects
    if let crate::model::PageBackground::Image { asset_id }
    | crate::model::PageBackground::Pdf { asset_id, .. } = page.effective_background()
    {
        state.get_or_load_texture(ui.ctx(), &asset_id);
    }
    for obj in &page.objects {
        if let DrawObject::Image { asset_id, .. } = obj {
            state.get_or_load_texture(ui.ctx(), asset_id);
        }
    }

    // 1. Draw Paper Background and Patterns
    painter.rect_filled(rect, 4.0, page.paper_color.to_color32());

    match page.effective_background() {
        crate::model::PageBackground::Template(template) => {
            draw_paper_pattern(&painter, rect, template, state.zoom);
        }
        crate::model::PageBackground::Image { asset_id }
        | crate::model::PageBackground::Pdf { asset_id, .. } => {
            if let Some(tex) = state.texture_cache.get(&asset_id) {
                draw_rotated_background_image(&painter, tex.id(), rect, page.rotation);
            }
        }
    }

    // 2. Draw existing page objects with Viewport Culling & Page-Space Mesh Cache
    for obj in &page.objects {
        // Culling: skip objects entirely outside the visible viewport
        let obj_b = object_bounds(obj);
        let screen_b = Rect::from_min_max(page_to_screen(obj_b.min), page_to_screen(obj_b.max));
        if !screen_clip_rect.intersects(screen_b) {
            continue;
        }

        match obj {
            DrawObject::Stroke {
                id,
                points,
                color,
                width,
                is_highlighter,
            } => {
                // Simpan mesh di dalam cache menggunakan koordinat lokal kertas (`|p| p`, skala 1.0),
                // lalu transformasikan ke posisi layar (`origin` + `state.zoom`) setiap frame agar
                // goresan tidak pernah bergeser atau hilang ketika halaman di-scroll maupun di-zoom.
                let key = (id.clone(), 0);
                let page_mesh = if let Some(cached) = state.mesh_cache.get(&key) {
                    cached.clone()
                } else {
                    let smoothed = smooth_stroke_points(points, state.smoothing_steps);
                    let stroke_c = if *is_highlighter {
                        Color32::from_rgba_unmultiplied(color.r, color.g, color.b, 90)
                    } else {
                        color.to_color32()
                    };
                    let mesh =
                        tessellate_stroke_ex(&smoothed, *width, stroke_c, *is_highlighter, |p| p);
                    if state.mesh_cache.len() > 1500 {
                        state.mesh_cache.clear();
                    }
                    state.mesh_cache.insert(key, mesh.clone());
                    mesh
                };
                let screen_mesh = transform_page_mesh_to_screen(&page_mesh, origin, state.zoom);
                painter.add(screen_mesh);
            }
            _ => {
                // Jangan gambar teks statis jika objek sedang disunting secara inline oleh TextEdit
                if state.editing_text_id.as_deref() == Some(obj.id())
                    && matches!(obj, DrawObject::Text { .. })
                {
                    continue;
                }
                draw_non_stroke_object(
                    &painter,
                    obj,
                    page_to_screen,
                    state.zoom,
                    &state.texture_cache,
                );
            }
        }
    }

    // 3. User interaction logic
    let pointer_pos = response.interact_pointer_pos();

    // Laser decay update (idle-based persistence matching user reference)
    let now = Instant::now();
    let idle_threshold = 1.0_f32; // tahan 1.0 detik setelah berhenti menggambar
    let fade_duration = 0.7_f32; // fade out halus selama 0.7 detik

    let elapsed = now.duration_since(state.laser_last_active).as_secs_f32();
    let laser_alpha = if state.laser_is_drawing || elapsed < idle_threshold {
        1.0_f32 // jika sedang menggambar atau jeda waktu singkat: TETAP 100% PERSISTEN
    } else {
        let fade_progress = (elapsed - idle_threshold) / fade_duration;
        (1.0 - fade_progress).clamp(0.0, 1.0)
    };

    if laser_alpha <= 0.0 && !state.laser_is_drawing {
        state.laser_strokes.clear();
        state.current_laser_stroke.clear();
    } else if !state.laser_strokes.is_empty() || !state.current_laser_stroke.is_empty() {
        ui.ctx().request_repaint();
    }

    // Active Selection Bounds
    let selected_objects: Vec<&DrawObject> = page
        .objects
        .iter()
        .filter(|o| state.selected_ids.contains(&o.id().to_string()))
        .collect();

    let selection_rect = combined_bounds(&selected_objects);
    let selected_single_line: Option<(Pos2, Pos2)> = if selected_objects.len() == 1 {
        if let DrawObject::Shape {
            shape_type: ShapeKind::Line,
            x,
            y,
            width,
            height,
            ..
        } = selected_objects[0]
        {
            Some((Pos2::new(*x, *y), Pos2::new(*x + *width, *y + *height)))
        } else {
            None
        }
    } else {
        None
    };

    if response.drag_started_by(PointerButton::Primary)
        && let Some(pos) = pointer_pos
    {
        let page_p = screen_to_page(pos);

        // Check if clicking resize/rotation handles on selection
        if let Some((line_start, line_end)) = selected_single_line {
            let handle_dist = 14.0 / state.zoom;
            let dir = line_end - line_start;
            let dir_norm = if dir.length() > 1e-3 {
                dir / dir.length()
            } else {
                Vec2::new(1.0, 0.0)
            };
            let line_rot_handle = line_end + dir_norm * (22.0 / state.zoom);

            // Handle rotasi (⟳) di ujung titik akhir garis (berputar terhadap titik awal)
            if (page_p - line_rot_handle).length() <= handle_dist {
                state.resize_corner = Some(ResizeCorner::Rotation);
                state.drag_start_pos = Some(page_p);
                state.push_undo(&page.id, &page.objects);
                return response;
            }
            // Handle titik akhir garis (dapat berotasi & diatur terhadap titik awal sebagai pusat)
            if (page_p - line_end).length() <= handle_dist {
                state.resize_corner = Some(ResizeCorner::LineEnd);
                state.drag_start_pos = Some(page_p);
                state.push_undo(&page.id, &page.objects);
                return response;
            }
            // Handle titik awal garis (pusat rotasi)
            if (page_p - line_start).length() <= handle_dist {
                state.resize_corner = Some(ResizeCorner::LineStart);
                state.drag_start_pos = Some(page_p);
                state.push_undo(&page.id, &page.objects);
                return response;
            }
            // Klik pada badan garis untuk menggeser garis (hanya saat menggunakan alat Lasso)
            if state.current_tool == ActiveTool::Lasso
                && crate::geometry::point_to_segment_dist(page_p, line_start, line_end)
                    <= handle_dist
            {
                state.is_dragging_selection = true;
                state.drag_start_pos = Some(page_p);
                state.push_undo(&page.id, &page.objects);
                return response;
            }
        } else if state.current_tool == ActiveTool::Lasso
            && let Some(b) = selection_rect
        {
            let handle_dist = 14.0 / state.zoom;
            // Check rotation handle first (circle above selection center)
            let rot_handle = Pos2::new(b.center().x, b.top() - 18.0 / state.zoom);
            if (page_p - rot_handle).length() <= handle_dist {
                state.resize_corner = Some(ResizeCorner::Rotation);
                state.drag_start_pos = Some(page_p);
                state.push_undo(&page.id, &page.objects);
                return response;
            }
            if (page_p - b.right_bottom()).length() <= handle_dist {
                state.resize_corner = Some(ResizeCorner::SouthEast);
                state.drag_start_pos = Some(page_p);
                state.push_undo(&page.id, &page.objects);
                return response;
            }
            if (page_p - b.left_top()).length() <= handle_dist {
                state.resize_corner = Some(ResizeCorner::NorthWest);
                state.drag_start_pos = Some(page_p);
                state.push_undo(&page.id, &page.objects);
                return response;
            }
            if (page_p - Pos2::new(b.right(), b.top())).length() <= handle_dist {
                state.resize_corner = Some(ResizeCorner::NorthEast);
                state.drag_start_pos = Some(page_p);
                state.push_undo(&page.id, &page.objects);
                return response;
            }
            if (page_p - Pos2::new(b.left(), b.bottom())).length() <= handle_dist {
                state.resize_corner = Some(ResizeCorner::SouthWest);
                state.drag_start_pos = Some(page_p);
                state.push_undo(&page.id, &page.objects);
                return response;
            }

            // Check if clicking inside selection to move
            if b.contains(page_p) {
                state.is_dragging_selection = true;
                state.drag_start_pos = Some(page_p);
                state.push_undo(&page.id, &page.objects);
                return response;
            }
        }

        match state.current_tool {
            ActiveTool::Pen | ActiveTool::Pencil | ActiveTool::Highlighter => {
                // Fase 8.5: Palm Rejection — tolak awal goresan jika multi-touch terdeteksi
                let multi_touch = ui.input(|i| i.multi_touch().is_some());
                if !(state.palm_rejection_enabled && multi_touch) {
                    state.active_stroke_page_id = Some(page.id.clone());
                    state.draft_stroke = Some(vec![StrokePoint::from(page_p)]);
                    state.last_stroke_move_time = Some(Instant::now());
                    state.recognized_draft_shape = None;
                    state.selected_ids.clear();
                }
            }
            ActiveTool::Shape(_) => {
                state.active_stroke_page_id = Some(page.id.clone());
                state.draft_shape_start = Some(page_p);
                state.draft_shape_current = Some(page_p);
                state.selected_ids.clear();
            }
            ActiveTool::Eraser => {
                state.eraser_undo_pushed = false;
                state.last_whole_erase_pos = None;
                let erase_radius = match state.eraser_mode {
                    EraserMode::WholeStroke => (3.8 / state.zoom).clamp(1.8, 4.5),
                    EraserMode::Partial => (10.0 / state.zoom).clamp(4.0, 16.0),
                };
                state.erase_at(page, page_p, erase_radius);
            }
            ActiveTool::Lasso => {
                state.active_stroke_page_id = Some(page.id.clone());
                state.selected_ids.clear();
                state.lasso_points = vec![page_p];
            }
            ActiveTool::Laser => {
                state.laser_is_drawing = true;
                state.laser_last_active = Instant::now();
                state.current_laser_stroke = vec![pos];
                ui.ctx().request_repaint();
            }
            ActiveTool::Text => {
                // Jika klik/drag pada objek Teks atau StickyNote yang sudah ada, sunting objek tersebut
                let hit_id = page
                    .objects
                    .iter()
                    .rev()
                    .find(|obj| {
                        matches!(obj, DrawObject::Text { .. } | DrawObject::StickyNote { .. })
                            && hit_test_object(obj, page_p, 8.0 / state.zoom)
                    })
                    .map(|o| o.id().to_string());
                if let Some(id) = hit_id {
                    state.selected_ids = vec![id.clone()];
                    state.editing_text_id = Some(id);
                    state.focus_text_edit = true;
                } else {
                    state.push_undo(&page.id, &page.objects);
                    let id = Uuid::new_v4().to_string();
                    let new_text = DrawObject::Text {
                        id: id.clone(),
                        x: page_p.x,
                        y: page_p.y,
                        text: String::new(),
                        font_size: (state.stroke_width * 4.0 + 14.0).clamp(12.0, 72.0),
                        color: state.stroke_color.into(),
                    };
                    page.objects.push(new_text);
                    state.spatial_grid.rebuild(&page.objects);
                    state.selected_ids = vec![id.clone()];
                    state.editing_text_id = Some(id);
                    state.focus_text_edit = true;
                }
            }
            ActiveTool::StickyNote => {
                state.push_undo(&page.id, &page.objects);
                let id = Uuid::new_v4().to_string();
                let new_note = DrawObject::StickyNote {
                    id: id.clone(),
                    x: page_p.x,
                    y: page_p.y,
                    width: 180.0,
                    height: 140.0,
                    text: String::new(),
                    color: Color32::from_rgb(254, 243, 189).into(),
                };
                page.objects.push(new_note);
                state.spatial_grid.rebuild(&page.objects);
                state.selected_ids = vec![id.clone()];
                state.editing_text_id = Some(id);
                state.focus_text_edit = true;
                state.current_tool = ActiveTool::Lasso;
            }
            ActiveTool::Hand => {}
        }
    }

    if response.dragged_by(PointerButton::Primary)
        && let Some(pos) = pointer_pos
    {
        let page_p = screen_to_page(pos);

        // Handle moving selection
        if state.is_dragging_selection {
            if let Some(start) = state.drag_start_pos {
                let dx = page_p.x - start.x;
                let dy = page_p.y - start.y;
                for obj in &mut page.objects {
                    if state.selected_ids.contains(&obj.id().to_string()) {
                        translate_object(obj, dx, dy);
                        let id = obj.id().to_string();
                        state.mesh_cache.retain(|(k, _), _| k != &id);
                    }
                }
                state.drag_start_pos = Some(page_p);
            }
        } else if let Some(corner) = state.resize_corner {
            if let Some(start) = state.drag_start_pos
                && let Some(b) = selection_rect
            {
                if corner == ResizeCorner::LineEnd {
                    for obj in &mut page.objects {
                        if state.selected_ids.contains(&obj.id().to_string())
                            && let DrawObject::Shape {
                                shape_type: ShapeKind::Line,
                                x,
                                y,
                                width,
                                height,
                                ..
                            } = obj
                        {
                            // Titik awal (x, y) tetap sebagai pusat, titik akhir berotasi mengikuti kursor
                            *width = page_p.x - *x;
                            *height = page_p.y - *y;
                        }
                    }
                    state.drag_start_pos = Some(page_p);
                } else if corner == ResizeCorner::LineStart {
                    for obj in &mut page.objects {
                        if state.selected_ids.contains(&obj.id().to_string())
                            && let DrawObject::Shape {
                                shape_type: ShapeKind::Line,
                                x,
                                y,
                                width,
                                height,
                                ..
                            } = obj
                        {
                            let end_x = *x + *width;
                            let end_y = *y + *height;
                            *x = page_p.x;
                            *y = page_p.y;
                            *width = end_x - *x;
                            *height = end_y - *y;
                        }
                    }
                    state.drag_start_pos = Some(page_p);
                } else if corner == ResizeCorner::Rotation {
                    let pivot = if let Some((line_start, _)) = selected_single_line {
                        line_start
                    } else {
                        b.center()
                    };
                    let v_start = start - pivot;
                    let v_curr = page_p - pivot;
                    let angle = v_curr.y.atan2(v_curr.x) - v_start.y.atan2(v_start.x);
                    if angle.abs() > 0.001 {
                        for obj in &mut page.objects {
                            if state.selected_ids.contains(&obj.id().to_string()) {
                                crate::geometry::rotate_object(obj, pivot, angle);
                                let id = obj.id().to_string();
                                state.mesh_cache.retain(|(k, _), _| k != &id);
                            }
                        }
                        state.drag_start_pos = Some(page_p);
                    }
                } else {
                    let origin = match corner {
                        ResizeCorner::SouthEast => b.left_top(),
                        ResizeCorner::NorthWest => b.right_bottom(),
                        ResizeCorner::NorthEast => b.left_bottom(),
                        ResizeCorner::SouthWest => b.right_top(),
                        ResizeCorner::Rotation
                        | ResizeCorner::LineStart
                        | ResizeCorner::LineEnd => b.center(),
                    };
                    let orig_w = b.width().max(1.0);
                    let orig_h = b.height().max(1.0);
                    let mut sx = ((page_p.x - origin.x).abs() / orig_w).max(0.05);
                    let mut sy = ((page_p.y - origin.y).abs() / orig_h).max(0.05);
                    // Shift+drag: kunci rasio aspek untuk proportional resize
                    if ui.input(|i| i.modifiers.shift) {
                        let s = if (sx - 1.0).abs() > (sy - 1.0).abs() {
                            sx
                        } else {
                            sy
                        };
                        sx = s;
                        sy = s;
                    }
                    for obj in &mut page.objects {
                        if state.selected_ids.contains(&obj.id().to_string()) {
                            scale_object(obj, origin, sx, sy);
                            let id = obj.id().to_string();
                            state.mesh_cache.retain(|(k, _), _| k != &id);
                        }
                    }
                    state.drag_start_pos = Some(page_p);
                }
            }
        } else {
            match state.current_tool {
                ActiveTool::Pen | ActiveTool::Pencil | ActiveTool::Highlighter => {
                    if state.active_stroke_page_id.as_deref() == Some(&page.id) {
                        if let Some(recognized) = &mut state.recognized_draft_shape {
                            if recognized.kind == ShapeKind::Line {
                                // Sama seperti tools Shape -> Line: titik awal (recognized.x, recognized.y)
                                // menjadi pusat rotasi (pivot), dan titik akhirnya berotasi mengikuti kursor!
                                recognized.width = page_p.x - recognized.x;
                                recognized.height = page_p.y - recognized.y;
                                ui.ctx().request_repaint();
                            }
                        } else if let Some(pts) = &mut state.draft_stroke {
                            let dt_secs = state
                                .last_stroke_move_time
                                .map(|t| now.saturating_duration_since(t).as_secs_f32())
                                .unwrap_or(0.016);
                            let multi_touch = ui.input(|i| i.multi_touch().is_some());
                            // Fase 8.5: Palm Rejection — abaikan titik yang terindikasi sentuhan telapak tangan
                            let rejected = state.palm_rejection_enabled
                                && should_reject_palm_input(pts, page_p, dt_secs, multi_touch);
                            let min_dist = 2.0 / state.zoom;
                            if !rejected
                                && let Some(last) = pts.last()
                                && (page_p - last.to_pos2()).length() >= min_dist
                            {
                                pts.push(StrokePoint::from(page_p));
                                state.last_stroke_move_time = Some(Instant::now());
                            }
                        }
                    }
                }
                ActiveTool::Shape(_) => {
                    if state.active_stroke_page_id.as_deref() == Some(&page.id) {
                        state.draft_shape_current = Some(page_p);
                    }
                }
                ActiveTool::Eraser => {
                    let erase_radius = match state.eraser_mode {
                        EraserMode::WholeStroke => (3.8 / state.zoom).clamp(1.8, 4.5),
                        EraserMode::Partial => (10.0 / state.zoom).clamp(4.0, 16.0),
                    };
                    if state.erase_at(page, page_p, erase_radius) {
                        ui.ctx().request_repaint();
                    }
                }
                ActiveTool::Lasso => {
                    if state.active_stroke_page_id.as_deref() == Some(&page.id) {
                        state.lasso_points.push(page_p);
                    }
                }
                ActiveTool::Laser => {
                    state.laser_is_drawing = true;
                    state.laser_last_active = Instant::now();
                    let should_add = state
                        .current_laser_stroke
                        .last()
                        .is_none_or(|last_p| last_p.distance(pos) > 2.0);
                    if should_add {
                        state.current_laser_stroke.push(pos);
                    }
                    ui.ctx().request_repaint();
                }
                ActiveTool::Hand => {
                    state.pan_offset += response.drag_delta();
                }
                _ => {}
            }
        }
    }

    // Fase 8.1: Deteksi tahan pena (~500 ms) di akhir goresan untuk Shape Recognition
    if state.active_stroke_page_id.as_deref() == Some(&page.id)
        && matches!(
            state.current_tool,
            ActiveTool::Pen | ActiveTool::Pencil | ActiveTool::Highlighter
        )
        && state.recognized_draft_shape.is_none()
        && let Some(pts) = &state.draft_stroke
        && let Some(last_move) = state.last_stroke_move_time
    {
        let elapsed = now.saturating_duration_since(last_move);
        if elapsed >= std::time::Duration::from_millis(500) {
            if let Some(recognized) = recognize_shape(pts) {
                state.recognized_draft_shape = Some(recognized);
                ui.ctx().request_repaint();
            } else {
                // Jangan cek berulang kali jika goresan bukan bentuk geometri hingga bergerak lagi
                state.last_stroke_move_time = None;
            }
        } else {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(50));
        }
    }

    if response.drag_stopped() {
        state.eraser_undo_pushed = false;
        state.last_whole_erase_pos = None;
        if state.laser_is_drawing {
            state.laser_is_drawing = false;
            if !state.current_laser_stroke.is_empty() {
                let stroke = std::mem::take(&mut state.current_laser_stroke);
                state.laser_strokes.push(stroke);
            }
            state.laser_last_active = Instant::now();
        }
        if state.is_dragging_selection {
            state.is_dragging_selection = false;
            state.drag_start_pos = None;
            state.spatial_grid.rebuild(&page.objects);
        } else if state.resize_corner.is_some() {
            state.resize_corner = None;
            state.drag_start_pos = None;
            state.spatial_grid.rebuild(&page.objects);
        } else if state.active_stroke_page_id.as_deref() == Some(&page.id) {
            state.active_stroke_page_id = None;
            state.last_stroke_move_time = None;
            let recognized_shape = state.recognized_draft_shape.take();
            if let Some(mut pts) = state.draft_stroke.take() {
                // Ensure exact final point
                if let Some(pos) = pointer_pos {
                    pts.push(StrokePoint::from(screen_to_page(pos)));
                }

                if pts.len() > 1 {
                    state.push_undo(&page.id, &page.objects);
                    let is_highlighter = state.current_tool == ActiveTool::Highlighter;
                    let stroke_width = if is_highlighter {
                        state.stroke_width * 6.0
                    } else if state.current_tool == ActiveTool::Pencil {
                        (state.stroke_width - 0.5).max(1.0)
                    } else {
                        state.stroke_width
                    };
                    let color = if is_highlighter {
                        Color32::from_rgba_unmultiplied(
                            state.stroke_color.r(),
                            state.stroke_color.g(),
                            state.stroke_color.b(),
                            110,
                        )
                    } else {
                        state.stroke_color
                    };
                    // Fase 8.1: Shape Recognition — jika sudah ditahan ~500ms dan dikenali sebagai Shape
                    if let Some(recognized) = recognized_shape {
                        let new_id = Uuid::new_v4().to_string();
                        let is_line = recognized.kind == ShapeKind::Line;
                        page.objects.push(DrawObject::Shape {
                            id: new_id.clone(),
                            shape_type: recognized.kind,
                            x: recognized.x,
                            y: recognized.y,
                            width: recognized.width,
                            height: recognized.height,
                            color: color.into(),
                            stroke_width,
                            fill: None,
                        });
                        if is_line {
                            state.selected_ids = vec![new_id];
                        }
                        state.spatial_grid.rebuild(&page.objects);
                    } else {
                        page.objects.push(DrawObject::Stroke {
                            id: Uuid::new_v4().to_string(),
                            points: pts,
                            color: color.into(),
                            width: stroke_width,
                            is_highlighter,
                        });
                        state.spatial_grid.rebuild(&page.objects);
                        // Fase 8.6: Perbarui indeks OCR tulisan tangan otomatis
                        page.ocr_text = recognize_handwriting_from_strokes(&page.objects);
                    }
                }
            } else if let (Some(start), Some(curr)) = (
                state.draft_shape_start.take(),
                state.draft_shape_current.take(),
            ) && (curr - start).length() > 4.0
            {
                state.push_undo(&page.id, &page.objects);
                if let ActiveTool::Shape(kind) = state.current_tool {
                    let new_id = Uuid::new_v4().to_string();
                    page.objects.push(DrawObject::Shape {
                        id: new_id.clone(),
                        shape_type: kind,
                        x: start.x,
                        y: start.y,
                        width: curr.x - start.x,
                        height: curr.y - start.y,
                        color: state.stroke_color.into(),
                        stroke_width: state.stroke_width,
                        fill: None,
                    });
                    if kind == ShapeKind::Line {
                        state.selected_ids = vec![new_id];
                    }
                    state.spatial_grid.rebuild(&page.objects);
                }
            } else if !state.lasso_points.is_empty() {
                if state.lasso_points.len() > 2 {
                    let mut selected = Vec::new();
                    for obj in &page.objects {
                        if object_in_lasso(obj, &state.lasso_points) {
                            selected.push(obj.id().to_string());
                        }
                    }
                    state.selected_ids = selected;
                }
                state.lasso_points.clear();
            }
        }
    }

    // 4. Render Active Draft Stroke / Shape with live smooth tessellation
    if state.active_stroke_page_id.as_deref() == Some(&page.id) {
        if let Some(recognized) = &state.recognized_draft_shape {
            let stroke = Stroke::new(state.stroke_width * state.zoom, state.stroke_color);
            draw_shape_primitive(
                &painter,
                recognized.kind,
                recognized.x,
                recognized.y,
                recognized.width,
                recognized.height,
                Color32::TRANSPARENT,
                stroke,
                page_to_screen,
            );
            if recognized.kind == ShapeKind::Line {
                let s_start = page_to_screen(Pos2::new(recognized.x, recognized.y));
                let s_end = page_to_screen(Pos2::new(
                    recognized.x + recognized.width,
                    recognized.y + recognized.height,
                ));
                let handle_stroke = Stroke::new(2.0_f32, Color32::from_rgb(0, 114, 206));
                painter.circle(s_start, 6.5, Color32::WHITE, handle_stroke);
                painter.circle_filled(s_start, 3.0, Color32::from_rgb(0, 114, 206));
                painter.circle(s_end, 6.0, Color32::WHITE, handle_stroke);
            }
        } else if let Some(pts) = &state.draft_stroke {
            let is_highlighter = state.current_tool == ActiveTool::Highlighter;
            let stroke_width = if is_highlighter {
                state.stroke_width * 6.0 * state.zoom
            } else {
                state.stroke_width * state.zoom
            };
            let color = if is_highlighter {
                Color32::from_rgba_unmultiplied(
                    state.stroke_color.r(),
                    state.stroke_color.g(),
                    state.stroke_color.b(),
                    110,
                )
            } else {
                state.stroke_color
            };
            let smoothed = smooth_stroke_points(pts, state.smoothing_steps);
            let mesh = tessellate_stroke_ex(
                &smoothed,
                stroke_width,
                color,
                is_highlighter,
                page_to_screen,
            );
            painter.add(mesh);
        }

        if let (Some(start), Some(curr)) = (state.draft_shape_start, state.draft_shape_current) {
            let stroke = Stroke::new(state.stroke_width * state.zoom, state.stroke_color);
            if let ActiveTool::Shape(kind) = state.current_tool {
                draw_shape_primitive(
                    &painter,
                    kind,
                    start.x,
                    start.y,
                    curr.x - start.x,
                    curr.y - start.y,
                    Color32::TRANSPARENT,
                    stroke,
                    page_to_screen,
                );
                if kind == ShapeKind::Line {
                    let s_start = page_to_screen(start);
                    let s_end = page_to_screen(curr);
                    let handle_stroke = Stroke::new(2.0_f32, Color32::from_rgb(0, 114, 206));
                    painter.circle(s_start, 6.5, Color32::WHITE, handle_stroke);
                    painter.circle_filled(s_start, 3.0, Color32::from_rgb(0, 114, 206));
                    painter.circle(s_end, 6.0, Color32::WHITE, handle_stroke);
                }
            }
        }
    }

    // 5. Render Active Lasso Loop (hanya pada halaman tempat Lasso sedang ditarik)
    if state.active_stroke_page_id.as_deref() == Some(&page.id) && state.lasso_points.len() > 1 {
        let stroke = Stroke::new(1.5_f32, Color32::from_rgb(0, 114, 206));
        for i in 1..state.lasso_points.len() {
            let p1 = page_to_screen(state.lasso_points[i - 1]);
            let p2 = page_to_screen(state.lasso_points[i]);
            painter.line_segment([p1, p2], stroke);
        }
    }

    // 6. Render Selection Bounding Box & Handles (recompute after drag mutations)
    let updated_selected_objects: Vec<&DrawObject> = page
        .objects
        .iter()
        .filter(|o| state.selected_ids.contains(&o.id().to_string()))
        .collect();
    let updated_selection_rect = combined_bounds(&updated_selected_objects);
    let updated_single_line: Option<(Pos2, Pos2)> = if updated_selected_objects.len() == 1 {
        if let DrawObject::Shape {
            shape_type: ShapeKind::Line,
            x,
            y,
            width,
            height,
            ..
        } = updated_selected_objects[0]
        {
            Some((Pos2::new(*x, *y), Pos2::new(*x + *width, *y + *height)))
        } else {
            None
        }
    } else {
        None
    };

    if let Some((line_start, line_end)) = updated_single_line {
        let s_start = page_to_screen(line_start);
        let s_end = page_to_screen(line_end);
        let handle_stroke = Stroke::new(2.0_f32, Color32::from_rgb(0, 114, 206));

        // Garis bantu seleksi antara titik awal (pusat) dan titik akhir
        painter.line_segment(
            [s_start, s_end],
            Stroke::new(1.5_f32, Color32::from_rgba_unmultiplied(0, 114, 206, 160)),
        );

        // Titik awal garis sebagai Pusat Rotasi (Pivot Anchor: lingkaran ganda)
        painter.circle(s_start, 7.0, Color32::WHITE, handle_stroke);
        painter.circle_filled(s_start, 3.5, Color32::from_rgb(0, 114, 206));

        // Titik akhir garis yang dapat dirotasi
        painter.circle(s_end, 6.5, Color32::WHITE, handle_stroke);

        // Tangkai & tombol rotasi (⟳) di ujung titik akhir garis
        let dir = s_end - s_start;
        let dir_norm = if dir.length() > 1e-3 {
            dir / dir.length()
        } else {
            Vec2::new(1.0, 0.0)
        };
        let rot_pos = s_end + dir_norm * 22.0;
        painter.line_segment([s_end + dir_norm * 6.5, rot_pos], handle_stroke);
        painter.circle(rot_pos, 7.5, Color32::WHITE, handle_stroke);
        painter.text(
            rot_pos,
            egui::Align2::CENTER_CENTER,
            "⟳",
            egui::FontId::proportional(11.0),
            Color32::from_rgb(0, 114, 206),
        );
    } else if let Some(b) = updated_selection_rect {
        let screen_box = Rect::from_min_max(page_to_screen(b.min), page_to_screen(b.max));
        painter.rect_stroke(
            screen_box.expand(4.0),
            2.0,
            Stroke::new(1.5_f32, Color32::from_rgb(0, 114, 206)),
            egui::StrokeKind::Outside,
        );

        // Corner handles
        let handle_stroke = Stroke::new(2.0_f32, Color32::from_rgb(0, 114, 206));
        painter.circle(
            screen_box.right_bottom(),
            6.0,
            Color32::WHITE,
            handle_stroke,
        );
        painter.circle(screen_box.left_top(), 6.0, Color32::WHITE, handle_stroke);
        painter.circle(screen_box.right_top(), 6.0, Color32::WHITE, handle_stroke);
        painter.circle(screen_box.left_bottom(), 6.0, Color32::WHITE, handle_stroke);

        // Rotation stem & handle (with ⟳ symbol)
        let stem_top = Pos2::new(screen_box.center().x, screen_box.top() - 18.0);
        painter.line_segment(
            [
                Pos2::new(screen_box.center().x, screen_box.top() - 4.0),
                stem_top,
            ],
            handle_stroke,
        );
        painter.circle(stem_top, 7.0, Color32::WHITE, handle_stroke);
        painter.text(
            stem_top,
            egui::Align2::CENTER_CENTER,
            "⟳",
            egui::FontId::proportional(11.0),
            Color32::from_rgb(0, 114, 206),
        );
    }

    // 7. Render Glowing Laser Trail (Multi-stroke Neon Laser Sesuai Gambar Referensi)
    if laser_alpha > 0.0 {
        let mut all_strokes_to_draw: Vec<&[Pos2]> = Vec::new();
        for s in &state.laser_strokes {
            if s.len() > 1 {
                all_strokes_to_draw.push(s.as_slice());
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
                painter.add(egui::Shape::Path(egui::epaint::PathShape::line(
                    smoothed.clone(),
                    Stroke::new(18.0_f32, outer_col),
                )));

                // Layer 2: Mid Neon Halo
                painter.add(egui::Shape::Path(egui::epaint::PathShape::line(
                    smoothed.clone(),
                    Stroke::new(10.0_f32, halo_col),
                )));

                // Layer 3: Solid Vibrant Red Core
                painter.add(egui::Shape::Path(egui::epaint::PathShape::line(
                    smoothed.clone(),
                    Stroke::new(5.0_f32, core_col),
                )));

                // Layer 4: Hotspot White Laser Beam Center
                painter.add(egui::Shape::Path(egui::epaint::PathShape::line(
                    smoothed.clone(),
                    Stroke::new(2.2_f32, beam_col),
                )));

                // Semicircle / round caps di awal dan akhir garis untuk menghilangkan ujung flat
                if let (Some(&start), Some(&end)) = (smoothed.first(), smoothed.last()) {
                    painter.circle_filled(start, 9.0, outer_col);
                    painter.circle_filled(end, 9.0, outer_col);
                    painter.circle_filled(start, 5.0, halo_col);
                    painter.circle_filled(end, 5.0, halo_col);
                    painter.circle_filled(start, 2.5, core_col);
                    painter.circle_filled(end, 2.5, core_col);
                    painter.circle_filled(start, 1.1, beam_col);
                    painter.circle_filled(end, 1.1, beam_col);
                }
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

    // 8. Fase 8.3: Render Indikator Kotak Target Zoom Window pada Kanvas Utama
    if state.show_zoom_window {
        let z_min = page_to_screen(state.zoom_target_rect.min);
        let z_max = page_to_screen(state.zoom_target_rect.max);
        let z_screen_rect = Rect::from_min_max(z_min, z_max);
        painter.rect_filled(
            z_screen_rect,
            2.0,
            Color32::from_rgba_unmultiplied(0, 114, 206, 18),
        );
        painter.rect_stroke(
            z_screen_rect,
            2.0,
            Stroke::new(2.0_f32, Color32::from_rgb(0, 114, 206)),
            egui::StrokeKind::Outside,
        );
        // Indikator zona auto-advance (25% sisi kanan)
        let adv_left = z_screen_rect.right() - z_screen_rect.width() * 0.25;
        let adv_rect = Rect::from_min_max(
            Pos2::new(adv_left, z_screen_rect.top()),
            z_screen_rect.right_bottom(),
        );
        painter.rect_filled(
            adv_rect,
            0.0,
            Color32::from_rgba_unmultiplied(0, 114, 206, 35),
        );
    }

    // Double-click pada objek Teks atau StickyNote untuk menyunting isinya secara langsung
    if response.double_clicked()
        && let Some(pos) = pointer_pos
    {
        let page_p = screen_to_page(pos);
        if let Some(obj) = page.objects.iter().rev().find(|obj| {
            matches!(obj, DrawObject::Text { .. } | DrawObject::StickyNote { .. })
                && hit_test_object(obj, page_p, 8.0 / state.zoom)
        }) {
            let id = obj.id().to_string();
            state.push_undo(&page.id, &page.objects);
            state.selected_ids = vec![id.clone()];
            state.editing_text_id = Some(id);
            state.focus_text_edit = true;
        }
    }

    // Single-click & Shift+click selection in Lasso mode, Text/StickyNote creation, or precision erase
    if response.clicked()
        && let Some(pos) = pointer_pos
    {
        let page_p = screen_to_page(pos);
        let shift_held = ui.input(|i| i.modifiers.shift);

        if state.current_tool == ActiveTool::Text {
            let hit_id = page
                .objects
                .iter()
                .rev()
                .find(|obj| {
                    matches!(obj, DrawObject::Text { .. } | DrawObject::StickyNote { .. })
                        && hit_test_object(obj, page_p, 8.0 / state.zoom)
                })
                .map(|o| o.id().to_string());
            if let Some(id) = hit_id {
                state.push_undo(&page.id, &page.objects);
                state.selected_ids = vec![id.clone()];
                state.editing_text_id = Some(id);
                state.focus_text_edit = true;
            } else if state.editing_text_id.is_some() {
                // Jika sedang menyunting teks dan klik di luar kotak teks, selesaikan penyuntingan
                state.editing_text_id = None;
                page.objects.retain(
                    |o| !matches!(o, DrawObject::Text { text, .. } if text.trim().is_empty()),
                );
                state.clean_redundant_top_undo(&page.id, &page.objects);
                state.spatial_grid.rebuild(&page.objects);
            } else {
                state.push_undo(&page.id, &page.objects);
                let id = Uuid::new_v4().to_string();
                let new_text = DrawObject::Text {
                    id: id.clone(),
                    x: page_p.x,
                    y: page_p.y,
                    text: String::new(),
                    font_size: (state.stroke_width * 4.0 + 14.0).clamp(12.0, 72.0),
                    color: state.stroke_color.into(),
                };
                page.objects.push(new_text);
                state.spatial_grid.rebuild(&page.objects);
                state.selected_ids = vec![id.clone()];
                state.editing_text_id = Some(id);
                state.focus_text_edit = true;
            }
        } else if state.current_tool == ActiveTool::StickyNote {
            state.push_undo(&page.id, &page.objects);
            let id = Uuid::new_v4().to_string();
            let new_note = DrawObject::StickyNote {
                id: id.clone(),
                x: page_p.x,
                y: page_p.y,
                width: 180.0,
                height: 140.0,
                text: String::new(),
                color: Color32::from_rgb(254, 243, 189).into(),
            };
            page.objects.push(new_note);
            state.spatial_grid.rebuild(&page.objects);
            state.selected_ids = vec![id.clone()];
            state.editing_text_id = Some(id);
            state.focus_text_edit = true;
            state.current_tool = ActiveTool::Lasso;
        } else if state.current_tool == ActiveTool::Eraser {
            state.eraser_undo_pushed = false;
            state.last_whole_erase_pos = None;
            let erase_radius = match state.eraser_mode {
                EraserMode::WholeStroke => (3.8 / state.zoom).clamp(1.8, 4.5),
                EraserMode::Partial => (10.0 / state.zoom).clamp(4.0, 16.0),
            };
            if state.erase_at(page, page_p, erase_radius) {
                ui.ctx().request_repaint();
            }
            state.eraser_undo_pushed = false;
            state.last_whole_erase_pos = None;
        } else if state.current_tool == ActiveTool::Lasso {
            let hit = page
                .objects
                .iter()
                .rev()
                .find(|obj| hit_test_object(obj, page_p, 8.0 / state.zoom));
            if let Some(obj) = hit {
                let id = obj.id().to_string();
                if shift_held {
                    if let Some(idx) = state.selected_ids.iter().position(|x| x == &id) {
                        state.selected_ids.remove(idx);
                    } else {
                        state.selected_ids.push(id);
                    }
                } else {
                    state.selected_ids = vec![id];
                }
            } else if !shift_held {
                state.selected_ids.clear();
                state.editing_text_id = None;
            }
        }
    }

    // 9. Inline Text / StickyNote Editor langsung di atas kanvas halaman
    if let Some(edit_id) = state.editing_text_id.clone() {
        let mut close_editor = ui.input(|i| i.key_pressed(Key::Escape));
        if let Some(obj) = page.objects.iter_mut().find(|o| o.id() == edit_id) {
            match obj {
                DrawObject::Text {
                    x,
                    y,
                    text,
                    font_size,
                    color,
                    ..
                } => {
                    let screen_tl = page_to_screen(Pos2::new(*x, *y));
                    let lines_cnt = text.lines().count().max(1) as f32;
                    let max_chars = text
                        .lines()
                        .map(|l| l.chars().count())
                        .max()
                        .unwrap_or(12)
                        .max(12) as f32;
                    let scaled_font = (*font_size * state.zoom).clamp(10.0, 96.0);
                    let box_w = (max_chars * scaled_font * 0.62 + 28.0).max(180.0);
                    let box_h = ((lines_cnt + 0.6) * scaled_font * 1.35 + 12.0).max(36.0);
                    let edit_rect = Rect::from_min_size(screen_tl, Vec2::new(box_w, box_h));

                    painter.rect_filled(
                        edit_rect.expand(4.0),
                        4.0,
                        Color32::from_rgba_unmultiplied(255, 255, 255, 215),
                    );
                    painter.rect_stroke(
                        edit_rect.expand(4.0),
                        4.0,
                        Stroke::new(1.8_f32, Color32::from_rgb(0, 114, 206)),
                        egui::StrokeKind::Outside,
                    );

                    let te_id = egui::Id::new(("canvas_inline_text", &edit_id));
                    let te = egui::TextEdit::multiline(text)
                        .id(te_id)
                        .font(egui::FontId::proportional(scaled_font))
                        .text_color(color.to_color32())
                        .hint_text("Ketik teks di sini...")
                        .frame(false)
                        .desired_width(box_w);
                    let te_resp = ui.put(edit_rect, te);
                    if state.focus_text_edit {
                        te_resp.request_focus();
                        state.focus_text_edit = false;
                    } else if te_resp.lost_focus() {
                        close_editor = true;
                    }
                }
                DrawObject::StickyNote {
                    x,
                    y,
                    width,
                    height,
                    text,
                    ..
                } => {
                    let p1 = page_to_screen(Pos2::new(*x, *y)) + Vec2::new(8.0, 8.0);
                    let p2 =
                        page_to_screen(Pos2::new(*x + *width, *y + *height)) - Vec2::new(8.0, 8.0);
                    let edit_rect = Rect::from_two_pos(p1, p2);
                    let scaled_font = (14.0 * state.zoom).clamp(10.0, 42.0);

                    painter.rect_stroke(
                        edit_rect.expand(6.0),
                        4.0,
                        Stroke::new(2.0_f32, Color32::from_rgb(0, 114, 206)),
                        egui::StrokeKind::Outside,
                    );

                    let te_id = egui::Id::new(("canvas_inline_sticky", &edit_id));
                    let te = egui::TextEdit::multiline(text)
                        .id(te_id)
                        .font(egui::FontId::proportional(scaled_font))
                        .text_color(Color32::from_rgb(35, 35, 35))
                        .hint_text("Tulis catatan tempel...")
                        .frame(false)
                        .desired_width(edit_rect.width());
                    let te_resp = ui.put(edit_rect, te);
                    if state.focus_text_edit {
                        te_resp.request_focus();
                        state.focus_text_edit = false;
                    } else if te_resp.lost_focus() {
                        close_editor = true;
                    }
                }
                _ => {
                    close_editor = true;
                }
            }
        }
        if close_editor {
            state.editing_text_id = None;
            page.objects
                .retain(|o| !matches!(o, DrawObject::Text { text, .. } if text.trim().is_empty()));
            state.clean_redundant_top_undo(&page.id, &page.objects);
            state.spatial_grid.rebuild(&page.objects);
        }
    }

    // Context menu on selection or blank canvas
    let has_selection_on_this_page = !state.selected_ids.is_empty()
        && page
            .objects
            .iter()
            .any(|o| state.selected_ids.contains(&o.id().to_string()));

    let click_page_p = screen_to_page(
        response
            .interact_pointer_pos()
            .or_else(|| ui.input(|i| i.pointer.hover_pos()))
            .unwrap_or(rect.center()),
    );

    response.context_menu(|ui| {
        if has_selection_on_this_page {
            let count = page
                .objects
                .iter()
                .filter(|o| state.selected_ids.contains(&o.id().to_string()))
                .count();
            ui.label(format!("{} objek dipilih", count));
            ui.separator();
            if ui.button("📋 Salin (Ctrl+C)").clicked() {
                state.copy_selected(&page.objects);
                ui.close_menu();
            }
            if ui.button("✂ Potong (Ctrl+X)").clicked() {
                state.cut_selected(page);
                ui.close_menu();
            }
            if ui.button("📄 Gandakan (Ctrl+D)").clicked() {
                state.duplicate_selected(page);
                ui.close_menu();
            }
            if ui.button("🗑 Hapus (Del)").clicked() {
                state.delete_selected(page);
                ui.close_menu();
            }
            ui.separator();
            if ui.button("🔄 Balik Horizontal").clicked() {
                state.flip_selected_horizontal(page);
                ui.close_menu();
            }
            if ui.button("🔃 Balik Vertikal").clicked() {
                state.flip_selected_vertical(page);
                ui.close_menu();
            }
            ui.separator();
            if ui.button("⬆ Pindah ke Depan").clicked() {
                state.bring_selected_to_front(page);
                ui.close_menu();
            }
            if ui.button("⬇ Pindah ke Belakang").clicked() {
                state.send_selected_to_back(page);
                ui.close_menu();
            }
        } else {
            if !state.object_clipboard.is_empty() {
                if ui.button("📋 Tempel Objek (Ctrl+V)").clicked() {
                    state.paste_clipboard(page, Some(click_page_p));
                    ui.close_menu();
                }
                ui.separator();
            }
            if ui.button("🖼 Tempel Gambar dari Clipboard").clicked() {
                state.request_paste_image = Some(click_page_p);
                ui.close_menu();
            }
            ui.separator();
            if ui.button("Pilih Semua (Ctrl+A)").clicked() {
                state.selected_ids = page.objects.iter().map(|o| o.id().to_string()).collect();
                ui.close_menu();
            }
        }
    });

    response
}

/// Fase 8.3: Render kanvas Zoom Window (panel menulis detail halus dengan auto-scroll ke kanan).
/// Mengembalikan `true` jika ada perubahan objek pada halaman.
pub fn render_zoom_window_canvas(
    ui: &mut Ui,
    page: &mut Page,
    state: &mut CanvasState,
    scale: f32,
) -> bool {
    let target = state.zoom_target_rect;
    let desired_size = Vec2::new(target.width() * scale, target.height() * scale);
    let (zw_rect, response) = ui.allocate_exact_size(desired_size, Sense::click_and_drag());
    let painter = ui.painter_at(zw_rect);

    let zw_origin = zw_rect.min;
    let target_min = target.min;

    let zw_to_page = move |screen_pos: Pos2| -> Pos2 {
        let rel = screen_pos - zw_origin;
        Pos2::new(target_min.x + rel.x / scale, target_min.y + rel.y / scale)
    };
    let page_to_zw = move |page_pos: Pos2| -> Pos2 {
        let rel = page_pos - target_min;
        zw_origin + Vec2::new(rel.x * scale, rel.y * scale)
    };

    // 1. Latar kertas & pola halaman ter-skala
    painter.rect_filled(zw_rect, 4.0, page.paper_color.to_color32());
    let full_page_zw_rect = Rect::from_min_size(
        page_to_zw(Pos2::ZERO),
        Vec2::new(page.width * scale, page.height * scale),
    );
    if let crate::model::PageBackground::Template(tmpl) = page.effective_background() {
        draw_paper_pattern(&painter, full_page_zw_rect, tmpl, scale);
    }

    // 2. Zona Auto-Advance di 25% sisi kanan Zoom Window
    let adv_screen_left = zw_rect.right() - zw_rect.width() * 0.25;
    let adv_rect = Rect::from_min_max(
        Pos2::new(adv_screen_left, zw_rect.top()),
        zw_rect.right_bottom(),
    );
    painter.rect_filled(
        adv_rect,
        0.0,
        Color32::from_rgba_unmultiplied(0, 114, 206, 22),
    );
    painter.line_segment(
        [
            Pos2::new(adv_screen_left, zw_rect.top()),
            Pos2::new(adv_screen_left, zw_rect.bottom()),
        ],
        Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(0, 114, 206, 90)),
    );

    // 3. Render objek halaman yang tampak di dalam kotak target
    for obj in &page.objects {
        let b = object_bounds(obj);
        if !b.intersects(target.expand(20.0)) {
            continue;
        }
        match obj {
            DrawObject::Stroke {
                points,
                color,
                width,
                is_highlighter,
                ..
            } => {
                let smoothed = smooth_stroke_points(points, state.smoothing_steps);
                let stroke_w = width * scale;
                let stroke_c = if *is_highlighter {
                    Color32::from_rgba_unmultiplied(color.r, color.g, color.b, 90)
                } else {
                    color.to_color32()
                };
                let mesh = tessellate_stroke_ex(
                    &smoothed,
                    stroke_w,
                    stroke_c,
                    *is_highlighter,
                    page_to_zw,
                );
                painter.add(mesh);
            }
            other => {
                draw_non_stroke_object(&painter, other, page_to_zw, scale, &state.texture_cache);
            }
        }
    }

    // 4. Tangani input menulis / menghapus di dalam Zoom Window
    let mut modified = false;
    let pointer_pos = response.interact_pointer_pos();
    let adv_page_x = target.right() - target.width() * 0.25;

    if response.drag_started_by(PointerButton::Primary)
        && let Some(pos) = pointer_pos
    {
        let page_p = zw_to_page(pos);
        match state.current_tool {
            ActiveTool::Eraser => {
                state.eraser_undo_pushed = false;
                if state.erase_at(page, page_p, 12.0 / scale * state.zoom) {
                    modified = true;
                }
            }
            _ => {
                state.zoom_draft_stroke = Some(vec![StrokePoint::from(page_p)]);
                state.zoom_wrote_in_advance_zone = page_p.x >= adv_page_x;
            }
        }
    }

    if response.dragged_by(PointerButton::Primary)
        && let Some(pos) = pointer_pos
    {
        let page_p = zw_to_page(pos);
        match state.current_tool {
            ActiveTool::Eraser => {
                if state.erase_at(page, page_p, 12.0 / scale * state.zoom) {
                    modified = true;
                }
            }
            _ => {
                if let Some(pts) = &mut state.zoom_draft_stroke {
                    let min_dist = 1.5 / scale;
                    if pts
                        .last()
                        .is_none_or(|last| (page_p - last.to_pos2()).length() >= min_dist)
                    {
                        pts.push(StrokePoint::from(page_p));
                    }
                    if page_p.x >= adv_page_x {
                        state.zoom_wrote_in_advance_zone = true;
                    }
                }
            }
        }
    }

    if response.drag_stopped() {
        state.eraser_undo_pushed = false;
        state.last_whole_erase_pos = None;
        if let Some(mut pts) = state.zoom_draft_stroke.take() {
            if let Some(pos) = pointer_pos {
                let page_p = zw_to_page(pos);
                if page_p.x >= adv_page_x {
                    state.zoom_wrote_in_advance_zone = true;
                }
                pts.push(StrokePoint::from(page_p));
            }
            if pts.len() > 1 {
                state.push_undo(&page.id, &page.objects);
                let is_highlighter = state.current_tool == ActiveTool::Highlighter;
                let stroke_width = if is_highlighter {
                    state.stroke_width * 6.0
                } else if state.current_tool == ActiveTool::Pencil {
                    (state.stroke_width - 0.5).max(1.0)
                } else {
                    state.stroke_width
                };
                let color = if is_highlighter {
                    Color32::from_rgba_unmultiplied(
                        state.stroke_color.r(),
                        state.stroke_color.g(),
                        state.stroke_color.b(),
                        110,
                    )
                } else {
                    state.stroke_color
                };
                page.objects.push(DrawObject::Stroke {
                    id: Uuid::new_v4().to_string(),
                    points: pts,
                    color: color.into(),
                    width: stroke_width,
                    is_highlighter,
                });
                state.spatial_grid.rebuild(&page.objects);
                // Fase 8.6: Perbarui indeks OCR tulisan tangan otomatis saat menulis di Zoom Window
                page.ocr_text = recognize_handwriting_from_strokes(&page.objects);
                modified = true;

                // Auto-advance ke kanan jika pengguna menulis di zona kanan
                if state.zoom_wrote_in_advance_zone {
                    state.advance_zoom_target(page.width, page.height);
                    state.zoom_wrote_in_advance_zone = false;
                }
            }
        }
    }

    // 5. Render draft stroke yang sedang ditulis di Zoom Window
    if let Some(pts) = &state.zoom_draft_stroke {
        let is_highlighter = state.current_tool == ActiveTool::Highlighter;
        let stroke_w = if is_highlighter {
            state.stroke_width * 6.0 * scale
        } else {
            state.stroke_width * scale
        };
        let color = if is_highlighter {
            Color32::from_rgba_unmultiplied(
                state.stroke_color.r(),
                state.stroke_color.g(),
                state.stroke_color.b(),
                110,
            )
        } else {
            state.stroke_color
        };
        let smoothed = smooth_stroke_points(pts, state.smoothing_steps);
        let mesh = tessellate_stroke_ex(&smoothed, stroke_w, color, is_highlighter, page_to_zw);
        painter.add(mesh);
    }

    painter.rect_stroke(
        zw_rect,
        4.0,
        Stroke::new(2.0_f32, Color32::from_rgb(0, 114, 206)),
        egui::StrokeKind::Outside,
    );

    modified
}

fn draw_paper_pattern(painter: &Painter, rect: Rect, template: PageTemplate, zoom: f32) {
    let spacing = 30.0 * zoom;
    let line_color = Color32::from_rgb(220, 226, 231);

    match template {
        PageTemplate::Blank => {}
        PageTemplate::Ruled => {
            let mut y = rect.top() + spacing;
            while y < rect.bottom() {
                painter.line_segment(
                    [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
                    Stroke::new(0.8_f32, line_color),
                );
                y += spacing;
            }
        }
        PageTemplate::Grid => {
            let mut y = rect.top() + spacing;
            while y < rect.bottom() {
                painter.line_segment(
                    [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
                    Stroke::new(0.8_f32, line_color),
                );
                y += spacing;
            }
            let mut x = rect.left() + spacing;
            while x < rect.right() {
                painter.line_segment(
                    [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
                    Stroke::new(0.8_f32, line_color),
                );
                x += spacing;
            }
        }
        PageTemplate::Dotted => {
            let mut y = rect.top() + spacing;
            while y < rect.bottom() {
                let mut x = rect.left() + spacing;
                while x < rect.right() {
                    painter.circle_filled(Pos2::new(x, y), 1.0, Color32::from_rgb(204, 211, 218));
                    x += spacing;
                }
                y += spacing;
            }
        }
        PageTemplate::Cornell => {
            let line_color = Color32::from_rgb(200, 210, 220);
            let summary_y = rect.bottom() - rect.height() * 0.16;
            let cue_x = rect.left() + rect.width() * 0.28;

            // Ruled lines in main area
            let mut y = rect.top() + spacing * 1.5;
            while y < summary_y {
                painter.line_segment(
                    [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
                    Stroke::new(0.8_f32, Color32::from_rgb(230, 235, 240)),
                );
                y += spacing;
            }

            // Cue vertical line
            painter.line_segment(
                [
                    Pos2::new(cue_x, rect.top() + rect.height() * 0.04),
                    Pos2::new(cue_x, summary_y),
                ],
                Stroke::new(1.5_f32, line_color),
            );

            // Summary horizontal line
            painter.line_segment(
                [
                    Pos2::new(rect.left(), summary_y),
                    Pos2::new(rect.right(), summary_y),
                ],
                Stroke::new(1.5_f32, line_color),
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_shape_primitive(
    painter: &Painter,
    shape_type: ShapeKind,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    fill_color: Color32,
    stroke: Stroke,
    page_to_screen: impl Fn(Pos2) -> Pos2,
) {
    let p1 = page_to_screen(Pos2::new(x, y));
    let p2 = page_to_screen(Pos2::new(x + width, y + height));
    match shape_type {
        ShapeKind::Line => {
            painter.line_segment([p1, p2], stroke);
        }
        ShapeKind::Rectangle => {
            let r = Rect::from_two_pos(p1, p2);
            painter.rect(r, 0.0, fill_color, stroke, egui::StrokeKind::Outside);
        }
        ShapeKind::Ellipse => {
            let r = Rect::from_two_pos(p1, p2);
            let radius = (r.width().min(r.height()) / 2.0).max(1.0);
            painter.circle(r.center(), radius, fill_color, stroke);
        }
        ShapeKind::Triangle
        | ShapeKind::RightTriangle
        | ShapeKind::ScaleneTriangle { .. }
        | ShapeKind::Parallelogram => {
            let verts: Vec<Pos2> = crate::geometry::shape_vertices(shape_type, x, y, width, height)
                .into_iter()
                .map(&page_to_screen)
                .collect();
            if fill_color != Color32::TRANSPARENT && verts.len() >= 3 {
                painter.add(egui::Shape::convex_polygon(
                    verts.clone(),
                    fill_color,
                    Stroke::NONE,
                ));
            }
            for i in 0..verts.len() {
                painter.line_segment([verts[i], verts[(i + 1) % verts.len()]], stroke);
            }
        }
    }
}

fn draw_non_stroke_object(
    painter: &Painter,
    obj: &DrawObject,
    page_to_screen: impl Fn(Pos2) -> Pos2,
    zoom: f32,
    textures: &std::collections::HashMap<String, egui::TextureHandle>,
) {
    match obj {
        DrawObject::Stroke { .. } => {}
        DrawObject::Shape {
            shape_type,
            x,
            y,
            width,
            height,
            color,
            stroke_width,
            fill,
            ..
        } => {
            let stroke = Stroke::new(stroke_width * zoom, color.to_color32());
            let fill_color = fill
                .as_ref()
                .map_or(Color32::TRANSPARENT, |f| f.to_color32());
            draw_shape_primitive(
                painter,
                *shape_type,
                *x,
                *y,
                *width,
                *height,
                fill_color,
                stroke,
                page_to_screen,
            );
        }
        DrawObject::Text {
            x,
            y,
            text,
            font_size,
            color,
            ..
        } => {
            let pos = page_to_screen(Pos2::new(*x, *y));
            painter.text(
                pos,
                egui::Align2::LEFT_TOP,
                text,
                egui::FontId::proportional(font_size * zoom),
                color.to_color32(),
            );
        }
        DrawObject::StickyNote {
            x,
            y,
            width,
            height,
            text,
            color,
            ..
        } => {
            let p1 = page_to_screen(Pos2::new(*x, *y));
            let p2 = page_to_screen(Pos2::new(x + width, y + height));
            let r = Rect::from_two_pos(p1, p2);
            painter.rect_filled(r, 4.0, color.to_color32());
            painter.rect_stroke(
                r,
                4.0,
                Stroke::new(1.0_f32, Color32::from_rgb(235, 208, 116)),
                egui::StrokeKind::Outside,
            );
            painter.text(
                r.left_top() + Vec2::new(10.0, 10.0),
                egui::Align2::LEFT_TOP,
                text,
                egui::FontId::proportional(14.0 * zoom),
                Color32::from_rgb(40, 40, 40),
            );
        }
        DrawObject::Image {
            asset_id,
            x,
            y,
            width,
            height,
            rotation,
            ..
        } => {
            let center = page_to_screen(Pos2::new(*x + *width / 2.0, *y + *height / 2.0));
            let half_w = (*width * zoom) / 2.0;
            let half_h = (*height * zoom) / 2.0;
            let r = Rect::from_center_size(center, Vec2::new(half_w * 2.0, half_h * 2.0));
            if let Some(tex) = textures.get(asset_id) {
                draw_rotated_image_object(painter, tex.id(), center, half_w, half_h, *rotation);
            } else {
                painter.rect_filled(r, 4.0, Color32::from_rgb(50, 50, 54));
                painter.rect_stroke(
                    r,
                    4.0,
                    Stroke::new(1.0_f32, Color32::from_rgb(90, 90, 95)),
                    egui::StrokeKind::Outside,
                );
                painter.text(
                    r.center(),
                    egui::Align2::CENTER_CENTER,
                    "🖼 Image",
                    egui::FontId::proportional(13.0 * zoom),
                    Color32::WHITE,
                );
            }
        }
    }
}

pub fn draw_rotated_background_image(
    painter: &Painter,
    tex_id: egui::TextureId,
    rect: Rect,
    rotation_deg: i32,
) {
    let (uv_tl, uv_tr, uv_br, uv_bl) = match rotation_deg.rem_euclid(360) {
        90 => (
            Pos2::new(0.0, 1.0),
            Pos2::new(0.0, 0.0),
            Pos2::new(1.0, 0.0),
            Pos2::new(1.0, 1.0),
        ),
        180 => (
            Pos2::new(1.0, 1.0),
            Pos2::new(0.0, 1.0),
            Pos2::new(0.0, 0.0),
            Pos2::new(1.0, 0.0),
        ),
        270 => (
            Pos2::new(1.0, 0.0),
            Pos2::new(1.0, 1.0),
            Pos2::new(0.0, 1.0),
            Pos2::new(0.0, 0.0),
        ),
        _ => (
            Pos2::new(0.0, 0.0),
            Pos2::new(1.0, 0.0),
            Pos2::new(1.0, 1.0),
            Pos2::new(0.0, 1.0),
        ),
    };

    let mut mesh = egui::Mesh::with_texture(tex_id);
    let idx = mesh.vertices.len() as u32;
    mesh.vertices.push(egui::epaint::Vertex {
        pos: rect.left_top(),
        uv: uv_tl,
        color: Color32::WHITE,
    });
    mesh.vertices.push(egui::epaint::Vertex {
        pos: rect.right_top(),
        uv: uv_tr,
        color: Color32::WHITE,
    });
    mesh.vertices.push(egui::epaint::Vertex {
        pos: rect.right_bottom(),
        uv: uv_br,
        color: Color32::WHITE,
    });
    mesh.vertices.push(egui::epaint::Vertex {
        pos: rect.left_bottom(),
        uv: uv_bl,
        color: Color32::WHITE,
    });

    mesh.add_triangle(idx, idx + 1, idx + 2);
    mesh.add_triangle(idx, idx + 2, idx + 3);

    painter.add(mesh);
}

fn draw_rotated_image_object(
    painter: &Painter,
    tex_id: egui::TextureId,
    center: Pos2,
    half_w: f32,
    half_h: f32,
    rot_rad: f32,
) {
    let cos_a = rot_rad.cos();
    let sin_a = rot_rad.sin();
    let rot = |dx: f32, dy: f32| -> Pos2 {
        center + Vec2::new(dx * cos_a - dy * sin_a, dx * sin_a + dy * cos_a)
    };

    let p0 = rot(-half_w, -half_h);
    let p1 = rot(half_w, -half_h);
    let p2 = rot(half_w, half_h);
    let p3 = rot(-half_w, half_h);

    let mut mesh = egui::Mesh::with_texture(tex_id);
    let idx = mesh.vertices.len() as u32;
    mesh.vertices.push(egui::epaint::Vertex {
        pos: p0,
        uv: Pos2::new(0.0, 0.0),
        color: Color32::WHITE,
    });
    mesh.vertices.push(egui::epaint::Vertex {
        pos: p1,
        uv: Pos2::new(1.0, 0.0),
        color: Color32::WHITE,
    });
    mesh.vertices.push(egui::epaint::Vertex {
        pos: p2,
        uv: Pos2::new(1.0, 1.0),
        color: Color32::WHITE,
    });
    mesh.vertices.push(egui::epaint::Vertex {
        pos: p3,
        uv: Pos2::new(0.0, 1.0),
        color: Color32::WHITE,
    });

    mesh.add_triangle(idx, idx + 1, idx + 2);
    mesh.add_triangle(idx, idx + 2, idx + 3);

    painter.add(mesh);
}

fn smooth_laser_points(pts: &[Pos2]) -> Vec<Pos2> {
    if pts.len() <= 2 {
        return pts.to_vec();
    }
    let mut smoothed = Vec::with_capacity(pts.len() * 3);
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
        for step in 1..=3 {
            let t = step as f32 / 3.0;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::tessellate_stroke;
    use crate::model::{ColorData, Page, PageTemplate, ShapeKind};

    fn make_test_page() -> Page {
        let mut page = Page::new(PageTemplate::Blank);
        page.id = "p1".to_string();
        page.objects.push(DrawObject::Shape {
            id: "s1".to_string(),
            shape_type: ShapeKind::Rectangle,
            x: 100.0,
            y: 100.0,
            width: 50.0,
            height: 50.0,
            color: ColorData::from(Color32::RED),
            stroke_width: 2.0,
            fill: None,
        });
        page.objects.push(DrawObject::Text {
            id: "t1".to_string(),
            x: 200.0,
            y: 200.0,
            text: "Hello".to_string(),
            font_size: 16.0,
            color: ColorData::from(Color32::BLACK),
        });
        page
    }

    #[test]
    fn test_canvas_state_copy_cut_paste_duplicate() {
        let mut state = CanvasState::default();
        let mut page = make_test_page();

        // 1. Copy
        state.selected_ids = vec!["s1".to_string()];
        state.copy_selected(&page.objects);
        assert_eq!(state.object_clipboard.len(), 1);
        assert_eq!(state.object_clipboard[0].id(), "s1");

        // 2. Paste
        state.paste_clipboard(&mut page, Some(Pos2::new(300.0, 300.0)));
        assert_eq!(page.objects.len(), 3);
        assert_eq!(state.selected_ids.len(), 1);
        let pasted_id = &state.selected_ids[0];
        assert_ne!(pasted_id, "s1"); // Pasted object gets new UUID

        // 3. Duplicate
        state.duplicate_selected(&mut page);
        assert_eq!(page.objects.len(), 4);
        assert_eq!(state.selected_ids.len(), 1);

        // 4. Cut
        state.selected_ids = vec!["t1".to_string()];
        state.cut_selected(&mut page);
        assert_eq!(page.objects.len(), 3);
        assert!(!page.objects.iter().any(|o| o.id() == "t1"));
        assert_eq!(state.object_clipboard.len(), 1);
        assert_eq!(state.object_clipboard[0].id(), "t1");
    }

    #[test]
    fn test_cross_page_copy_and_paste() {
        let mut state = CanvasState::default();
        let page1 = make_test_page();
        let mut page2 = Page::new(PageTemplate::Blank);
        page2.id = "p2".to_string();
        assert_eq!(page2.objects.len(), 0);

        // Copy "s1" and "t1" from page 1
        state.selected_ids = vec!["s1".to_string(), "t1".to_string()];
        state.copy_selected(&page1.objects);
        assert_eq!(state.object_clipboard.len(), 2);

        // Switch to page 2: selected_ids cleared
        state.selected_ids.clear();

        // Paste onto page 2
        state.paste_clipboard(&mut page2, None);
        assert_eq!(page2.objects.len(), 2);
        assert_eq!(state.selected_ids.len(), 2);
        // Pasted objects on page 2 have new IDs
        assert!(
            !page2
                .objects
                .iter()
                .any(|o| o.id() == "s1" || o.id() == "t1")
        );
    }

    #[test]
    fn test_canvas_state_delete_and_reorder() {
        let mut state = CanvasState::default();
        let mut page = make_test_page();

        // Bring to front
        state.selected_ids = vec!["s1".to_string()];
        state.bring_selected_to_front(&mut page);
        assert_eq!(page.objects.last().unwrap().id(), "s1");

        // Send to back
        state.send_selected_to_back(&mut page);
        assert_eq!(page.objects.first().unwrap().id(), "s1");

        // Delete
        state.delete_selected(&mut page);
        assert_eq!(page.objects.len(), 1);
        assert_eq!(page.objects[0].id(), "t1");
        assert!(state.selected_ids.is_empty());
    }

    #[test]
    fn test_canvas_state_flip_selection() {
        let mut state = CanvasState::default();
        let mut page = make_test_page();

        state.selected_ids = vec!["s1".to_string()];
        state.flip_selected_horizontal(&mut page);
        // "s1" is at x: 100, width: 50 -> center is 125.
        // Flipped across 125: x' = 2*125 - (100+50) = 250 - 150 = 100.
        if let DrawObject::Shape { x, .. } = &page.objects[0] {
            assert!((x - 100.0).abs() < 1e-4);
        } else {
            panic!("Expected shape");
        }

        state.flip_selected_vertical(&mut page);
        if let DrawObject::Shape { y, .. } = &page.objects[0] {
            assert!((y - 100.0).abs() < 1e-4);
        } else {
            panic!("Expected shape");
        }
    }

    #[test]
    fn test_canvas_state_partial_eraser_and_undo() {
        let mut state = CanvasState::default();
        let mut page = Page::new(PageTemplate::Blank);
        page.id = "p_erase".to_string();

        // Tambahkan 1 goresan panjang dari x=0 ke x=100 pada y=50
        let pts: Vec<StrokePoint> = (0..=10)
            .map(|i| StrokePoint {
                x: i as f32 * 10.0,
                y: 50.0,
                pressure: 0.5,
            })
            .collect();
        page.objects.push(DrawObject::Stroke {
            id: "st1".to_string(),
            points: pts,
            color: ColorData::from(Color32::BLACK),
            width: 2.0,
            is_highlighter: false,
        });
        state.spatial_grid.rebuild(&page.objects);

        // Mode default adalah Partial
        assert_eq!(state.eraser_mode, EraserMode::Partial);
        state.eraser_undo_pushed = false;
        let changed = state.erase_at(&mut page, Pos2::new(50.0, 50.0), 12.0);
        assert!(changed);
        // Goresan tunggal harus terbelah menjadi 2 goresan terpisah
        assert_eq!(page.objects.len(), 2);

        // Undo harus mengembalikan menjadi 1 goresan utuh
        state.undo(&mut page);
        assert_eq!(page.objects.len(), 1);
        assert_eq!(page.objects[0].id(), "st1");
    }

    #[test]
    fn test_zoom_window_auto_advance() {
        let mut state = CanvasState::default();
        let initial_x = state.zoom_target_rect.min.x;
        let initial_y = state.zoom_target_rect.min.y;

        // Advance sekali ke kanan pada halaman lebar 595
        state.advance_zoom_target(595.0, 842.0);
        assert!(state.zoom_target_rect.min.x > initial_x);
        assert!((state.zoom_target_rect.min.y - initial_y).abs() < 1e-4);

        // Newline mengembalikan x ke margin kiri dan menambah y
        state.newline_zoom_target(595.0, 842.0);
        assert!((state.zoom_target_rect.min.x - 30.0).abs() < 1e-4);
        assert!(state.zoom_target_rect.min.y > initial_y);
    }

    #[test]
    fn test_stroke_mesh_sticks_to_page_on_scroll_and_zoom() {
        let pts = vec![
            StrokePoint {
                x: 40.0,
                y: 60.0,
                pressure: 0.5,
            },
            StrokePoint {
                x: 120.0,
                y: 150.0,
                pressure: 0.7,
            },
            StrokePoint {
                x: 200.0,
                y: 180.0,
                pressure: 0.6,
            },
        ];
        let smoothed = smooth_stroke_points(&pts, 3);
        let page_mesh = tessellate_stroke(&smoothed, 3.0, Color32::BLACK, |p| p);

        // Simulasikan posisi layar setelah di-scroll (origin bergeser) dan di-zoom (zoom = 1.65)
        let scrolled_origin = Pos2::new(140.0, -220.0);
        let zoom = 1.65_f32;

        let transformed = transform_page_mesh_to_screen(&page_mesh, scrolled_origin, zoom);
        let direct_screen = tessellate_stroke(&smoothed, 3.0 * zoom, Color32::BLACK, |p| {
            scrolled_origin + Vec2::new(p.x * zoom, p.y * zoom)
        });

        assert_eq!(transformed.vertices.len(), direct_screen.vertices.len());
        for (v_cached, v_direct) in transformed
            .vertices
            .iter()
            .zip(direct_screen.vertices.iter())
        {
            assert!((v_cached.pos.x - v_direct.pos.x).abs() < 1e-3);
            assert!((v_cached.pos.y - v_direct.pos.y).abs() < 1e-3);
        }
    }

    #[test]
    fn test_whole_stroke_eraser_precision_on_connected_strokes() {
        let mut state = CanvasState {
            eraser_mode: EraserMode::WholeStroke,
            ..Default::default()
        };
        let mut page = Page::new(PageTemplate::Blank);
        page.id = "p_whole_precision".to_string();

        // Stroke 1: Garis vertikal dari (100, 50) ke (100, 150) yang digambar pada sesi 1
        page.objects.push(DrawObject::Stroke {
            id: "stroke_vert".to_string(),
            points: vec![
                StrokePoint {
                    x: 100.0,
                    y: 50.0,
                    pressure: 0.5,
                },
                StrokePoint {
                    x: 100.0,
                    y: 150.0,
                    pressure: 0.5,
                },
            ],
            color: ColorData::from(Color32::BLACK),
            width: 3.0,
            is_highlighter: false,
        });

        // Stroke 2: Garis horizontal dari (60, 50) ke (140, 50) yang menyambung di (100, 50), digambar pada sesi 2
        page.objects.push(DrawObject::Stroke {
            id: "stroke_horiz".to_string(),
            points: vec![
                StrokePoint {
                    x: 60.0,
                    y: 50.0,
                    pressure: 0.5,
                },
                StrokePoint {
                    x: 140.0,
                    y: 50.0,
                    pressure: 0.5,
                },
            ],
            color: ColorData::from(Color32::BLACK),
            width: 3.0,
            is_highlighter: false,
        });
        state.spatial_grid.rebuild(&page.objects);

        // Sentuh di dekat sambungan pada Stroke 2 (mis. x=103, y=50) + gerakan mikro 2px ke (101, 50):
        // Hanya Stroke 2 ("stroke_horiz") yang boleh terhapus; Stroke 1 ("stroke_vert") wajib tetap utuh!
        let erased_first = state.erase_at(&mut page, Pos2::new(103.0, 50.0), 4.0);
        let erased_micro_move = state.erase_at(&mut page, Pos2::new(100.5, 50.0), 4.0);

        assert!(erased_first);
        assert!(!erased_micro_move);
        assert_eq!(page.objects.len(), 1);
        assert_eq!(page.objects[0].id(), "stroke_vert");
    }

    #[test]
    fn test_undo_redo_does_not_affect_existing_text_on_subsequent_actions() {
        let mut state = CanvasState::default();
        let mut page = Page::new(PageTemplate::Blank);
        page.id = "page_undo_text".to_string();

        // 1. Pengguna membuat teks "Catatan Penting"
        state.push_undo(&page.id, &page.objects);
        page.objects.push(DrawObject::Text {
            id: "text_1".to_string(),
            x: 50.0,
            y: 50.0,
            text: "Catatan Penting".to_string(),
            font_size: 16.0,
            color: ColorData::from(Color32::BLACK),
        });

        assert_eq!(page.objects.len(), 1);
        assert_eq!(state.undo_stacks.get(&page.id).unwrap().len(), 1);

        // 2. Pengguna kemudian menggambar goresan pen baru
        state.push_undo(&page.id, &page.objects);
        page.objects.push(DrawObject::Stroke {
            id: "stroke_1".to_string(),
            points: vec![
                StrokePoint {
                    x: 10.0,
                    y: 10.0,
                    pressure: 0.5,
                },
                StrokePoint {
                    x: 50.0,
                    y: 50.0,
                    pressure: 0.5,
                },
            ],
            color: ColorData::from(Color32::BLACK),
            width: 2.0,
            is_highlighter: false,
        });

        assert_eq!(page.objects.len(), 2);
        assert_eq!(state.undo_stacks.get(&page.id).unwrap().len(), 2);

        // 3. Pengguna menekan Undo: HANYA goresan pen yang boleh dibatalkan!
        // Teks "Catatan Penting" yang sudah tertempel TIDAK BOLEH terpengaruh!
        state.undo(&mut page);

        assert_eq!(page.objects.len(), 1);
        assert_eq!(page.objects[0].id(), "text_1");
        if let DrawObject::Text { text, .. } = &page.objects[0] {
            assert_eq!(text, "Catatan Penting");
        } else {
            panic!("Expected text object to remain intact");
        }

        // 4. Pengguna menekan Redo: goresan pen kembali muncul
        state.redo(&mut page);
        assert_eq!(page.objects.len(), 2);
        assert_eq!(page.objects[0].id(), "text_1");
        assert_eq!(page.objects[1].id(), "stroke_1");
    }

    #[test]
    fn test_undo_redo_multi_page_isolation_and_order() {
        let mut state = CanvasState::default();
        let mut page1 = Page::new(PageTemplate::Blank);
        page1.id = "p1".to_string();
        let mut page2 = Page::new(PageTemplate::Blank);
        page2.id = "p2".to_string();

        // Di Page 1: Tambahkan teks
        state.push_undo(&page1.id, &page1.objects);
        page1.objects.push(DrawObject::Text {
            id: "txt_p1".to_string(),
            x: 20.0,
            y: 20.0,
            text: "Page 1 Text".to_string(),
            font_size: 14.0,
            color: ColorData::from(Color32::BLACK),
        });

        // Di Page 2: Tambahkan sebuah bentuk persegi
        state.push_undo(&page2.id, &page2.objects);
        page2.objects.push(DrawObject::Shape {
            id: "rect_p2".to_string(),
            shape_type: ShapeKind::Rectangle,
            x: 100.0,
            y: 100.0,
            width: 50.0,
            height: 50.0,
            color: ColorData::from(Color32::BLUE),
            stroke_width: 2.0,
            fill: None,
        });

        // Aksi mutasi terakhir adalah di Page 2
        assert_eq!(state.last_undo_page_id(), Some("p2"));

        // Lakukan undo pada halaman mutasi terakhir (Page 2)
        state.undo(&mut page2);
        assert_eq!(page2.objects.len(), 0);

        // Page 1 wajib 100% tidak terganggu dan tetap berisi teksnya!
        assert_eq!(page1.objects.len(), 1);
        assert_eq!(page1.objects[0].id(), "txt_p1");

        // Sekarang aksi undo berikutnya menunjuk ke Page 1
        assert_eq!(state.last_undo_page_id(), Some("p1"));
    }

    #[test]
    fn test_clean_redundant_top_undo_on_empty_text() {
        let mut state = CanvasState::default();
        let mut page = Page::new(PageTemplate::Blank);
        page.id = "p_empty".to_string();

        // 1. Keadaan awal: 1 stroke yang sudah ada
        page.objects.push(DrawObject::Stroke {
            id: "existing_stroke".to_string(),
            points: vec![StrokePoint {
                x: 0.0,
                y: 0.0,
                pressure: 0.5,
            }],
            color: ColorData::from(Color32::BLACK),
            width: 2.0,
            is_highlighter: false,
        });

        // 2. User klik Text Tool: snapshot dibuat dan teks kosong dimasukkan
        state.push_undo(&page.id, &page.objects);
        assert_eq!(state.undo_stacks.get(&page.id).unwrap().len(), 1);

        page.objects.push(DrawObject::Text {
            id: "empty_text".to_string(),
            x: 40.0,
            y: 40.0,
            text: "   ".to_string(),
            font_size: 14.0,
            color: ColorData::from(Color32::BLACK),
        });

        // 3. User batal (klik di luar atau Escape): teks kosong di-remove
        page.objects
            .retain(|o| !matches!(o, DrawObject::Text { text, .. } if text.trim().is_empty()));
        assert_eq!(page.objects.len(), 1);

        // 4. clean_redundant_top_undo harus membersihkan snapshot palsu
        state.clean_redundant_top_undo(&page.id, &page.objects);
        assert_eq!(state.undo_stacks.get(&page.id).unwrap().len(), 0);
        assert!(!state.can_undo(&page.id));
    }
}

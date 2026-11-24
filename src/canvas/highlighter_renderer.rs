use super::types::ActiveTool;
use crate::geometry::{
    HighlighterColorGroup, HighlighterStrokeInput, MaskTransform, group_highlighters_by_rgb,
    highlighter_opacity_to_alpha, rasterize_highlighter_group_mask, resolve_highlighter_alpha,
    stroke_radius_at_pressure,
};
use crate::model::{DrawObject, Page, StrokePoint, StrokeStyle};
use eframe::egui::{
    self, Color32, ColorImage, Pos2, Rect, Shape, TextureHandle, TextureOptions, Vec2,
};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

/// Cakupan lapisan render yang memiliki cache tekstur highlighter tersendiri.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HighlighterCacheScope {
    MainCanvas,
    ZoomWindow,
    Thumbnail,
}

impl HighlighterCacheScope {
    fn label(self) -> &'static str {
        match self {
            Self::MainCanvas => "main",
            Self::ZoomWindow => "zoom",
            Self::Thumbnail => "thumb",
        }
    }
}

/// Deskripsi goresan highlighter aktif yang sedang digambar (belum di-commit).
#[derive(Clone, Copy, Debug)]
pub struct ActiveHighlighterDraft<'a> {
    pub points: &'a [StrokePoint],
    pub stroke_color: Color32,
    pub page_width: f32,
    pub style: StrokeStyle,
}

struct HighlighterCacheEntry {
    content_hash: u64,
    raster_scale_bits: u32,
    page_rect: Rect,
    texture: TextureHandle,
    last_used_tick: u64,
}

/// Cache tekstur coverage-mask highlighter berkapasitas terbatas (LRU) untuk menghindari
/// rasterisasi ulang tiap frame saat tidak ada perubahan goresan atau skala.
pub struct HighlighterLayerCache {
    entries: HashMap<(HighlighterCacheScope, String), HighlighterCacheEntry>,
    tick: u64,
}

impl Default for HighlighterLayerCache {
    fn default() -> Self {
        Self::new()
    }
}

impl HighlighterLayerCache {
    pub const MAX_ENTRIES: usize = 24;
    pub const MAX_TEXTURE_SIDE: usize = 2048;

    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            tick: 0,
        }
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn remove_scope_page(&mut self, scope: HighlighterCacheScope, page_id: &str) {
        self.entries.remove(&(scope, page_id.to_string()));
    }

    fn evict_lru_if_needed(&mut self) {
        while self.entries.len() > Self::MAX_ENTRIES {
            if let Some(oldest_key) = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.last_used_tick)
                .map(|(k, _)| k.clone())
            {
                self.entries.remove(&oldest_key);
            } else {
                break;
            }
        }
    }
}

/// Mengompositkan seluruh grup warna highlighter ke dalam satu `ColorImage` berlatar transparan.
/// Goresan dalam grup warna yang sama digabungkan via coverage maksimum (tanpa penumpukan alpha),
/// sedangkan antar-grup warna berbeda dikompositkan dengan urutan deterministik (`SrcOver`).
pub fn composite_highlighter_groups_to_color_image(
    groups: &[HighlighterColorGroup<'_>],
    tex_w: usize,
    tex_h: usize,
    transform: MaskTransform,
    smoothing_steps: usize,
) -> ColorImage {
    let w = tex_w.max(1);
    let h = tex_h.max(1);
    let mut image = ColorImage::new([w, h], Color32::TRANSPARENT);
    let mut mask = vec![0u8; w.saturating_mul(h)];

    for (group_idx, group) in groups.iter().enumerate() {
        if group_idx > 0 {
            mask.fill(0);
        }
        rasterize_highlighter_group_mask(&mut mask, w, h, group, transform, smoothing_steps);
        let (r, g, b) = group.rgb;

        if group_idx == 0 {
            for (dst, &a) in image.pixels.iter_mut().zip(mask.iter()) {
                if a > 0 {
                    *dst = Color32::from_rgba_unmultiplied(r, g, b, a);
                }
            }
        } else {
            for (dst, &a) in image.pixels.iter_mut().zip(mask.iter()) {
                if a > 0 {
                    let src = Color32::from_rgba_unmultiplied(r, g, b, a);
                    *dst = if dst.a() == 0 { src } else { dst.blend(src) };
                }
            }
        }
    }

    image
}

/// Menghasilkan `ActiveHighlighterDraft` bila alat aktif adalah Highlighter dan memiliki titik draft.
pub fn active_highlighter_draft<'a>(
    tool: ActiveTool,
    draft_points: Option<&'a [StrokePoint]>,
    stroke_color: Color32,
    base_stroke_width: f32,
    style: StrokeStyle,
) -> Option<ActiveHighlighterDraft<'a>> {
    if tool != ActiveTool::Highlighter {
        return None;
    }
    let pts = draft_points?;
    if pts.is_empty() {
        return None;
    }
    Some(ActiveHighlighterDraft {
        points: pts,
        stroke_color,
        page_width: base_stroke_width * 6.0,
        style,
    })
}

/// Membangun `egui::Shape` lapisan underlay highlighter terkomposit (dengan cache tekstur)
/// untuk kanvas utama, Zoom Window, maupun thumbnail sidebar.
#[allow(clippy::too_many_arguments)]
pub fn build_highlighter_layer_shape(
    ctx: &egui::Context,
    scope: HighlighterCacheScope,
    page: &Page,
    active_draft: Option<ActiveHighlighterDraft<'_>>,
    cache: &mut HighlighterLayerCache,
    highlighter_opacity: f32,
    smoothing_steps: usize,
    zoom: f32,
    page_screen_rect: Rect,
    screen_clip_rect: Rect,
    visible_page_rect: Rect,
    page_to_screen: impl Fn(Pos2) -> Pos2,
) -> Option<Shape> {
    let mut strokes: Vec<HighlighterStrokeInput<'_>> = Vec::new();
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    smoothing_steps.hash(&mut hasher);

    for obj in &page.objects {
        if let DrawObject::Stroke {
            id,
            points,
            color,
            width,
            is_highlighter: true,
            style,
        } = obj
        {
            if points.is_empty() {
                continue;
            }
            let alpha = resolve_highlighter_alpha(color.a, Some(highlighter_opacity));
            let input = HighlighterStrokeInput {
                points,
                rgb: (color.r, color.g, color.b),
                width: *width,
                alpha,
                style: *style,
            };
            hash_stroke_input(&mut hasher, Some(id.as_str()), &input);
            strokes.push(input);
        }
    }

    if let Some(draft) = active_draft
        && !draft.points.is_empty()
    {
        let [dr, dg, db, _] = draft.stroke_color.to_srgba_unmultiplied();
        let d_alpha = highlighter_opacity_to_alpha(highlighter_opacity);
        let input = HighlighterStrokeInput {
            points: draft.points,
            rgb: (dr, dg, db),
            width: draft.page_width,
            alpha: d_alpha,
            style: draft.style,
        };
        hash_stroke_input(&mut hasher, None, &input);
        strokes.push(input);
    }

    if strokes.is_empty() {
        cache.remove_scope_page(scope, &page.id);
        return None;
    }

    let safe_zoom = zoom.max(1e-3);
    let hl_page_bounds = compute_strokes_page_bounds(&strokes, safe_zoom);

    let visible_screen_rect = page_screen_rect.intersect(screen_clip_rect);
    if !visible_screen_rect.is_positive() {
        return None;
    }

    let visible_hl_page_rect = hl_page_bounds.intersect(visible_page_rect);
    if !visible_hl_page_rect.is_positive() {
        return None;
    }

    let desired_ppp = if scope == HighlighterCacheScope::Thumbnail {
        1.0
    } else {
        ctx.pixels_per_point().clamp(1.0, 2.0)
    };
    let max_side = ctx
        .input(|i| i.max_texture_side)
        .clamp(256, HighlighterLayerCache::MAX_TEXTURE_SIDE);
    let base_scale = safe_zoom * desired_ppp;

    let full_w_px = (hl_page_bounds.width() * base_scale).ceil();
    let full_h_px = (hl_page_bounds.height() * base_scale).ceil();

    let (target_page_rect, raster_scale) =
        if full_w_px <= max_side as f32 && full_h_px <= max_side as f32 {
            (hl_page_bounds, base_scale)
        } else {
            let padded_vis = visible_hl_page_rect
                .expand(64.0 / safe_zoom)
                .intersect(hl_page_bounds);
            let req_w = (padded_vis.width() * base_scale).ceil().max(1.0);
            let req_h = (padded_vis.height() * base_scale).ceil().max(1.0);
            let fit_factor = (max_side as f32 / req_w.max(req_h)).min(1.0);
            (padded_vis, (base_scale * fit_factor).max(1e-3))
        };

    let content_hash = hasher.finish();
    let raster_scale_bits = raster_scale.to_bits();
    let cache_key = (scope, page.id.clone());

    cache.tick = cache.tick.wrapping_add(1);
    let current_tick = cache.tick;

    if let Some(entry) = cache.entries.get_mut(&cache_key)
        && entry.content_hash == content_hash
        && entry.raster_scale_bits == raster_scale_bits
        && rect_contains(&entry.page_rect, visible_hl_page_rect, 1e-3)
    {
        entry.last_used_tick = current_tick;
        let screen_rect = Rect::from_min_max(
            page_to_screen(entry.page_rect.min),
            page_to_screen(entry.page_rect.max),
        );
        return Some(Shape::image(
            entry.texture.id(),
            screen_rect,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            Color32::WHITE,
        ));
    }

    let tex_w = ((target_page_rect.width() * raster_scale).ceil() as usize).clamp(1, max_side);
    let tex_h = ((target_page_rect.height() * raster_scale).ceil() as usize).clamp(1, max_side);
    let aligned_page_rect = Rect::from_min_size(
        target_page_rect.min,
        Vec2::new(tex_w as f32 / raster_scale, tex_h as f32 / raster_scale),
    );

    let groups = group_highlighters_by_rgb(strokes);
    let color_image = composite_highlighter_groups_to_color_image(
        &groups,
        tex_w,
        tex_h,
        MaskTransform {
            origin_x: aligned_page_rect.min.x,
            origin_y: aligned_page_rect.min.y,
            scale: raster_scale,
        },
        smoothing_steps,
    );

    let tex_id = if let Some(entry) = cache.entries.get_mut(&cache_key) {
        entry.content_hash = content_hash;
        entry.raster_scale_bits = raster_scale_bits;
        entry.page_rect = aligned_page_rect;
        entry.last_used_tick = current_tick;
        entry.texture.set(color_image, TextureOptions::LINEAR);
        entry.texture.id()
    } else {
        let tex_name = format!("hl_{}_{}", scope.label(), page.id);
        let handle = ctx.load_texture(tex_name, color_image, TextureOptions::LINEAR);
        let tex_id = handle.id();
        cache.entries.insert(
            cache_key,
            HighlighterCacheEntry {
                content_hash,
                raster_scale_bits,
                page_rect: aligned_page_rect,
                texture: handle,
                last_used_tick: current_tick,
            },
        );
        cache.evict_lru_if_needed();
        tex_id
    };

    let screen_rect = Rect::from_min_max(
        page_to_screen(aligned_page_rect.min),
        page_to_screen(aligned_page_rect.max),
    );
    Some(Shape::image(
        tex_id,
        screen_rect,
        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
        Color32::WHITE,
    ))
}

fn hash_stroke_input(
    hasher: &mut impl Hasher,
    id: Option<&str>,
    stroke: &HighlighterStrokeInput<'_>,
) {
    id.hash(hasher);
    stroke.rgb.hash(hasher);
    stroke.alpha.hash(hasher);
    stroke.width.to_bits().hash(hasher);
    (stroke.style as u8).hash(hasher);
    stroke.points.len().hash(hasher);
    for pt in stroke.points {
        pt.x.to_bits().hash(hasher);
        pt.y.to_bits().hash(hasher);
    }
}

fn compute_strokes_page_bounds(strokes: &[HighlighterStrokeInput<'_>], zoom: f32) -> Rect {
    let mut min_x = f32::INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut max_y = f32::NEG_INFINITY;

    for s in strokes {
        let pad = stroke_radius_at_pressure(s.width, 1.0, true) + 2.0 / zoom.max(0.1);
        for pt in s.points {
            if pt.x.is_finite() && pt.y.is_finite() {
                min_x = min_x.min(pt.x - pad);
                min_y = min_y.min(pt.y - pad);
                max_x = max_x.max(pt.x + pad);
                max_y = max_y.max(pt.y + pad);
            }
        }
    }

    if !min_x.is_finite() || !min_y.is_finite() || !max_x.is_finite() || !max_y.is_finite() {
        Rect::NOTHING
    } else {
        Rect::from_min_max(Pos2::new(min_x, min_y), Pos2::new(max_x, max_y))
    }
}

#[inline]
fn rect_contains(outer: &Rect, inner: Rect, eps: f32) -> bool {
    outer.min.x <= inner.min.x + eps
        && outer.min.y <= inner.min.y + eps
        && outer.max.x >= inner.max.x - eps
        && outer.max.y >= inner.max.y - eps
}

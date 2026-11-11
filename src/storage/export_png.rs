use super::notebook_store::load_asset_from_notebook;
use crate::model::{Page, PageBackground};
use std::path::Path;

fn blend_pixel(img: &mut image::RgbaImage, x: i32, y: i32, color: [u8; 4]) {
    if x < 0 || y < 0 || x >= img.width() as i32 || y >= img.height() as i32 {
        return;
    }
    let alpha = color[3] as f32 / 255.0;
    if alpha <= 0.0 {
        return;
    }
    let dst = img.get_pixel_mut(x as u32, y as u32);
    if alpha >= 0.999 {
        *dst = image::Rgba([color[0], color[1], color[2], 255]);
    } else {
        let inv = 1.0 - alpha;
        let r = (color[0] as f32 * alpha + dst[0] as f32 * inv)
            .round()
            .clamp(0.0, 255.0) as u8;
        let g = (color[1] as f32 * alpha + dst[1] as f32 * inv)
            .round()
            .clamp(0.0, 255.0) as u8;
        let b = (color[2] as f32 * alpha + dst[2] as f32 * inv)
            .round()
            .clamp(0.0, 255.0) as u8;
        *dst = image::Rgba([r, g, b, 255]);
    }
}

fn draw_capsule_segment(
    img: &mut image::RgbaImage,
    a: eframe::egui::Pos2,
    b: eframe::egui::Pos2,
    radius: f32,
    color: [u8; 4],
) {
    let r = radius.max(0.6);
    let min_x = (a.x.min(b.x) - r - 1.0).floor().max(0.0) as i32;
    let max_x = (a.x.max(b.x) + r + 1.0)
        .ceil()
        .min(img.width() as f32 - 1.0) as i32;
    let min_y = (a.y.min(b.y) - r - 1.0).floor().max(0.0) as i32;
    let max_y = (a.y.max(b.y) + r + 1.0)
        .ceil()
        .min(img.height() as f32 - 1.0) as i32;

    for py in min_y..=max_y {
        for px in min_x..=max_x {
            let p = eframe::egui::Pos2::new(px as f32 + 0.5, py as f32 + 0.5);
            let d = crate::geometry::point_to_segment_dist(p, a, b);
            if d <= r + 0.5 {
                let coverage = (r + 0.5 - d).clamp(0.0, 1.0);
                let eff_alpha = (color[3] as f32 * coverage).round() as u8;
                blend_pixel(img, px, py, [color[0], color[1], color[2], eff_alpha]);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_egui_text(
    img: &mut image::RgbaImage,
    fonts: &eframe::egui::text::Fonts,
    x_pt: f32,
    y_pt: f32,
    text: &str,
    font_size_pt: f32,
    color: [u8; 4],
    scale: f32,
    rotation: f32,
) {
    if text.trim().is_empty() {
        return;
    }
    let font_id = eframe::egui::FontId::proportional(font_size_pt);
    let galley = fonts.layout_no_wrap(
        text.to_string(),
        font_id,
        eframe::egui::Color32::from_rgba_unmultiplied(color[0], color[1], color[2], color[3]),
    );

    let font_image = fonts.image();
    let atlas_w = font_image.width();
    let atlas_h = font_image.height();

    let cos_a = rotation.cos();
    let sin_a = rotation.sin();
    let is_rotated = rotation.abs() > 1e-4;

    for row in &galley.rows {
        for glyph in &row.glyphs {
            let uv_rect = glyph.uv_rect;
            if uv_rect.is_nothing() {
                continue;
            }
            let u_min = uv_rect.min[0] as usize;
            let v_min = uv_rect.min[1] as usize;
            let u_max = uv_rect.max[0] as usize;
            let v_max = uv_rect.max[1] as usize;

            let w = u_max.saturating_sub(u_min);
            let h = v_max.saturating_sub(v_min);

            for dy in 0..h {
                let v = v_min + dy;
                if v >= atlas_h {
                    continue;
                }
                for dx in 0..w {
                    let u = u_min + dx;
                    if u >= atlas_w {
                        continue;
                    }
                    let coverage = font_image[(u, v)];
                    if coverage > 0.005 {
                        let eff_alpha = ((color[3] as f32 * coverage).clamp(0.0, 255.0)) as u8;
                        let (px, py) = if is_rotated {
                            let lx = glyph.pos.x + uv_rect.offset.x + dx as f32;
                            let ly = glyph.pos.y + uv_rect.offset.y + dy as f32;
                            let rx = lx * cos_a - ly * sin_a;
                            let ry = lx * sin_a + ly * cos_a;
                            (
                                ((x_pt + rx) * scale).round() as i32,
                                ((y_pt + ry) * scale).round() as i32,
                            )
                        } else {
                            (
                                ((x_pt + glyph.pos.x + uv_rect.offset.x + dx as f32) * scale)
                                    .round() as i32,
                                ((y_pt + glyph.pos.y + uv_rect.offset.y + dy as f32) * scale)
                                    .round() as i32,
                            )
                        };
                        blend_pixel(img, px, py, [color[0], color[1], color[2], eff_alpha]);
                    }
                }
            }
        }
    }
}

fn is_stroke_dash_on(dist: f32, style: crate::model::StrokeStyle, w: f32) -> bool {
    match style {
        crate::model::StrokeStyle::Solid => true,
        crate::model::StrokeStyle::Dashed => {
            let dash = (w * 3.0).max(8.0);
            let gap = (w * 2.0).max(5.0);
            (dist % (dash + gap)) < dash
        }
        crate::model::StrokeStyle::DashDot => {
            let dash = (w * 3.0).max(8.0);
            let gap1 = (w * 1.5).max(4.0);
            let dot = (w * 0.8).max(2.5);
            let gap2 = gap1;
            let period = dash + gap1 + dot + gap2;
            let offset = dist % period;
            offset < dash || (offset >= (dash + gap1) && offset < (dash + gap1 + dot))
        }
        crate::model::StrokeStyle::Dotted => {
            let dot = (w * 0.6).max(2.0);
            let gap = (w * 1.5).max(4.0);
            (dist % (dot + gap)) < dot
        }
    }
}

pub fn export_page_to_png(
    dir: &Path,
    notebook_id: &str,
    page: &Page,
    output_path: &Path,
) -> Result<(), String> {
    let scale = 2.0_f32; // 2x resolution
    let width_px = (page.width * scale).round() as u32;
    let height_px = (page.height * scale).round() as u32;

    let mut img = image::RgbaImage::new(width_px, height_px);
    let fonts =
        eframe::egui::text::Fonts::new(scale, 4096, eframe::egui::FontDefinitions::default());

    // 1. Isi warna kertas dasar
    let bg_color = image::Rgba([
        page.paper_color.r,
        page.paper_color.g,
        page.paper_color.b,
        255,
    ]);
    for pixel in img.pixels_mut() {
        *pixel = bg_color;
    }

    // 2. Render latar belakang halaman (PDF / Gambar atau Pola Template Kertas)
    match page.effective_background() {
        PageBackground::Image { asset_id } | PageBackground::Pdf { asset_id, .. } => {
            if let Ok(bytes) = load_asset_from_notebook(dir, notebook_id, &asset_id)
                && let Ok(bg_img) = image::load_from_memory(&bytes)
            {
                let rotated_img = match page.rotation.rem_euclid(360) {
                    90 => bg_img.rotate90(),
                    180 => bg_img.rotate180(),
                    270 => bg_img.rotate270(),
                    _ => bg_img,
                };
                let resized = rotated_img.resize_exact(
                    width_px,
                    height_px,
                    image::imageops::FilterType::Lanczos3,
                );
                image::imageops::overlay(&mut img, &resized, 0, 0);
            }
        }
        PageBackground::Template(tmpl) => {
            let spacing = 30.0 * scale;
            let line_col = [220, 226, 231, 255];
            let w_f = width_px as f32;
            let h_f = height_px as f32;
            match tmpl {
                crate::model::PageTemplate::Blank => {}
                crate::model::PageTemplate::Ruled => {
                    let mut y = spacing;
                    while y < h_f {
                        draw_capsule_segment(
                            &mut img,
                            eframe::egui::Pos2::new(0.0, y),
                            eframe::egui::Pos2::new(w_f, y),
                            0.75,
                            line_col,
                        );
                        y += spacing;
                    }
                }
                crate::model::PageTemplate::RuledNarrow => {
                    let sp = 20.0 * scale;
                    let mut y = sp;
                    while y < h_f {
                        draw_capsule_segment(
                            &mut img,
                            eframe::egui::Pos2::new(0.0, y),
                            eframe::egui::Pos2::new(w_f, y),
                            0.65,
                            line_col,
                        );
                        y += sp;
                    }
                }
                crate::model::PageTemplate::RuledWide => {
                    let sp = 40.0 * scale;
                    let mut y = sp;
                    while y < h_f {
                        draw_capsule_segment(
                            &mut img,
                            eframe::egui::Pos2::new(0.0, y),
                            eframe::egui::Pos2::new(w_f, y),
                            0.8,
                            line_col,
                        );
                        y += sp;
                    }
                }
                crate::model::PageTemplate::Grid => {
                    let mut y = spacing;
                    while y < h_f {
                        draw_capsule_segment(
                            &mut img,
                            eframe::egui::Pos2::new(0.0, y),
                            eframe::egui::Pos2::new(w_f, y),
                            0.75,
                            line_col,
                        );
                        y += spacing;
                    }
                    let mut x = spacing;
                    while x < w_f {
                        draw_capsule_segment(
                            &mut img,
                            eframe::egui::Pos2::new(x, 0.0),
                            eframe::egui::Pos2::new(x, h_f),
                            0.75,
                            line_col,
                        );
                        x += spacing;
                    }
                }
                crate::model::PageTemplate::GridFine => {
                    let sp = 15.0 * scale;
                    let mut y = sp;
                    while y < h_f {
                        draw_capsule_segment(
                            &mut img,
                            eframe::egui::Pos2::new(0.0, y),
                            eframe::egui::Pos2::new(w_f, y),
                            0.55,
                            line_col,
                        );
                        y += sp;
                    }
                    let mut x = sp;
                    while x < w_f {
                        draw_capsule_segment(
                            &mut img,
                            eframe::egui::Pos2::new(x, 0.0),
                            eframe::egui::Pos2::new(x, h_f),
                            0.55,
                            line_col,
                        );
                        x += sp;
                    }
                }
                crate::model::PageTemplate::Dotted => {
                    let mut y = spacing;
                    while y < h_f {
                        let mut x = spacing;
                        while x < w_f {
                            let pt = eframe::egui::Pos2::new(x, y);
                            draw_capsule_segment(&mut img, pt, pt, 1.5, [204, 211, 218, 255]);
                            x += spacing;
                        }
                        y += spacing;
                    }
                }
                crate::model::PageTemplate::DottedFine => {
                    let sp = 15.0 * scale;
                    let mut y = sp;
                    while y < h_f {
                        let mut x = sp;
                        while x < w_f {
                            let pt = eframe::egui::Pos2::new(x, y);
                            draw_capsule_segment(&mut img, pt, pt, 1.0, [204, 211, 218, 255]);
                            x += sp;
                        }
                        y += sp;
                    }
                }
                crate::model::PageTemplate::Isometric => {
                    let dx = 30.0 * scale;
                    let dy = 30.0 * 0.8660254 * scale;
                    let dot_col = [204, 211, 218, 255];
                    let mut row = 0usize;
                    let mut y = dy;
                    while y < h_f {
                        let x_offset = if row % 2 == 1 { dx * 0.5 } else { 0.0 };
                        let mut x = x_offset + dx;
                        while x < w_f {
                            let pt = eframe::egui::Pos2::new(x, y);
                            draw_capsule_segment(&mut img, pt, pt, 1.2, dot_col);
                            x += dx;
                        }
                        y += dy;
                        row += 1;
                    }
                }
                crate::model::PageTemplate::Cornell => {
                    let summary_y = h_f * 0.84;
                    let cue_x = w_f * 0.28;
                    let mut y = spacing * 1.5;
                    while y < summary_y {
                        draw_capsule_segment(
                            &mut img,
                            eframe::egui::Pos2::new(0.0, y),
                            eframe::egui::Pos2::new(w_f, y),
                            0.75,
                            [230, 235, 240, 255],
                        );
                        y += spacing;
                    }
                    draw_capsule_segment(
                        &mut img,
                        eframe::egui::Pos2::new(cue_x, h_f * 0.04),
                        eframe::egui::Pos2::new(cue_x, summary_y),
                        1.4,
                        [200, 210, 220, 255],
                    );
                    draw_capsule_segment(
                        &mut img,
                        eframe::egui::Pos2::new(0.0, summary_y),
                        eframe::egui::Pos2::new(w_f, summary_y),
                        1.4,
                        [200, 210, 220, 255],
                    );
                }
            }
        }
    }

    // 3. Render seluruh objek halaman (Stroke, Shape, Image, StickyNote, Text, Connector)
    for obj in &page.objects {
        match obj {
            crate::model::DrawObject::Stroke {
                points,
                color,
                width,
                is_highlighter,
                style,
                ..
            } => {
                if points.is_empty() {
                    continue;
                }
                let smoothed = crate::geometry::smooth_stroke_points(points, 3);
                let rgba = if *is_highlighter {
                    [color.r, color.g, color.b, crate::model::HIGHLIGHTER_ALPHA]
                } else {
                    [color.r, color.g, color.b, color.a]
                };
                if smoothed.len() == 1 {
                    let p = eframe::egui::Pos2::new(smoothed[0].x * scale, smoothed[0].y * scale);
                    let r = (width * scale * 0.5 * smoothed[0].pressure).max(1.0);
                    draw_capsule_segment(&mut img, p, p, r, rgba);
                } else if *is_highlighter {
                    let r = (width * scale * 0.5).max(1.5);
                    let mut min_x = f32::INFINITY;
                    let mut min_y = f32::INFINITY;
                    let mut max_x = f32::NEG_INFINITY;
                    let mut max_y = f32::NEG_INFINITY;
                    for pt in &smoothed {
                        min_x = min_x.min(pt.x * scale);
                        min_y = min_y.min(pt.y * scale);
                        max_x = max_x.max(pt.x * scale);
                        max_y = max_y.max(pt.y * scale);
                    }
                    let px_min = (min_x - r - 1.0).floor().max(0.0) as i32;
                    let px_max = (max_x + r + 1.0).ceil().min(width_px as f32 - 1.0) as i32;
                    let py_min = (min_y - r - 1.0).floor().max(0.0) as i32;
                    let py_max = (max_y + r + 1.0).ceil().min(height_px as f32 - 1.0) as i32;

                    for py in py_min..=py_max {
                        for px in px_min..=px_max {
                            let p = eframe::egui::Pos2::new(px as f32 + 0.5, py as f32 + 0.5);
                            let mut min_d = f32::INFINITY;
                            for w in smoothed.windows(2) {
                                let a = eframe::egui::Pos2::new(w[0].x * scale, w[0].y * scale);
                                let b = eframe::egui::Pos2::new(w[1].x * scale, w[1].y * scale);
                                min_d = min_d.min(crate::geometry::point_to_segment_dist(p, a, b));
                            }
                            if min_d <= r {
                                blend_pixel(&mut img, px, py, rgba);
                            }
                        }
                    }
                } else if *style == crate::model::StrokeStyle::Solid {
                    for w in smoothed.windows(2) {
                        let a = eframe::egui::Pos2::new(w[0].x * scale, w[0].y * scale);
                        let b = eframe::egui::Pos2::new(w[1].x * scale, w[1].y * scale);
                        let avg_p = ((w[0].pressure + w[1].pressure) * 0.5).clamp(0.1, 1.0);
                        let r = (width * scale * 0.5 * (0.4 + 0.6 * avg_p)).max(0.8);
                        draw_capsule_segment(&mut img, a, b, r, rgba);
                    }
                } else {
                    let eff_w = width * scale;
                    let mut cum_dist = 0.0_f32;
                    for w in smoothed.windows(2) {
                        let a = eframe::egui::Pos2::new(w[0].x * scale, w[0].y * scale);
                        let b = eframe::egui::Pos2::new(w[1].x * scale, w[1].y * scale);
                        let seg_len = (b - a).length();
                        if seg_len < 0.001 {
                            continue;
                        }
                        let avg_p = ((w[0].pressure + w[1].pressure) * 0.5).clamp(0.1, 1.0);
                        let r = (eff_w * 0.5 * (0.4 + 0.6 * avg_p)).max(0.8);
                        let step_size = (r * 0.8).max(1.5);
                        let mut t = 0.0_f32;
                        while t < seg_len {
                            let next_t = (t + step_size).min(seg_len);
                            let mid = cum_dist + (t + next_t) * 0.5;
                            if is_stroke_dash_on(mid, *style, eff_w) {
                                let pa = a + (b - a) * (t / seg_len);
                                let pb = a + (b - a) * (next_t / seg_len);
                                draw_capsule_segment(&mut img, pa, pb, r, rgba);
                            }
                            t = next_t;
                        }
                        cum_dist += seg_len;
                    }
                }
            }
            crate::model::DrawObject::Shape {
                shape_type,
                x,
                y,
                width,
                height,
                color,
                stroke_width,
                fill,
                rotation,
                corner_radius,
                label,
                ..
            } => {
                let stroke_rgba = [color.r, color.g, color.b, color.a];
                let r_stroke = (stroke_width * scale * 0.5).max(1.0);
                let p1 = eframe::egui::Pos2::new(x * scale, y * scale);
                let p2 = eframe::egui::Pos2::new((x + width) * scale, (y + height) * scale);
                let rect = eframe::egui::Rect::from_two_pos(p1, p2);

                match shape_type {
                    crate::model::ShapeKind::Line => {
                        draw_capsule_segment(&mut img, p1, p2, r_stroke, stroke_rgba);
                    }
                    crate::model::ShapeKind::Ellipse => {
                        let center = rect.center();
                        let rx = (rect.width() * 0.5).max(1.0);
                        let ry = (rect.height() * 0.5).max(1.0);
                        let rot_rad = rotation.to_radians();
                        let cos_t = rot_rad.cos();
                        let sin_t = rot_rad.sin();
                        let max_r = rx.max(ry);
                        let px_min = (center.x - max_r - r_stroke - 1.0).floor().max(0.0) as i32;
                        let px_max = (center.x + max_r + r_stroke + 1.0)
                            .ceil()
                            .min(width_px as f32 - 1.0) as i32;
                        let py_min = (center.y - max_r - r_stroke - 1.0).floor().max(0.0) as i32;
                        let py_max = (center.y + max_r + r_stroke + 1.0)
                            .ceil()
                            .min(height_px as f32 - 1.0)
                            as i32;

                        let avg_r = (rx + ry) * 0.5;
                        for py in py_min..=py_max {
                            for px in px_min..=px_max {
                                let p = eframe::egui::Pos2::new(px as f32 + 0.5, py as f32 + 0.5);
                                let dx = p.x - center.x;
                                let dy = p.y - center.y;
                                let lx = dx * cos_t + dy * sin_t;
                                let ly = -dx * sin_t + dy * cos_t;
                                let norm_d = ((lx / rx).powi(2) + (ly / ry).powi(2)).sqrt();
                                if let Some(f) = fill
                                    && norm_d <= 1.0
                                {
                                    blend_pixel(&mut img, px, py, [f.r, f.g, f.b, f.a]);
                                }
                                let dist_to_edge = (norm_d - 1.0).abs() * avg_r;
                                if dist_to_edge <= r_stroke {
                                    blend_pixel(&mut img, px, py, stroke_rgba);
                                }
                            }
                        }
                    }
                    _ => {
                        let center = rect.center();
                        let verts: Vec<eframe::egui::Pos2> = if rotation.abs() > 0.001 {
                            let raw = crate::geometry::shape_vertices(
                                *shape_type,
                                *x * scale,
                                *y * scale,
                                *width * scale,
                                *height * scale,
                            );
                            let rot_rad = rotation.to_radians();
                            let cos_t = rot_rad.cos();
                            let sin_t = rot_rad.sin();
                            raw.into_iter()
                                .map(|v| {
                                    let dx = v.x - center.x;
                                    let dy = v.y - center.y;
                                    eframe::egui::Pos2::new(
                                        center.x + dx * cos_t - dy * sin_t,
                                        center.y + dx * sin_t + dy * cos_t,
                                    )
                                })
                                .collect()
                        } else {
                            crate::geometry::shape_vertices(
                                *shape_type,
                                *x * scale,
                                *y * scale,
                                *width * scale,
                                *height * scale,
                            )
                        };

                        if let Some(f) = fill
                            && verts.len() >= 3
                        {
                            let px_min = rect.min.x.floor().max(0.0) as i32;
                            let px_max = rect.max.x.ceil().min(width_px as f32 - 1.0) as i32;
                            let py_min = rect.min.y.floor().max(0.0) as i32;
                            let py_max = rect.max.y.ceil().min(height_px as f32 - 1.0) as i32;
                            let fill_rgba = [f.r, f.g, f.b, f.a];
                            for py in py_min..=py_max {
                                for px in px_min..=px_max {
                                    let p =
                                        eframe::egui::Pos2::new(px as f32 + 0.5, py as f32 + 0.5);
                                    if crate::geometry::point_in_polygon(p, &verts) {
                                        blend_pixel(&mut img, px, py, fill_rgba);
                                    }
                                }
                            }
                        }
                        for i in 0..verts.len() {
                            draw_capsule_segment(
                                &mut img,
                                verts[i],
                                verts[(i + 1) % verts.len()],
                                r_stroke,
                                stroke_rgba,
                            );
                        }
                    }
                }
                if let Some(lbl) = label
                    && !lbl.text.trim().is_empty()
                {
                    let (safe_w, safe_h) = crate::geometry::calculate_shape_text_bounds(
                        *shape_type,
                        *width,
                        *height,
                        *corner_radius,
                        *stroke_width,
                    );
                    let auto_font = crate::geometry::calculate_auto_fit_font_size(
                        lbl.font_size,
                        &lbl.text,
                        safe_w,
                        safe_h,
                        8.0,
                    );
                    let center_x = *x + *width / 2.0;
                    let center_y = *y + *height / 2.0;
                    let char_w = auto_font * 0.55;
                    let max_line_len = lbl
                        .text
                        .lines()
                        .map(|l| l.chars().count())
                        .max()
                        .unwrap_or(1)
                        .max(1) as f32;
                    let lines_cnt = lbl.text.lines().count().max(1) as f32;
                    let text_w = max_line_len * char_w;
                    let text_h = lines_cnt * auto_font * 1.25;
                    let tx = (center_x - text_w / 2.0).max(*x);
                    let ty = (center_y - text_h / 2.0).max(*y);

                    draw_egui_text(
                        &mut img,
                        &fonts,
                        tx,
                        ty,
                        &lbl.text,
                        auto_font,
                        [lbl.color.r, lbl.color.g, lbl.color.b, lbl.color.a],
                        scale,
                        *rotation,
                    );
                }
            }
            crate::model::DrawObject::Image {
                asset_id,
                x,
                y,
                width,
                height,
                rotation,
                ..
            } => {
                if let Ok(bytes) = load_asset_from_notebook(dir, notebook_id, asset_id)
                    && let Ok(obj_img) = image::load_from_memory(&bytes)
                {
                    let rot_deg = (rotation.to_degrees().round() as i32).rem_euclid(360);
                    let rotated = match rot_deg {
                        90 => obj_img.rotate90(),
                        180 => obj_img.rotate180(),
                        270 => obj_img.rotate270(),
                        _ => obj_img,
                    };
                    let cx = *x + *width / 2.0;
                    let cy = *y + *height / 2.0;
                    let (bb_w, bb_h) = if rot_deg == 90 || rot_deg == 270 {
                        (*height, *width)
                    } else {
                        (*width, *height)
                    };
                    let ox = ((cx - bb_w / 2.0) * scale).round() as i64;
                    let oy = ((cy - bb_h / 2.0) * scale).round() as i64;
                    let ow = (bb_w * scale).round().max(1.0) as u32;
                    let oh = (bb_h * scale).round().max(1.0) as u32;
                    let resized =
                        rotated.resize_exact(ow, oh, image::imageops::FilterType::Lanczos3);
                    image::imageops::overlay(&mut img, &resized, ox, oy);
                }
            }
            crate::model::DrawObject::StickyNote {
                x,
                y,
                width,
                height,
                text,
                color,
                ..
            } => {
                let x0 = (*x * scale).round().max(0.0) as i32;
                let y0 = (*y * scale).round().max(0.0) as i32;
                let x1 = ((*x + *width) * scale).round().min(width_px as f32 - 1.0) as i32;
                let y1 = ((*y + *height) * scale).round().min(height_px as f32 - 1.0) as i32;
                let fill_rgba = [color.r, color.g, color.b, color.a];
                for py in y0..=y1 {
                    for px in x0..=x1 {
                        blend_pixel(&mut img, px, py, fill_rgba);
                    }
                }
                let tl = eframe::egui::Pos2::new(x0 as f32, y0 as f32);
                let tr = eframe::egui::Pos2::new(x1 as f32, y0 as f32);
                let br = eframe::egui::Pos2::new(x1 as f32, y1 as f32);
                let bl = eframe::egui::Pos2::new(x0 as f32, y1 as f32);
                let border_col = [235, 208, 116, 255];
                draw_capsule_segment(&mut img, tl, tr, 1.2, border_col);
                draw_capsule_segment(&mut img, tr, br, 1.2, border_col);
                draw_capsule_segment(&mut img, br, bl, 1.2, border_col);
                draw_capsule_segment(&mut img, bl, tl, 1.2, border_col);
                draw_egui_text(
                    &mut img,
                    &fonts,
                    *x + 10.0,
                    *y + 10.0,
                    text,
                    14.0,
                    [40, 40, 40, 255],
                    scale,
                    0.0,
                );
            }
            crate::model::DrawObject::Text {
                x,
                y,
                text,
                font_size,
                color,
                rotation,
                ..
            } => {
                draw_egui_text(
                    &mut img,
                    &fonts,
                    *x,
                    *y,
                    text,
                    *font_size,
                    [color.r, color.g, color.b, color.a],
                    scale,
                    *rotation,
                );
            }
            crate::model::DrawObject::Connector { style, .. } => {
                if let Some(poly) = crate::geometry::connector_polyline(obj, &page.objects)
                    && poly.len() >= 2
                {
                    let stroke_rgba = [
                        style.stroke_color.r,
                        style.stroke_color.g,
                        style.stroke_color.b,
                        style.stroke_color.a,
                    ];
                    let r_stroke = (style.stroke_width * scale * 0.5).max(1.0);
                    for i in 0..poly.len() - 1 {
                        let p1 = eframe::egui::Pos2::new(poly[i].x * scale, poly[i].y * scale);
                        let p2 =
                            eframe::egui::Pos2::new(poly[i + 1].x * scale, poly[i + 1].y * scale);
                        draw_capsule_segment(&mut img, p1, p2, r_stroke, stroke_rgba);
                    }
                    if style.end_marker == crate::model::MarkerKind::Arrow {
                        let last = poly[poly.len() - 1];
                        let prev = poly[poly.len() - 2];
                        let dx = (last.x - prev.x) * scale;
                        let dy = (last.y - prev.y) * scale;
                        let len = (dx * dx + dy * dy).sqrt();
                        if len > 0.001 {
                            let dir_x = dx / len;
                            let dir_y = dy / len;
                            let perp_x = -dir_y;
                            let perp_y = dir_x;
                            let arrow_len = (style.stroke_width * 3.5 * scale).max(8.0 * scale);
                            let arrow_half_w = arrow_len * 0.5;
                            let tip = eframe::egui::Pos2::new(last.x * scale, last.y * scale);
                            let base_center =
                                tip - eframe::egui::Vec2::new(dir_x * arrow_len, dir_y * arrow_len);
                            let left = base_center
                                + eframe::egui::Vec2::new(
                                    perp_x * arrow_half_w,
                                    perp_y * arrow_half_w,
                                );
                            let right = base_center
                                - eframe::egui::Vec2::new(
                                    perp_x * arrow_half_w,
                                    perp_y * arrow_half_w,
                                );
                            draw_capsule_segment(&mut img, tip, left, r_stroke, stroke_rgba);
                            draw_capsule_segment(&mut img, tip, right, r_stroke, stroke_rgba);
                            draw_capsule_segment(&mut img, left, right, r_stroke, stroke_rgba);
                        }
                    }
                    if style.start_marker == crate::model::MarkerKind::Arrow {
                        let first = poly[0];
                        let next = poly[1];
                        let dx = (first.x - next.x) * scale;
                        let dy = (first.y - next.y) * scale;
                        let len = (dx * dx + dy * dy).sqrt();
                        if len > 0.001 {
                            let dir_x = dx / len;
                            let dir_y = dy / len;
                            let perp_x = -dir_y;
                            let perp_y = dir_x;
                            let arrow_len = (style.stroke_width * 3.5 * scale).max(8.0 * scale);
                            let arrow_half_w = arrow_len * 0.5;
                            let tip = eframe::egui::Pos2::new(first.x * scale, first.y * scale);
                            let base_center =
                                tip - eframe::egui::Vec2::new(dir_x * arrow_len, dir_y * arrow_len);
                            let left = base_center
                                + eframe::egui::Vec2::new(
                                    perp_x * arrow_half_w,
                                    perp_y * arrow_half_w,
                                );
                            let right = base_center
                                - eframe::egui::Vec2::new(
                                    perp_x * arrow_half_w,
                                    perp_y * arrow_half_w,
                                );
                            draw_capsule_segment(&mut img, tip, left, r_stroke, stroke_rgba);
                            draw_capsule_segment(&mut img, tip, right, r_stroke, stroke_rgba);
                            draw_capsule_segment(&mut img, left, right, r_stroke, stroke_rgba);
                        }
                    }
                }
            }
        }
    }

    img.save(output_path)
        .map_err(|e| format!("Gagal menyimpan berkas PNG: {e}"))
}

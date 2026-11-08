use eframe::egui::{
    self, Color32, CornerRadius, FontId, Painter, Pos2, Rect, Stroke, StrokeKind, Vec2,
};

pub fn draw_goodnotes_cover(
    painter: &Painter,
    rect: Rect,
    title: &str,
    cover_color: Color32,
    is_favorite: bool,
) {
    // 1. Cover body with rounded right edges:
    let rounding = CornerRadius {
        nw: 0,
        ne: 12,
        se: 12,
        sw: 0,
    };
    painter.rect_filled(rect, rounding, cover_color);

    // 2. Left spine strip (10px width) with light gradient
    let spine_rect = Rect::from_min_size(rect.left_top(), Vec2::new(10.0, rect.height()));
    painter.rect_filled(
        spine_rect,
        CornerRadius {
            nw: 0,
            ne: 0,
            se: 0,
            sw: 0,
        },
        Color32::from_rgba_premultiplied(255, 255, 255, 45),
    );
    painter.line_segment(
        [spine_rect.right_top(), spine_rect.right_bottom()],
        Stroke::new(1.0_f32, Color32::from_rgba_premultiplied(0, 0, 0, 60)),
    );

    // 3. Center Label Badge (white label badge)
    let badge_w = rect.width() * 0.72;
    let badge_h = 38.0;
    let badge_rect = Rect::from_center_size(
        Pos2::new(rect.center().x + 5.0, rect.top() + rect.height() * 0.35),
        Vec2::new(badge_w, badge_h),
    );
    painter.rect_filled(
        badge_rect,
        3.0,
        Color32::from_rgba_premultiplied(255, 255, 255, 220),
    );
    painter.rect_stroke(
        badge_rect,
        3.0,
        Stroke::new(1.0_f32, Color32::from_rgba_premultiplied(0, 0, 0, 40)),
        StrokeKind::Outside,
    );

    // Title inside badge
    let truncated_title = if title.chars().count() > 16 {
        format!("{}…", title.chars().take(15).collect::<String>())
    } else {
        title.to_string()
    };
    painter.text(
        badge_rect.center(),
        egui::Align2::CENTER_CENTER,
        truncated_title,
        FontId::proportional(12.0),
        Color32::from_rgb(30, 30, 30),
    );

    // 4. Subtle watermark lines at bottom right
    let watermark_rect = Rect::from_min_size(
        rect.right_bottom() - Vec2::new(38.0, 30.0),
        Vec2::new(28.0, 18.0),
    );
    for i in 0..4 {
        let y = watermark_rect.top() + (i as f32) * 5.0;
        painter.line_segment(
            [
                Pos2::new(watermark_rect.left(), y),
                Pos2::new(watermark_rect.right(), y),
            ],
            Stroke::new(1.0_f32, Color32::from_rgba_premultiplied(0, 0, 0, 30)),
        );
    }

    // 5. Gold Star if favorite
    if is_favorite {
        let star_pos = rect.right_top() + Vec2::new(-16.0, 16.0);
        painter.text(
            star_pos,
            egui::Align2::CENTER_CENTER,
            "★",
            FontId::proportional(16.0),
            Color32::from_rgb(255, 204, 0),
        );
    }
}

/// Fase 7: Muat ikon aplikasi dari berkas PNG tertanam
pub fn load_app_icon() -> Option<egui::IconData> {
    let icon_bytes = include_bytes!("../../assets/icon.png");
    if let Ok(img) = image::load_from_memory(icon_bytes) {
        let rgba = img.to_rgba8();
        let (width, height) = rgba.dimensions();
        Some(egui::IconData {
            rgba: rgba.into_raw(),
            width,
            height,
        })
    } else {
        None
    }
}

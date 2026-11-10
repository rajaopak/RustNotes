use crate::canvas::state::CanvasState;
use crate::canvas::types::{ActiveTool, ResizeCorner};
use crate::canvas::widgets::{render_floating_connector_toolbar, render_floating_shape_toolbar};
use crate::geometry::{
    AlignmentGuide, TransformHandleKind, combined_bounds, compute_transform_handles,
    connector_polyline, hit_test_connector_details, hit_test_connector_midpoint,
    hit_test_transform_handles, shape_connection_point,
};
use crate::model::{ConnectionSite, DrawObject, Page, Point2D, ShapeKind};
use eframe::egui::{self, Color32, CursorIcon, Painter, Pos2, Rect, Stroke, Ui, Vec2};
use std::collections::HashMap;

pub type QuickDiagramDot = (ConnectionSite, Pos2, Point2D);

/// Informasi data seleksi yang aktif pada halaman
#[derive(Clone, Debug)]
pub struct SelectionData {
    pub rect: Option<Rect>,
    pub single_line: Option<(Pos2, Pos2)>,
    pub single_shape: Option<(f32, f32, f32, f32, f32)>,
    pub single_connector: Option<(String, Option<Vec<Point2D>>)>,
    pub single_shape_quick_dots: Option<(String, Vec<QuickDiagramDot>)>,
    pub is_single_selected: bool,
    pub is_selection_fully_locked: bool,
    pub has_shapes_selected: bool,
    pub any_locked: bool,
    pub all_locked: bool,
}

impl SelectionData {
    pub fn compute(page: &Page, state: &CanvasState) -> Self {
        let selected: Vec<&DrawObject> = page
            .objects
            .iter()
            .filter(|o| state.selected_ids.contains(&o.id().to_string()))
            .collect();
        let rect = combined_bounds(&selected);
        let single_line = if selected.len() == 1 {
            if let DrawObject::Shape {
                shape_type: ShapeKind::Line,
                x,
                y,
                width,
                height,
                locked,
                ..
            } = selected[0]
            {
                if *locked {
                    None
                } else {
                    Some((Pos2::new(*x, *y), Pos2::new(*x + *width, *y + *height)))
                }
            } else {
                None
            }
        } else {
            None
        };
        let single_shape = if selected.len() == 1 {
            if let DrawObject::Shape {
                x,
                y,
                width,
                height,
                rotation,
                ..
            } = selected[0]
            {
                Some((*x, *y, *width, *height, *rotation))
            } else {
                None
            }
        } else {
            None
        };
        let single_connector = if selected.len() == 1 {
            if let DrawObject::Connector { id, .. } = selected[0] {
                Some((id.clone(), connector_polyline(selected[0], &page.objects)))
            } else {
                None
            }
        } else {
            None
        };
        let is_selection_fully_locked =
            !selected.is_empty() && selected.iter().all(|o| o.is_locked());
        let quick_dots = if selected.len() == 1
            && let DrawObject::Shape { id, locked, .. } = selected[0]
            && !*locked
        {
            let shape = selected[0];
            let sites = [
                ConnectionSite::Top,
                ConnectionSite::Right,
                ConnectionSite::Bottom,
                ConnectionSite::Left,
            ];
            let dots: Vec<(ConnectionSite, Pos2, Point2D)> = sites
                .iter()
                .map(|&site| {
                    let (pt, dir) = shape_connection_point(shape, site, None);
                    let dot_pt = pt + dir.to_vector() * 12.0;
                    (site, dot_pt.to_pos2(), pt)
                })
                .collect();
            Some((id.clone(), dots))
        } else {
            None
        };

        let has_shapes_selected = selected
            .iter()
            .any(|o| matches!(o, DrawObject::Shape { .. }));
        let any_locked = selected.iter().any(|o| o.is_locked());
        let all_locked = !selected.is_empty() && selected.iter().all(|o| o.is_locked());

        Self {
            rect,
            single_line,
            single_shape,
            single_connector,
            single_shape_quick_dots: quick_dots,
            is_single_selected: selected.len() == 1,
            is_selection_fully_locked,
            has_shapes_selected,
            any_locked,
            all_locked,
        }
    }
}

/// Render garis panduan magnetik (Alignment Guides)
pub fn render_alignment_guides(
    painter: &Painter,
    guides: &[AlignmentGuide],
    page_to_screen: impl Fn(Pos2) -> Pos2,
) {
    if guides.is_empty() {
        return;
    }
    let guide_stroke = Stroke::new(1.3_f32, Color32::from_rgb(235, 0, 140));
    for guide in guides {
        match guide.orientation {
            crate::geometry::GuideOrientation::Horizontal => {
                let y = guide.position;
                let p1 = page_to_screen(Pos2::new(guide.min_span, y));
                let p2 = page_to_screen(Pos2::new(guide.max_span, y));
                painter.line_segment([p1, p2], guide_stroke);
                painter.circle_filled(p1, 2.5, Color32::from_rgb(235, 0, 140));
                painter.circle_filled(p2, 2.5, Color32::from_rgb(235, 0, 140));
            }
            crate::geometry::GuideOrientation::Vertical => {
                let x = guide.position;
                let p1 = page_to_screen(Pos2::new(x, guide.min_span));
                let p2 = page_to_screen(Pos2::new(x, guide.max_span));
                painter.line_segment([p1, p2], guide_stroke);
                painter.circle_filled(p1, 2.5, Color32::from_rgb(235, 0, 140));
                painter.circle_filled(p2, 2.5, Color32::from_rgb(235, 0, 140));
            }
        }
    }
}

/// Merender gizmo seleksi, handle transformasi, dan toolbar mengambang terkait
#[allow(clippy::too_many_arguments)]
pub fn render_selection_gizmo(
    painter: &Painter,
    ui: &mut Ui,
    page: &mut Page,
    state: &mut CanvasState,
    sel: &SelectionData,
    canvas_rect: Rect,
    pointer_pos: Option<Pos2>,
    page_to_screen: impl Fn(Pos2) -> Pos2,
    screen_to_page: impl Fn(Pos2) -> Pos2,
) {
    if let Some((conn_id, Some(poly))) = &sel.single_connector {
        let is_conn_locked = page
            .objects
            .iter()
            .any(|o| o.id() == conn_id && o.is_locked());
        let handle_stroke = if is_conn_locked {
            Stroke::new(2.0_f32, Color32::from_rgb(140, 150, 165))
        } else {
            Stroke::new(2.0_f32, Color32::from_rgb(0, 114, 206))
        };

        let highlight_col = if is_conn_locked {
            Color32::from_rgba_unmultiplied(140, 150, 165, 50)
        } else {
            Color32::from_rgba_unmultiplied(0, 114, 206, 50)
        };
        for i in 0..poly.len() - 1 {
            let p1 = page_to_screen(poly[i].to_pos2());
            let p2 = page_to_screen(poly[i + 1].to_pos2());
            painter.line_segment([p1, p2], Stroke::new(6.0_f32, highlight_col));
        }

        if is_conn_locked {
            // Task 10.1: Visual lock badge 🔒 di titik tengah connector
            let mid_idx = poly.len() / 2;
            let lock_pos = page_to_screen(poly[mid_idx].to_pos2());
            painter.circle(
                lock_pos,
                11.0,
                Color32::WHITE,
                Stroke::new(1.8_f32, Color32::from_rgb(140, 150, 165)),
            );
            painter.text(
                lock_pos,
                egui::Align2::CENTER_CENTER,
                "🔒",
                egui::FontId::proportional(12.0),
                Color32::from_rgb(90, 100, 115),
            );
            if let Some(pos) = pointer_pos
                && (pos - lock_pos).length() <= 14.0
            {
                egui::show_tooltip(
                    ui.ctx(),
                    ui.layer_id(),
                    egui::Id::new("locked_conn_badge_tooltip"),
                    |ui| {
                        ui.label("🔒 Terkunci: Connector tidak dapat digeser atau diubah titik jalurnya. Tekan Ctrl+L atau gunakan toolbar untuk membuka kunci.");
                    },
                );
            }
        } else {
            // Start Handle di poly[0]
            let s_start = page_to_screen(poly[0].to_pos2());
            painter.circle(s_start, 6.5, Color32::WHITE, handle_stroke);
            painter.circle_filled(s_start, 3.0, Color32::from_rgb(0, 114, 206));

            // End Handle di poly[last]
            let s_end = page_to_screen(poly[poly.len() - 1].to_pos2());
            painter.circle(s_end, 6.5, Color32::WHITE, handle_stroke);
            painter.circle_filled(s_end, 3.0, Color32::from_rgb(0, 114, 206));

            // Waypoint handles di titik tikungan (Task 8.1, 8.3)
            for pt in poly.iter().take(poly.len() - 1).skip(1) {
                let s_cp = page_to_screen(pt.to_pos2());
                painter.circle(s_cp, 5.0, Color32::WHITE, handle_stroke);
                painter.circle_filled(s_cp, 2.5, Color32::from_rgb(0, 114, 206));
            }

            // Midpoint ghost handles untuk menambah tikungan baru (Task 8.1, 8.2)
            let mid_stroke =
                Stroke::new(1.2_f32, Color32::from_rgba_unmultiplied(0, 114, 206, 180));
            for i in 0..poly.len() - 1 {
                let mid = (poly[i] + poly[i + 1]) * 0.5;
                let s_mid = page_to_screen(mid.to_pos2());
                painter.circle(s_mid, 4.0, Color32::WHITE, mid_stroke);
                painter.circle_filled(s_mid, 1.5, Color32::from_rgb(0, 114, 206));
            }

            // Task 8.1 & 8.3: Tooltip & kursor pada handle connector saat hover
            if let Some(pos) = pointer_pos
                && state.resize_corner.is_none()
                && !state.is_dragging_selection
            {
                let page_p = screen_to_page(pos);
                let handle_dist = 14.0 / state.zoom;
                if let Some((_seg_idx, _mid_pt)) =
                    hit_test_connector_midpoint(Point2D::new(page_p.x, page_p.y), poly, handle_dist)
                {
                    ui.ctx().set_cursor_icon(CursorIcon::Crosshair);
                    egui::show_tooltip(
                        ui.ctx(),
                        ui.layer_id(),
                        egui::Id::new("connector_mid_tooltip"),
                        |ui| {
                            ui.label("Tambah Tikungan: Seret untuk menambah titik kontrol baru.");
                        },
                    );
                } else if let Some(target) =
                    hit_test_connector_details(Point2D::new(page_p.x, page_p.y), poly, handle_dist)
                {
                    match target {
                        crate::geometry::ConnectorHitTarget::StartHandle
                        | crate::geometry::ConnectorHitTarget::EndHandle => {
                            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                            egui::show_tooltip(
                                ui.ctx(),
                                ui.layer_id(),
                                egui::Id::new("connector_end_tooltip"),
                                |ui| {
                                    ui.label("Ujung Connector: Seret untuk menyambung atau melepas titik sambung.");
                                },
                            );
                        }
                        crate::geometry::ConnectorHitTarget::ControlPoint(_) => {
                            ui.ctx().set_cursor_icon(CursorIcon::Grab);
                            egui::show_tooltip(
                                ui.ctx(),
                                ui.layer_id(),
                                egui::Id::new("connector_cp_tooltip"),
                                |ui| {
                                    ui.label("Titik Kontrol: Seret untuk memindahkan jalur. Klik ganda untuk menghapus.");
                                },
                            );
                        }
                        crate::geometry::ConnectorHitTarget::LineSegment(_) => {
                            ui.ctx().set_cursor_icon(CursorIcon::Crosshair);
                        }
                    }
                }
            }
        }

        // Floating Toolbar untuk Connector Terpilih
        render_floating_connector_toolbar(
            ui,
            page,
            state,
            conn_id,
            poly,
            canvas_rect,
            &page_to_screen,
        );
    } else if let Some((line_start, line_end)) = sel.single_line {
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
    } else if let Some(b) = sel.rect {
        let (bx, by, bw, bh, brot) = if let Some(s) = sel.single_shape {
            s
        } else {
            (b.min.x, b.min.y, b.width(), b.height(), 0.0)
        };

        let transform_handles = compute_transform_handles(bx, by, bw, bh, brot, 20.0 / state.zoom);

        let mut handle_screen_map = HashMap::new();
        for h in &transform_handles {
            handle_screen_map.insert(h.kind, page_to_screen(h.pos.to_pos2()));
        }

        let s_nw = handle_screen_map[&TransformHandleKind::NorthWest];
        let s_ne = handle_screen_map[&TransformHandleKind::NorthEast];
        let s_se = handle_screen_map[&TransformHandleKind::SouthEast];
        let s_sw = handle_screen_map[&TransformHandleKind::SouthWest];
        let s_n = handle_screen_map[&TransformHandleKind::North];
        let s_e = handle_screen_map[&TransformHandleKind::East];
        let s_s = handle_screen_map[&TransformHandleKind::South];
        let s_w = handle_screen_map[&TransformHandleKind::West];
        let s_rot = handle_screen_map[&TransformHandleKind::Rotation];

        // Garis batas seleksi berotasi
        let box_stroke = if sel.all_locked {
            Stroke::new(1.5_f32, Color32::from_rgb(140, 150, 165))
        } else {
            Stroke::new(1.5_f32, Color32::from_rgb(0, 114, 206))
        };
        painter.line_segment([s_nw, s_ne], box_stroke);
        painter.line_segment([s_ne, s_se], box_stroke);
        painter.line_segment([s_se, s_sw], box_stroke);
        painter.line_segment([s_sw, s_nw], box_stroke);

        if sel.all_locked {
            // Task 10.1: Visual lock badge 🔒 di sudut kanan-atas bounding box
            let lock_badge_pos = s_ne + Vec2::new(10.0, -10.0);
            painter.circle(
                lock_badge_pos,
                11.0,
                Color32::WHITE,
                Stroke::new(1.8_f32, Color32::from_rgb(140, 150, 165)),
            );
            painter.text(
                lock_badge_pos,
                egui::Align2::CENTER_CENTER,
                "🔒",
                egui::FontId::proportional(12.0),
                Color32::from_rgb(90, 100, 115),
            );
            if let Some(pos) = pointer_pos
                && (pos - lock_badge_pos).length() <= 14.0
            {
                egui::show_tooltip(
                    ui.ctx(),
                    ui.layer_id(),
                    egui::Id::new("locked_shape_badge_tooltip"),
                    |ui| {
                        ui.label("🔒 Terkunci: Bentuk tidak dapat digeser atau diubah ukurannya. Tekan Ctrl+L atau gunakan toolbar untuk membuka kunci.");
                    },
                );
            }
        } else {
            // 4 Corner handles: Lingkaran putih di tiap sudut untuk scale proporsional
            let corner_stroke = Stroke::new(2.0_f32, Color32::from_rgb(0, 114, 206));
            for pos in [s_nw, s_ne, s_se, s_sw] {
                painter.circle(pos, 6.0, Color32::WHITE, corner_stroke);
            }

            // 4 Edge handles: Persegi biru di tengah sisi untuk resize satu sumbu
            for pos in [s_n, s_e, s_s, s_w] {
                let edge_rect = Rect::from_center_size(pos, Vec2::splat(9.0));
                painter.rect_filled(edge_rect, 1.5, Color32::from_rgb(0, 114, 206));
                painter.rect_stroke(
                    edge_rect,
                    1.5,
                    Stroke::new(1.0_f32, Color32::WHITE),
                    egui::StrokeKind::Outside,
                );
            }

            // 1 Rotation handle: Lingkaran putih menonjol di atas dengan ⟳
            painter.line_segment([s_n, s_rot], corner_stroke);
            painter.circle(s_rot, 7.5, Color32::WHITE, corner_stroke);
            painter.text(
                s_rot,
                egui::Align2::CENTER_CENTER,
                "⟳",
                egui::FontId::proportional(11.0),
                Color32::from_rgb(0, 114, 206),
            );
        }

        // Kursor dinamis saat hover di atas handle seleksi
        if let Some(pos) = pointer_pos {
            let page_p = screen_to_page(pos);
            if let Some(corner) = state.resize_corner {
                match corner {
                    ResizeCorner::Rotation => ui.ctx().set_cursor_icon(CursorIcon::Crosshair),
                    ResizeCorner::North | ResizeCorner::South => {
                        ui.ctx().set_cursor_icon(CursorIcon::ResizeVertical)
                    }
                    ResizeCorner::East | ResizeCorner::West => {
                        ui.ctx().set_cursor_icon(CursorIcon::ResizeHorizontal)
                    }
                    ResizeCorner::NorthWest | ResizeCorner::SouthEast => {
                        ui.ctx().set_cursor_icon(CursorIcon::ResizeNwSe)
                    }
                    ResizeCorner::NorthEast | ResizeCorner::SouthWest => {
                        ui.ctx().set_cursor_icon(CursorIcon::ResizeNeSw)
                    }
                    ResizeCorner::LineStart | ResizeCorner::LineEnd => {
                        ui.ctx().set_cursor_icon(CursorIcon::Crosshair)
                    }
                    ResizeCorner::ConnectorStart | ResizeCorner::ConnectorEnd => {
                        ui.ctx().set_cursor_icon(CursorIcon::PointingHand)
                    }
                    ResizeCorner::ConnectorControl(_) => ui.ctx().set_cursor_icon(CursorIcon::Grab),
                    ResizeCorner::QuickDiagramDot(_) => {
                        ui.ctx().set_cursor_icon(CursorIcon::PointingHand)
                    }
                }
            } else if state.is_dragging_selection {
                ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
            } else if matches!(state.current_tool, ActiveTool::Lasso | ActiveTool::Shape(_))
                && !sel.all_locked
            {
                let handle_dist = 14.0 / state.zoom;
                if let Some(h_kind) = hit_test_transform_handles(
                    &transform_handles,
                    Point2D::new(page_p.x, page_p.y),
                    handle_dist,
                ) {
                    match h_kind {
                        TransformHandleKind::Rotation => {
                            ui.ctx().set_cursor_icon(CursorIcon::Crosshair)
                        }
                        TransformHandleKind::North | TransformHandleKind::South => {
                            ui.ctx().set_cursor_icon(CursorIcon::ResizeVertical)
                        }
                        TransformHandleKind::East | TransformHandleKind::West => {
                            ui.ctx().set_cursor_icon(CursorIcon::ResizeHorizontal)
                        }
                        TransformHandleKind::NorthWest | TransformHandleKind::SouthEast => {
                            ui.ctx().set_cursor_icon(CursorIcon::ResizeNwSe)
                        }
                        TransformHandleKind::NorthEast | TransformHandleKind::SouthWest => {
                            ui.ctx().set_cursor_icon(CursorIcon::ResizeNeSw)
                        }
                    }
                } else if b.contains(page_p) {
                    ui.ctx().set_cursor_icon(CursorIcon::Grab);
                }
            }
        }

        // Locked Badge bila ada objek terkunci yang sedang dipilih
        if sel.any_locked {
            let lock_pos = s_ne + Vec2::new(12.0, -12.0);
            painter.circle_filled(lock_pos, 9.0, Color32::from_rgb(220, 53, 69));
            painter.text(
                lock_pos,
                egui::Align2::CENTER_CENTER,
                "🔒",
                egui::FontId::proportional(11.0),
                Color32::WHITE,
            );
        }

        // Panel Gaya Melayang untuk Bentuk
        if sel.has_shapes_selected {
            render_floating_shape_toolbar(
                ui,
                page,
                state,
                s_nw,
                s_ne,
                s_se,
                s_rot,
                canvas_rect,
                sel.all_locked,
            );
        }

        // Task 9.1: Visual feedback 4 titik tepi (Quick Diagramming edge dots)
        if let Some((_source_id, dots)) = &sel.single_shape_quick_dots {
            let dot_stroke = Stroke::new(1.8_f32, Color32::from_rgb(0, 114, 206));
            let hover_p = pointer_pos.map(&screen_to_page);
            for &(site, dpos, _) in dots {
                let s_dot = page_to_screen(dpos);
                let is_hovered = hover_p.is_some_and(|p| (p - dpos).length() <= 14.0 / state.zoom);

                if is_hovered {
                    painter.circle(
                        s_dot,
                        10.0,
                        Color32::from_rgba_unmultiplied(0, 114, 206, 40),
                        Stroke::NONE,
                    );
                    painter.circle(
                        s_dot,
                        8.5,
                        Color32::from_rgb(230, 242, 251),
                        Stroke::new(2.2_f32, Color32::from_rgb(0, 114, 206)),
                    );
                    painter.text(
                        s_dot,
                        egui::Align2::CENTER_CENTER,
                        "+",
                        egui::FontId::proportional(12.0),
                        Color32::from_rgb(0, 114, 206),
                    );
                    egui::show_tooltip(
                        ui.ctx(),
                        ui.layer_id(),
                        egui::Id::new(format!("quick_diagram_tooltip_{:?}", site)),
                        |ui| {
                            ui.label("Quick Diagramming: Ketuk untuk membuat bentuk terhubung. Seret untuk menarik konektor.");
                        },
                    );
                } else {
                    painter.circle(s_dot, 7.0, Color32::WHITE, dot_stroke);
                    painter.circle_filled(s_dot, 2.5, Color32::from_rgb(0, 114, 206));
                }
            }
        }
    }
}

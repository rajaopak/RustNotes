use super::editor_navbar::paint_rainbow_circle;
use super::state::RustNotesApp;
use super::types::ViewMode;
use crate::canvas::{ActiveTool, EraserMode};
use crate::model::{
    FloatingToolBarDock, FontFamilyPreset, LaserColorMode, LassoSelectionMode, ShapeKind,
    StrokeStyle, TextAlign,
};
use eframe::egui::{
    self, Button, Color32, CursorIcon, FontId, Pos2, Rect, RichText, Sense, Stroke, StrokeKind,
    Vec2,
};

const EDGE_MARGIN: f32 = 10.0;
const UNDO_PILL_CLEAR_X: f32 = 224.0;
const UNDO_PILL_CLEAR_Y: f32 = 52.0;
const ZOOM_PILL_COLLAPSED_X: f32 = 62.0;
const ZOOM_PILL_EXPANDED_X: f32 = 268.0;
const ZOOM_PILL_CLEAR_Y: f32 = 52.0;
const VERTICAL_BAR_INNER_WIDTH: f32 = 44.0;

/// Menghitung posisi koordinat kiri-atas (`Pos2`) Floating Quick Tool Bar pada keempat sisi kanvas
/// tanpa bertabrakan dengan panel Side Pages (`canvas_rect` sudah di luar sidebar), kapsul Undo/Page,
/// kapsul Zoom, maupun batas layar.
pub(crate) fn compute_floating_tool_bar_origin(
    dock: FloatingToolBarDock,
    canvas_rect: Rect,
    bar_size: Vec2,
    zoom_expanded: bool,
) -> Pos2 {
    let bw = bar_size
        .x
        .clamp(24.0, (canvas_rect.width() - EDGE_MARGIN * 2.0).max(24.0));
    let bh = bar_size
        .y
        .clamp(24.0, (canvas_rect.height() - EDGE_MARGIN * 2.0).max(24.0));

    match dock {
        FloatingToolBarDock::Top => {
            let safe_left = canvas_rect.left() + UNDO_PILL_CLEAR_X;
            let max_x =
                (canvas_rect.right() - bw - EDGE_MARGIN).max(canvas_rect.left() + EDGE_MARGIN);
            let ideal_x = canvas_rect.center().x - bw * 0.5;
            if safe_left <= max_x {
                Pos2::new(
                    ideal_x.clamp(safe_left, max_x),
                    canvas_rect.top() + EDGE_MARGIN,
                )
            } else {
                let fallback_y = (canvas_rect.top() + UNDO_PILL_CLEAR_Y).min(
                    (canvas_rect.bottom() - bh - EDGE_MARGIN).max(canvas_rect.top() + EDGE_MARGIN),
                );
                Pos2::new(
                    ideal_x.clamp(canvas_rect.left() + EDGE_MARGIN, max_x),
                    fallback_y,
                )
            }
        }
        FloatingToolBarDock::Bottom => {
            let zoom_clear_x = if zoom_expanded {
                ZOOM_PILL_EXPANDED_X
            } else {
                ZOOM_PILL_COLLAPSED_X
            };
            let safe_left = canvas_rect.left() + zoom_clear_x;
            let max_x =
                (canvas_rect.right() - bw - EDGE_MARGIN).max(canvas_rect.left() + EDGE_MARGIN);
            let ideal_x = canvas_rect.center().x - bw * 0.5;
            let bottom_y =
                (canvas_rect.bottom() - bh - EDGE_MARGIN).max(canvas_rect.top() + EDGE_MARGIN);
            if safe_left <= max_x {
                Pos2::new(ideal_x.clamp(safe_left, max_x), bottom_y)
            } else {
                let lifted_y = (canvas_rect.bottom() - ZOOM_PILL_CLEAR_Y - bh)
                    .max(canvas_rect.top() + EDGE_MARGIN);
                Pos2::new(
                    ideal_x.clamp(canvas_rect.left() + EDGE_MARGIN, max_x),
                    lifted_y,
                )
            }
        }
        FloatingToolBarDock::Left => {
            let x = canvas_rect.left() + EDGE_MARGIN;
            let corridor_top = canvas_rect.top() + UNDO_PILL_CLEAR_Y;
            let corridor_bot = (canvas_rect.bottom() - ZOOM_PILL_CLEAR_Y).max(corridor_top + bh);
            let ideal_y = ((corridor_top + corridor_bot) * 0.5) - bh * 0.5;
            let max_y = (corridor_bot - bh).max(corridor_top);
            let final_y = ideal_y.clamp(corridor_top, max_y).min(
                (canvas_rect.bottom() - bh - EDGE_MARGIN).max(canvas_rect.top() + EDGE_MARGIN),
            );
            Pos2::new(x, final_y)
        }
        FloatingToolBarDock::Right => {
            let x = (canvas_rect.right() - bw - EDGE_MARGIN).max(canvas_rect.left() + EDGE_MARGIN);
            let min_y = canvas_rect.top() + EDGE_MARGIN;
            let max_y = (canvas_rect.bottom() - bh - EDGE_MARGIN).max(min_y);
            let ideal_y = (canvas_rect.center().y - bh * 0.5).clamp(min_y, max_y);
            Pos2::new(x, ideal_y)
        }
    }
}

/// Menentukan sisi docking terdekat (`Top`, `Bottom`, `Left`, atau `Right`) dari posisi kursor terhadap kanvas.
pub(crate) fn nearest_dock_side(pointer: Pos2, canvas_rect: Rect) -> FloatingToolBarDock {
    let w = canvas_rect.width().max(1.0);
    let h = canvas_rect.height().max(1.0);
    let candidates = [
        (
            FloatingToolBarDock::Top,
            (pointer.y - canvas_rect.top()).max(0.0) / h,
        ),
        (
            FloatingToolBarDock::Bottom,
            (canvas_rect.bottom() - pointer.y).max(0.0) / h,
        ),
        (
            FloatingToolBarDock::Left,
            (pointer.x - canvas_rect.left()).max(0.0) / w,
        ),
        (
            FloatingToolBarDock::Right,
            (canvas_rect.right() - pointer.x).max(0.0) / w,
        ),
    ];
    candidates
        .into_iter()
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(dock, _)| dock)
        .unwrap_or(FloatingToolBarDock::Top)
}

#[derive(Clone, Copy)]
struct BarPalette {
    fill: Color32,
    stroke: Color32,
    fg: Color32,
    btn_bg: Color32,
    active_bg: Color32,
    muted: Color32,
}

impl BarPalette {
    fn from_ctx(ctx: &egui::Context, theme_mode: crate::model::ThemeMode) -> Self {
        match super::resolve_theme_mode(ctx, theme_mode) {
            crate::model::ThemeMode::Dark | crate::model::ThemeMode::System => Self {
                fill: Color32::from_rgba_unmultiplied(28, 31, 38, 240),
                stroke: Color32::from_rgb(68, 75, 88),
                fg: Color32::WHITE,
                btn_bg: Color32::from_rgb(46, 51, 62),
                active_bg: Color32::from_rgb(0, 114, 206),
                muted: Color32::from_rgb(165, 174, 188),
            },
            crate::model::ThemeMode::Light => Self {
                fill: Color32::from_rgba_unmultiplied(249, 250, 253, 245),
                stroke: Color32::from_rgb(196, 204, 216),
                fg: Color32::from_rgb(28, 32, 40),
                btn_bg: Color32::from_rgb(231, 236, 243),
                active_bg: Color32::from_rgb(0, 114, 206),
                muted: Color32::from_rgb(95, 104, 118),
            },
        }
    }
}

impl RustNotesApp {
    /// Merender Floating Quick Tool Bar kontekstual per-tool pada salah satu dari 4 sisi kanvas.
    pub(crate) fn render_floating_tool_bar(
        &mut self,
        ctx: &egui::Context,
        nb_idx: usize,
        canvas_rect: Rect,
    ) {
        if !self.settings.floating_tool_bar_visible || !canvas_rect.is_positive() {
            self.floating_tool_bar_last_rect = None;
            self.dragging_floating_tool_bar = false;
            self.floating_tool_bar_drag_preview = None;
            return;
        }

        let dock = self.settings.floating_tool_bar_dock;
        let is_horiz = dock.is_horizontal();
        let fallback_size = if is_horiz {
            Vec2::new(430.0, 38.0)
        } else {
            Vec2::new(56.0, 320.0)
        };
        let estimated_size = self
            .floating_tool_bar_last_rect
            .filter(|r| {
                let rect_horiz = r.width() >= r.height();
                rect_horiz == is_horiz
            })
            .map(|r| r.size())
            .unwrap_or(fallback_size);

        let origin = compute_floating_tool_bar_origin(
            dock,
            canvas_rect,
            estimated_size,
            self.show_zoom_slider_expanded,
        );
        let pal = BarPalette::from_ctx(ctx, self.settings.theme_mode);
        let lang = self.settings.language;

        let area_resp = egui::Area::new(egui::Id::new("rustnotes_floating_tool_bar"))
            .fixed_pos(origin)
            .order(egui::Order::Middle)
            .constrain_to(canvas_rect)
            .show(ctx, |ui| {
                egui::Frame::NONE
                    .fill(pal.fill)
                    .stroke(Stroke::new(1.0_f32, pal.stroke))
                    .corner_radius(14.0)
                    .shadow(egui::epaint::Shadow {
                        offset: [0, 4],
                        blur: 14,
                        spread: 0,
                        color: Color32::from_rgba_unmultiplied(0, 0, 0, 65),
                    })
                    .inner_margin(egui::Margin::symmetric(8, 6))
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(5.0, 5.0);
                        if !is_horiz {
                            ui.set_max_width(VERTICAL_BAR_INNER_WIDTH);
                        }
                        let mut render_contents = |ui: &mut egui::Ui| {
                            // 1. Handle seret untuk memindahkan posisi ke 4 sisi kanvas
                            let grip_text = if is_horiz { "⋮⋮" } else { "⋯" };
                            let grip_size = if is_horiz {
                                Vec2::new(18.0, 24.0)
                            } else {
                                Vec2::new(36.0, 18.0)
                            };
                            let (grip_rect, grip_resp) =
                                ui.allocate_exact_size(grip_size, Sense::click_and_drag());
                            ui.painter().rect_filled(
                                grip_rect,
                                6.0,
                                if grip_resp.hovered() || self.dragging_floating_tool_bar {
                                    pal.btn_bg
                                } else {
                                    Color32::TRANSPARENT
                                },
                            );
                            ui.painter().text(
                                grip_rect.center(),
                                egui::Align2::CENTER_CENTER,
                                grip_text,
                                FontId::proportional(13.0),
                                pal.muted,
                            );
                            if grip_resp.hovered() {
                                ctx.set_cursor_icon(CursorIcon::Grab);
                            }
                            if grip_resp.drag_started() || grip_resp.dragged() {
                                self.dragging_floating_tool_bar = true;
                                ctx.set_cursor_icon(CursorIcon::Grabbing);
                                let ptr = ctx
                                    .input(|i| i.pointer.hover_pos().or(i.pointer.interact_pos()));
                                self.floating_tool_bar_drag_preview =
                                    ptr.map(|p| nearest_dock_side(p, canvas_rect));
                            }
                            grip_resp.on_hover_text(lang.tr(
                                "Seret ke 4 sisi kanvas (Atas, Bawah, Kiri, Kanan) untuk mengubah tata letak Floating Bar",
                                "Drag to any of the 4 canvas edges (Top, Bottom, Left, Right) to reposition Floating Bar",
                            ));

                            ui.separator();

                            // 2. Pengaturan cepat kontekstual sesuai tool yang sedang aktif
                            self.render_floating_bar_tool_controls(ui, nb_idx, is_horiz, pal);

                            ui.separator();

                            // 3. Kontrol tata letak 4-sisi, akses setting lengkap, dan sembunyikan bar
                            self.render_floating_bar_management_controls(ui, is_horiz, pal);
                        };

                        if is_horiz {
                            ui.horizontal(|ui| {
                                render_contents(ui);
                            });
                        } else {
                            ui.vertical_centered(|ui| {
                                render_contents(ui);
                            });
                        }
                    });
            });

        self.floating_tool_bar_last_rect = Some(area_resp.response.rect);

        if self.dragging_floating_tool_bar {
            self.render_floating_dock_drop_zones(ctx, canvas_rect);
            let pointer_released = ctx.input(|i| !i.pointer.primary_down());
            if pointer_released {
                if let Some(target_dock) = self.floating_tool_bar_drag_preview.take()
                    && target_dock != self.settings.floating_tool_bar_dock
                {
                    self.settings.floating_tool_bar_dock = target_dock;
                    self.floating_tool_bar_last_rect = None;
                    self.persist_settings();
                }
                self.dragging_floating_tool_bar = false;
            }
        }
    }

    /// Menampilkan 4 zona target docking saat pengguna menyeret grip Floating Tool Bar.
    fn render_floating_dock_drop_zones(&self, ctx: &egui::Context, canvas_rect: Rect) {
        let lang = self.settings.language;
        let preview = self.floating_tool_bar_drag_preview;
        egui::Area::new(egui::Id::new("rustnotes_floating_bar_drop_zones"))
            .fixed_pos(canvas_rect.min)
            .order(egui::Order::Foreground)
            .interactable(false)
            .show(ctx, |ui| {
                ui.set_min_size(canvas_rect.size());
                let painter = ui.painter();
                let zones = [
                    (
                        FloatingToolBarDock::Top,
                        Rect::from_center_size(
                            Pos2::new(canvas_rect.center().x, canvas_rect.top() + 26.0),
                            Vec2::new((canvas_rect.width() * 0.34).clamp(120.0, 260.0), 28.0),
                        ),
                    ),
                    (
                        FloatingToolBarDock::Bottom,
                        Rect::from_center_size(
                            Pos2::new(canvas_rect.center().x, canvas_rect.bottom() - 26.0),
                            Vec2::new((canvas_rect.width() * 0.34).clamp(120.0, 260.0), 28.0),
                        ),
                    ),
                    (
                        FloatingToolBarDock::Left,
                        Rect::from_center_size(
                            Pos2::new(canvas_rect.left() + 26.0, canvas_rect.center().y),
                            Vec2::new(34.0, (canvas_rect.height() * 0.36).clamp(110.0, 240.0)),
                        ),
                    ),
                    (
                        FloatingToolBarDock::Right,
                        Rect::from_center_size(
                            Pos2::new(canvas_rect.right() - 26.0, canvas_rect.center().y),
                            Vec2::new(34.0, (canvas_rect.height() * 0.36).clamp(110.0, 240.0)),
                        ),
                    ),
                ];
                for (dock, zrect) in zones {
                    let is_hot = preview == Some(dock);
                    let fill = if is_hot {
                        Color32::from_rgba_unmultiplied(0, 114, 206, 150)
                    } else {
                        Color32::from_rgba_unmultiplied(40, 48, 64, 90)
                    };
                    let stroke = Stroke::new(
                        if is_hot { 2.0_f32 } else { 1.2_f32 },
                        if is_hot {
                            Color32::from_rgb(130, 205, 255)
                        } else {
                            Color32::from_rgba_unmultiplied(180, 195, 220, 160)
                        },
                    );
                    painter.rect(zrect, 10.0, fill, stroke, StrokeKind::Inside);
                    painter.text(
                        zrect.center(),
                        egui::Align2::CENTER_CENTER,
                        dock.label(lang),
                        FontId::proportional(11.5),
                        Color32::WHITE,
                    );
                }
            });
    }

    fn render_floating_bar_management_controls(
        &mut self,
        ui: &mut egui::Ui,
        _is_horiz: bool,
        pal: BarPalette,
    ) {
        let lang = self.settings.language;
        let dock_menu_btn = ui.menu_button(RichText::new("◰").size(13.5).color(pal.fg), |ui| {
            ui.label(
                RichText::new(lang.tr("Posisi Floating Bar:", "Dock Position:"))
                    .size(11.5)
                    .strong(),
            );
            for dock in FloatingToolBarDock::ALL {
                let icon = match dock {
                    FloatingToolBarDock::Top => "⬆",
                    FloatingToolBarDock::Bottom => "⬇",
                    FloatingToolBarDock::Left => "⬅",
                    FloatingToolBarDock::Right => "➡",
                };
                let is_sel = self.settings.floating_tool_bar_dock == dock;
                if ui
                    .selectable_label(is_sel, format!("{icon} {}", dock.label(lang)))
                    .clicked()
                {
                    self.settings.floating_tool_bar_dock = dock;
                    self.floating_tool_bar_last_rect = None;
                    self.persist_settings();
                    ui.close_menu();
                }
            }
        });
        dock_menu_btn.response.on_hover_text(lang.tr(
            "Ubah posisi Floating Bar (Atas / Bawah / Kiri / Kanan)",
            "Change Floating Bar dock side (Top / Bottom / Left / Right)",
        ));

        let settings_btn = Button::new(RichText::new("⚙").size(13.5).color(
            if self.show_tool_popup {
                Color32::WHITE
            } else {
                pal.fg
            },
        ))
        .fill(if self.show_tool_popup {
            pal.active_bg
        } else {
            pal.btn_bg
        })
        .corner_radius(7.0)
        .min_size(Vec2::new(24.0, 24.0));
        if ui
            .add(settings_btn)
            .on_hover_text(lang.tr(
                "Buka Pengaturan Lengkap Alat & Palet",
                "Open Full Tool & Palette Settings",
            ))
            .clicked()
        {
            self.show_tool_popup = !self.show_tool_popup;
            self.show_more_menu_popup = false;
            self.show_doc_title_popup = false;
        }

        let hide_btn = Button::new(RichText::new("✕").size(12.0).color(pal.muted))
            .fill(Color32::TRANSPARENT)
            .corner_radius(7.0)
            .min_size(Vec2::new(22.0, 24.0));
        if ui
            .add(hide_btn)
            .on_hover_text(lang.tr(
                "Sembunyikan Floating Tool Bar (klik ikon 🎛 di navbar untuk memunculkan lagi)",
                "Hide Floating Tool Bar (click 🎛 on navbar to show again)",
            ))
            .clicked()
        {
            self.settings.floating_tool_bar_visible = false;
            self.floating_tool_bar_last_rect = None;
            self.persist_settings();
        }
    }

    fn render_floating_bar_tool_controls(
        &mut self,
        ui: &mut egui::Ui,
        nb_idx: usize,
        is_horiz: bool,
        pal: BarPalette,
    ) {
        let lang = self.settings.language;
        match self.canvas_state.current_tool {
            ActiveTool::Pen | ActiveTool::Pencil => {
                self.render_quick_stroke_color_swatches(ui, is_horiz, pal, 6);
                ui.color_edit_button_srgba(&mut self.canvas_state.stroke_color);
                ui.separator();
                self.render_quick_width_presets(ui, is_horiz, pal);
                ui.separator();
                self.render_quick_stroke_style_buttons(ui, nb_idx, is_horiz, pal);
                ui.separator();
                let auto_btn = self.quick_toggle_chip(
                    ui,
                    "📐",
                    self.settings.auto_shape_enabled,
                    pal,
                    lang.tr(
                        "Otomatis Koreksi Bentuk (Hold-to-Snap)",
                        "Auto-convert Shapes",
                    ),
                );
                if auto_btn.clicked() {
                    self.settings.auto_shape_enabled = !self.settings.auto_shape_enabled;
                    self.canvas_state.auto_shape_enabled = self.settings.auto_shape_enabled;
                    self.persist_settings();
                }
            }
            ActiveTool::Highlighter => {
                let hl_palette: [Color32; 5] = [
                    Color32::from_rgb(255, 224, 58),
                    Color32::from_rgb(122, 235, 118),
                    Color32::from_rgb(90, 212, 255),
                    Color32::from_rgb(255, 138, 198),
                    Color32::from_rgb(255, 168, 72),
                ];
                self.render_color_circle_list(
                    ui,
                    &hl_palette,
                    self.canvas_state.stroke_color,
                    is_horiz,
                    |app, c| app.canvas_state.stroke_color = c,
                );
                ui.color_edit_button_srgba(&mut self.canvas_state.stroke_color);
                ui.separator();
                self.render_quick_width_presets(ui, is_horiz, pal);
                ui.separator();
                for (op, lbl) in [
                    (0.20_f32, "20%"),
                    (0.35, "35%"),
                    (0.50, "50%"),
                    (0.65, "65%"),
                ] {
                    let is_sel = (self.settings.highlighter_opacity - op).abs() < 0.04;
                    if self
                        .quick_pill_button(ui, lbl, is_sel, pal)
                        .on_hover_text(lang.tr("Opasitas Stabilo", "Highlighter Opacity"))
                        .clicked()
                    {
                        self.settings.highlighter_opacity = op;
                        self.canvas_state.highlighter_opacity = op;
                        self.canvas_state.highlighter_cache.clear();
                        self.persist_settings();
                    }
                }
                ui.separator();
                self.render_quick_stroke_style_buttons(ui, nb_idx, is_horiz, pal);
            }
            ActiveTool::Eraser => {
                for (mode, lbl, tip) in [
                    (
                        EraserMode::Partial,
                        "✂",
                        lang.tr("Partial: potong segmen garis", "Partial: erase segments"),
                    ),
                    (
                        EraserMode::WholeStroke,
                        "⌫",
                        lang.tr(
                            "Whole Stroke: hapus satu objek utuh",
                            "Whole Stroke: delete whole object",
                        ),
                    ),
                ] {
                    let is_sel = self.canvas_state.eraser_mode == mode;
                    if self.quick_toggle_chip(ui, lbl, is_sel, pal, tip).clicked() {
                        self.canvas_state.eraser_mode = mode;
                    }
                }
                ui.separator();
                for w in [2.0_f32, 5.0, 10.0, 18.0] {
                    let is_sel = (self.canvas_state.stroke_width - w).abs() < 0.4;
                    if self
                        .quick_pill_button(ui, &format!("{w:.0}"), is_sel, pal)
                        .on_hover_text(format!(
                            "{} {w:.0}px",
                            lang.tr("Ukuran penghapus", "Eraser size")
                        ))
                        .clicked()
                    {
                        self.canvas_state.stroke_width = w;
                    }
                }
                if self.quick_step_button(ui, "−", pal).clicked() {
                    self.canvas_state.stroke_width =
                        (self.canvas_state.stroke_width - 1.0).max(0.5);
                }
                ui.label(
                    RichText::new(format!("{:.0}px", self.canvas_state.stroke_width))
                        .size(11.0)
                        .color(pal.muted),
                );
                if self.quick_step_button(ui, "+", pal).clicked() {
                    self.canvas_state.stroke_width =
                        (self.canvas_state.stroke_width + 1.0).min(28.0);
                }
            }
            ActiveTool::Shape(cur_kind) => {
                let shapes = [
                    (ShapeKind::Rectangle, "▭"),
                    (ShapeKind::Ellipse, "◯"),
                    (ShapeKind::Line, "╱"),
                    (ShapeKind::Triangle, "△"),
                    (ShapeKind::RightTriangle, "◣"),
                    (ShapeKind::Parallelogram, "▱"),
                ];
                let container = |ui: &mut egui::Ui, add_contents: &mut dyn FnMut(&mut egui::Ui)| {
                    if is_horiz {
                        ui.horizontal(add_contents);
                    } else {
                        ui.horizontal_wrapped(add_contents);
                    }
                };
                container(ui, &mut |ui| {
                    for (kind, icon) in shapes {
                        let is_sel = cur_kind == kind;
                        if self
                            .quick_toggle_chip(
                                ui,
                                icon,
                                is_sel,
                                pal,
                                lang.tr("Pilih Bentuk", "Select Shape"),
                            )
                            .clicked()
                        {
                            self.canvas_state.current_tool = ActiveTool::Shape(kind);
                        }
                    }
                });
                ui.separator();
                self.render_quick_stroke_color_swatches(ui, is_horiz, pal, 5);
                ui.separator();
                self.render_quick_width_presets(ui, is_horiz, pal);
                ui.separator();
                self.render_quick_snap_toggles(ui, pal);
            }
            ActiveTool::Connector => {
                self.render_quick_stroke_color_swatches(ui, is_horiz, pal, 5);
                ui.separator();
                self.render_quick_width_presets(ui, is_horiz, pal);
                ui.separator();
                self.render_quick_snap_toggles(ui, pal);
            }
            ActiveTool::Lasso => {
                for mode in LassoSelectionMode::ALL {
                    let (icon, tip) = match mode {
                        LassoSelectionMode::Containment => (
                            "⬚",
                            lang.tr("Containment: lingkup penuh", "Containment selection"),
                        ),
                        LassoSelectionMode::Intersection => (
                            "✂",
                            lang.tr("Intersection: sentuh / iris", "Intersection selection"),
                        ),
                    };
                    let is_sel = self.canvas_state.lasso_selection_mode == mode;
                    if self.quick_toggle_chip(ui, icon, is_sel, pal, tip).clicked() {
                        self.canvas_state.lasso_selection_mode = mode;
                    }
                }
                ui.separator();
                self.render_quick_snap_toggles(ui, pal);
                if !self.canvas_state.selected_ids.is_empty() {
                    ui.separator();
                    if self
                        .quick_toggle_chip(ui, "📋", false, pal, lang.tr("Salin", "Copy"))
                        .clicked()
                    {
                        let page = &self.library.notebooks[nb_idx].pages[self.current_page_index];
                        self.canvas_state.copy_selected(&page.objects);
                    }
                    if self
                        .quick_toggle_chip(ui, "📄", false, pal, lang.tr("Gandakan", "Duplicate"))
                        .clicked()
                    {
                        let nb_id = self.library.notebooks[nb_idx].id.clone();
                        let page =
                            &mut self.library.notebooks[nb_idx].pages[self.current_page_index];
                        self.canvas_state.duplicate_selected(page);
                        self.canvas_state
                            .rebuild_spatial_grid(&page.id, &page.objects);
                        self.mark_dirty(&nb_id);
                    }
                    if self
                        .quick_toggle_chip(ui, "🗑", false, pal, lang.tr("Hapus", "Delete"))
                        .clicked()
                    {
                        let nb_id = self.library.notebooks[nb_idx].id.clone();
                        let page =
                            &mut self.library.notebooks[nb_idx].pages[self.current_page_index];
                        self.canvas_state.delete_selected(page);
                        self.canvas_state
                            .rebuild_spatial_grid(&page.id, &page.objects);
                        self.mark_dirty(&nb_id);
                    }
                }
            }
            ActiveTool::Text => {
                let text_cols: [Color32; 5] = [
                    Color32::from_rgb(35, 35, 35),
                    Color32::from_rgb(0, 114, 206),
                    Color32::from_rgb(220, 48, 56),
                    Color32::from_rgb(34, 154, 72),
                    Color32::from_rgb(130, 76, 163),
                ];
                self.render_color_circle_list(
                    ui,
                    &text_cols,
                    self.canvas_state.text_color,
                    is_horiz,
                    |app, c| app.canvas_state.text_color = c,
                );
                ui.separator();
                if self.quick_step_button(ui, "−", pal).clicked() {
                    self.canvas_state.text_font_size =
                        (self.canvas_state.text_font_size - 2.0).max(8.0);
                }
                ui.label(
                    RichText::new(format!("{:.0}pt", self.canvas_state.text_font_size))
                        .size(11.0)
                        .color(pal.fg),
                );
                if self.quick_step_button(ui, "+", pal).clicked() {
                    self.canvas_state.text_font_size =
                        (self.canvas_state.text_font_size + 2.0).min(96.0);
                }
                ui.separator();
                if self
                    .quick_toggle_chip(ui, "B", self.canvas_state.text_bold, pal, "Bold")
                    .clicked()
                {
                    self.canvas_state.text_bold = !self.canvas_state.text_bold;
                }
                if self
                    .quick_toggle_chip(ui, "I", self.canvas_state.text_italic, pal, "Italic")
                    .clicked()
                {
                    self.canvas_state.text_italic = !self.canvas_state.text_italic;
                }
                for (align, icon, tip) in [
                    (TextAlign::Left, "⬅", lang.tr("Rata Kiri", "Align Left")),
                    (
                        TextAlign::Center,
                        "⬌",
                        lang.tr("Rata Tengah", "Align Center"),
                    ),
                    (TextAlign::Right, "➡", lang.tr("Rata Kanan", "Align Right")),
                ] {
                    let is_sel = self.canvas_state.text_alignment == align;
                    if self.quick_toggle_chip(ui, icon, is_sel, pal, tip).clicked() {
                        self.canvas_state.text_alignment = align;
                    }
                }
                let ff_lbl = match self.canvas_state.text_font_family {
                    FontFamilyPreset::SansSerif => "Sans",
                    FontFamilyPreset::Serif => "Serif",
                    FontFamilyPreset::Monospace => "Mono",
                };
                if self
                    .quick_pill_button(ui, ff_lbl, false, pal)
                    .on_hover_text(lang.tr("Ganti jenis huruf", "Cycle font family"))
                    .clicked()
                {
                    self.canvas_state.text_font_family = match self.canvas_state.text_font_family {
                        FontFamilyPreset::SansSerif => FontFamilyPreset::Serif,
                        FontFamilyPreset::Serif => FontFamilyPreset::Monospace,
                        FontFamilyPreset::Monospace => FontFamilyPreset::SansSerif,
                    };
                }
            }
            ActiveTool::StickyNote => {
                let sticky_bgs: [Color32; 5] = [
                    Color32::from_rgb(254, 243, 189),
                    Color32::from_rgb(255, 209, 220),
                    Color32::from_rgb(198, 232, 255),
                    Color32::from_rgb(204, 245, 214),
                    Color32::from_rgb(229, 212, 255),
                ];
                self.render_color_circle_list(
                    ui,
                    &sticky_bgs,
                    self.canvas_state.sticky_bg_color,
                    is_horiz,
                    |app, c| app.canvas_state.sticky_bg_color = c,
                );
                ui.separator();
                if self.quick_step_button(ui, "−", pal).clicked() {
                    self.canvas_state.text_font_size =
                        (self.canvas_state.text_font_size - 2.0).clamp(10.0, 36.0);
                }
                ui.label(
                    RichText::new(format!(
                        "{:.0}pt",
                        self.canvas_state.text_font_size.clamp(10.0, 36.0)
                    ))
                    .size(11.0)
                    .color(pal.fg),
                );
                if self.quick_step_button(ui, "+", pal).clicked() {
                    self.canvas_state.text_font_size =
                        (self.canvas_state.text_font_size + 2.0).clamp(10.0, 36.0);
                }
                if self
                    .quick_toggle_chip(ui, "B", self.canvas_state.text_bold, pal, "Bold")
                    .clicked()
                {
                    self.canvas_state.text_bold = !self.canvas_state.text_bold;
                }
                if self
                    .quick_toggle_chip(ui, "I", self.canvas_state.text_italic, pal, "Italic")
                    .clicked()
                {
                    self.canvas_state.text_italic = !self.canvas_state.text_italic;
                }
            }
            ActiveTool::Equation => {
                if self
                    .quick_pill_button(
                        ui,
                        if is_horiz {
                            lang.tr("∑ Editor LaTeX", "∑ LaTeX Editor")
                        } else {
                            "∑"
                        },
                        self.canvas_state.show_equation_dialog,
                        pal,
                    )
                    .on_hover_text(lang.tr("Buka Editor & Preview LaTeX", "Open LaTeX Editor"))
                    .clicked()
                {
                    self.canvas_state.show_equation_dialog = true;
                    self.canvas_state.editing_equation_id = None;
                    self.canvas_state.equation_target_page_id = self.library.notebooks[nb_idx]
                        .pages
                        .get(self.current_page_index)
                        .map(|p| p.id.clone());
                }
                ui.separator();
                if self.quick_step_button(ui, "−", pal).clicked() {
                    self.canvas_state.equation_font_size =
                        (self.canvas_state.equation_font_size - 2.0).max(12.0);
                }
                ui.label(
                    RichText::new(format!("{:.0}pt", self.canvas_state.equation_font_size))
                        .size(11.0)
                        .color(pal.fg),
                );
                if self.quick_step_button(ui, "+", pal).clicked() {
                    self.canvas_state.equation_font_size =
                        (self.canvas_state.equation_font_size + 2.0).min(64.0);
                }
                ui.separator();
                let eq_cols: [Color32; 4] = [
                    Color32::from_rgb(35, 35, 35),
                    Color32::from_rgb(0, 114, 206),
                    Color32::from_rgb(220, 48, 56),
                    Color32::from_rgb(34, 154, 72),
                ];
                self.render_color_circle_list(
                    ui,
                    &eq_cols,
                    self.canvas_state.text_color,
                    is_horiz,
                    |app, c| app.canvas_state.text_color = c,
                );
            }
            ActiveTool::Laser => {
                let neon_cols: [Color32; 6] = [
                    Color32::from_rgb(255, 0, 35),
                    Color32::from_rgb(0, 212, 255),
                    Color32::from_rgb(24, 232, 90),
                    Color32::from_rgb(255, 213, 0),
                    Color32::from_rgb(176, 66, 255),
                    Color32::from_rgb(255, 46, 184),
                ];
                self.render_color_circle_list(
                    ui,
                    &neon_cols,
                    if self.canvas_state.laser_color_mode == LaserColorMode::Solid {
                        self.canvas_state.laser_color
                    } else {
                        Color32::TRANSPARENT
                    },
                    is_horiz,
                    |app, c| {
                        app.canvas_state.laser_color = c;
                        app.canvas_state.laser_color_mode = LaserColorMode::Solid;
                        app.settings.laser_color = c.into();
                        app.settings.laser_color_mode = LaserColorMode::Solid;
                        app.persist_settings();
                    },
                );
                let (rb_rect, rb_resp) =
                    ui.allocate_exact_size(Vec2::new(20.0, 20.0), Sense::click());
                let is_rb = self.canvas_state.laser_color_mode == LaserColorMode::Rainbow;
                let stroke = if is_rb {
                    Stroke::new(2.2_f32, Color32::from_rgb(76, 147, 236))
                } else {
                    Stroke::new(1.0_f32, Color32::from_rgb(110, 110, 115))
                };
                paint_rainbow_circle(&ui.painter_at(rb_rect), rb_rect.center(), 8.5, stroke);
                if rb_resp
                    .on_hover_text(lang.tr("Mode Rainbow (Pelangi)", "Rainbow Mode"))
                    .clicked()
                {
                    self.canvas_state.laser_color_mode = LaserColorMode::Rainbow;
                    self.settings.laser_color_mode = LaserColorMode::Rainbow;
                    self.persist_settings();
                }
            }
            ActiveTool::Hand => {
                let vm_lbl = match self.view_mode {
                    ViewMode::ContinuousVertical => "📜",
                    ViewMode::SinglePage => "📄",
                };
                if self
                    .quick_toggle_chip(
                        ui,
                        vm_lbl,
                        self.view_mode == ViewMode::ContinuousVertical,
                        pal,
                        lang.tr(
                            "Ganti Mode Halaman (Scroll / 1 Hal)",
                            "Toggle Page View Mode",
                        ),
                    )
                    .clicked()
                {
                    let next = match self.view_mode {
                        ViewMode::SinglePage => ViewMode::ContinuousVertical,
                        ViewMode::ContinuousVertical => ViewMode::SinglePage,
                    };
                    self.set_view_mode(nb_idx, next);
                }
                if self.quick_step_button(ui, "−", pal).clicked() {
                    let z =
                        (self.canvas_state.zoom - 0.1).max(crate::canvas::CanvasState::MIN_ZOOM);
                    self.apply_zoom_anchored(nb_idx, z, None);
                }
                if self
                    .quick_pill_button(
                        ui,
                        &format!("{}%", (self.canvas_state.zoom * 100.0).round() as i32),
                        false,
                        pal,
                    )
                    .on_hover_text("Reset Zoom 100%")
                    .clicked()
                {
                    self.apply_zoom_anchored(nb_idx, 1.0, None);
                    self.canvas_pan_offset = Vec2::ZERO;
                }
                if self.quick_step_button(ui, "+", pal).clicked() {
                    let z =
                        (self.canvas_state.zoom + 0.1).min(crate::canvas::CanvasState::MAX_ZOOM);
                    self.apply_zoom_anchored(nb_idx, z, None);
                }
                if self
                    .quick_toggle_chip(
                        ui,
                        "🔍+",
                        self.canvas_state.show_zoom_window,
                        pal,
                        lang.tr("Jendela Pembesar Tulis", "Zoom Window"),
                    )
                    .clicked()
                {
                    self.canvas_state.show_zoom_window = !self.canvas_state.show_zoom_window;
                }
            }
        }
    }

    fn render_quick_stroke_color_swatches(
        &mut self,
        ui: &mut egui::Ui,
        is_horiz: bool,
        _pal: BarPalette,
        max_count: usize,
    ) {
        let swatches: Vec<Color32> = self
            .settings
            .color_swatches
            .iter()
            .take(max_count)
            .map(crate::model::ColorData::to_color32)
            .collect();
        self.render_color_circle_list(
            ui,
            &swatches,
            self.canvas_state.stroke_color,
            is_horiz,
            |app, c| app.canvas_state.stroke_color = c,
        );
    }

    fn render_color_circle_list(
        &mut self,
        ui: &mut egui::Ui,
        colors: &[Color32],
        active_color: Color32,
        is_horiz: bool,
        mut on_select: impl FnMut(&mut Self, Color32),
    ) {
        let mut draw_circles = |ui: &mut egui::Ui| {
            for &c in colors {
                let (rect, resp) = ui.allocate_exact_size(Vec2::new(18.0, 18.0), Sense::click());
                let is_sel = active_color == c;
                let stroke = if is_sel {
                    Stroke::new(2.2_f32, Color32::from_rgb(76, 147, 236))
                } else {
                    Stroke::new(1.0_f32, Color32::from_rgb(120, 125, 135))
                };
                ui.painter_at(rect).circle(rect.center(), 7.5, c, stroke);
                if resp.clicked() {
                    on_select(self, c);
                }
            }
        };
        if is_horiz {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 3.0;
                draw_circles(ui);
            });
        } else {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(3.0, 3.0);
                draw_circles(ui);
            });
        }
    }

    fn render_quick_width_presets(&mut self, ui: &mut egui::Ui, is_horiz: bool, pal: BarPalette) {
        let lang = self.settings.language;
        let widths: Vec<f32> = self.settings.pen_widths.iter().copied().take(4).collect();
        let draw_widths = |ui: &mut egui::Ui| {
            for w in widths {
                let is_sel = (self.canvas_state.stroke_width - w).abs() < 0.1;
                if self
                    .quick_pill_button(ui, &format!("{w:.1}"), is_sel, pal)
                    .on_hover_text(format!(
                        "{} {w:.1}px",
                        lang.tr("Ketebalan garis", "Stroke width")
                    ))
                    .clicked()
                {
                    self.canvas_state.stroke_width = w;
                }
            }
            if is_horiz {
                if self.quick_step_button(ui, "−", pal).clicked() {
                    self.canvas_state.stroke_width =
                        (self.canvas_state.stroke_width - 0.5).max(0.5);
                }
                if self.quick_step_button(ui, "+", pal).clicked() {
                    self.canvas_state.stroke_width =
                        (self.canvas_state.stroke_width + 0.5).min(28.0);
                }
            }
        };
        if is_horiz {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 3.0;
                draw_widths(ui);
            });
        } else {
            ui.vertical_centered(|ui| {
                ui.spacing_mut().item_spacing.y = 3.0;
                draw_widths(ui);
            });
        }
    }

    fn render_quick_stroke_style_buttons(
        &mut self,
        ui: &mut egui::Ui,
        nb_idx: usize,
        is_horiz: bool,
        pal: BarPalette,
    ) {
        let lang = self.settings.language;
        let styles = [
            (StrokeStyle::Solid, "—"),
            (StrokeStyle::Dashed, "- -"),
            (StrokeStyle::DashDot, "-·-"),
            (StrokeStyle::Dotted, "···"),
        ];
        let mut draw_styles = |ui: &mut egui::Ui| {
            for (style, sym) in styles {
                let is_sel = self.canvas_state.stroke_style == style;
                if self
                    .quick_pill_button(ui, sym, is_sel, pal)
                    .on_hover_text(style.name_tr(lang))
                    .clicked()
                {
                    self.canvas_state.stroke_style = style;
                    if let Some(page) = self.library.notebooks[nb_idx]
                        .pages
                        .get_mut(self.current_page_index)
                    {
                        let mut changed = false;
                        for obj in &mut page.objects {
                            if self
                                .canvas_state
                                .selected_ids
                                .contains(&obj.id().to_string())
                                && let crate::model::DrawObject::Stroke {
                                    style: stroke_st, ..
                                } = obj
                            {
                                *stroke_st = style;
                                changed = true;
                            }
                        }
                        if changed {
                            self.canvas_state.mesh_cache.clear();
                            self.canvas_state.push_undo(&page.id, &page.objects);
                        }
                    }
                }
            }
        };
        if is_horiz {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 3.0;
                draw_styles(ui);
            });
        } else {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(2.0, 2.0);
                draw_styles(ui);
            });
        }
    }

    fn render_quick_snap_toggles(&mut self, ui: &mut egui::Ui, pal: BarPalette) {
        let lang = self.settings.language;
        if self
            .quick_toggle_chip(
                ui,
                "🧲",
                self.canvas_state.smart_guides,
                pal,
                lang.tr("Garis Pemandu Magnetik (Smart Guides)", "Smart Guides"),
            )
            .clicked()
        {
            self.canvas_state.smart_guides = !self.canvas_state.smart_guides;
            if !self.canvas_state.smart_guides {
                self.canvas_state.active_alignment_guides.clear();
            }
        }
        if self
            .quick_toggle_chip(
                ui,
                "▦",
                self.canvas_state.snap_to_grid,
                pal,
                lang.tr("Tempel ke Kisi (Snap to Grid)", "Snap to Grid"),
            )
            .clicked()
        {
            self.canvas_state.snap_to_grid = !self.canvas_state.snap_to_grid;
        }
    }

    fn quick_pill_button(
        &self,
        ui: &mut egui::Ui,
        label: &str,
        selected: bool,
        pal: BarPalette,
    ) -> egui::Response {
        let txt_col = if selected { Color32::WHITE } else { pal.fg };
        let bg = if selected { pal.active_bg } else { pal.btn_bg };
        ui.add(
            Button::new(RichText::new(label).size(11.0).strong().color(txt_col))
                .fill(bg)
                .corner_radius(6.0)
                .min_size(Vec2::new(26.0, 22.0)),
        )
    }

    fn quick_step_button(&self, ui: &mut egui::Ui, label: &str, pal: BarPalette) -> egui::Response {
        ui.add(
            Button::new(RichText::new(label).size(12.0).strong().color(pal.fg))
                .fill(pal.btn_bg)
                .corner_radius(6.0)
                .min_size(Vec2::new(20.0, 22.0)),
        )
    }

    fn quick_toggle_chip(
        &self,
        ui: &mut egui::Ui,
        label: &str,
        active: bool,
        pal: BarPalette,
        hover: &str,
    ) -> egui::Response {
        let txt_col = if active { Color32::WHITE } else { pal.fg };
        let bg = if active { pal.active_bg } else { pal.btn_bg };
        ui.add(
            Button::new(RichText::new(label).size(12.0).strong().color(txt_col))
                .fill(bg)
                .corner_radius(6.0)
                .min_size(Vec2::new(24.0, 22.0)),
        )
        .on_hover_text(hover)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Notebook, PageTemplate};

    #[test]
    fn test_floating_tool_bar_4_sides_never_collide_with_side_pages_undo_or_zoom_pills() {
        let side_panel_rect = Rect::from_min_max(Pos2::ZERO, Pos2::new(185.0, 800.0));
        let cases = [
            (
                "normal",
                Rect::from_min_max(Pos2::new(0.0, 48.0), Pos2::new(1280.0, 800.0)),
                false,
            ),
            (
                "with_side_pages_and_zoom_expanded",
                Rect::from_min_max(Pos2::new(185.0, 48.0), Pos2::new(1280.0, 800.0)),
                true,
            ),
            (
                "narrow_with_zoom_window",
                Rect::from_min_max(Pos2::new(185.0, 48.0), Pos2::new(720.0, 560.0)),
                true,
            ),
        ];

        for (name, canvas_rect, zoom_expanded) in cases {
            let undo_pill = Rect::from_min_size(
                canvas_rect.left_top() + Vec2::new(12.0, 10.0),
                Vec2::new(204.0, 34.0),
            );
            let zoom_pill = Rect::from_min_size(
                canvas_rect.left_bottom() + Vec2::new(12.0, -42.0),
                Vec2::new(if zoom_expanded { 246.0 } else { 42.0 }, 36.0),
            );

            for dock in FloatingToolBarDock::ALL {
                let sz = if dock.is_horizontal() {
                    Vec2::new(420.0, 38.0)
                } else {
                    Vec2::new(56.0, 300.0)
                };
                let origin = compute_floating_tool_bar_origin(dock, canvas_rect, sz, zoom_expanded);
                let bar_rect = Rect::from_min_size(origin, sz);

                assert!(
                    canvas_rect.contains_rect(bar_rect),
                    "{name}/{dock:?}: bar {bar_rect:?} must stay inside canvas {canvas_rect:?}"
                );
                assert!(
                    !bar_rect.intersects(undo_pill),
                    "{name}/{dock:?}: bar {bar_rect:?} overlaps undo pill {undo_pill:?}"
                );
                assert!(
                    !bar_rect.intersects(zoom_pill),
                    "{name}/{dock:?}: bar {bar_rect:?} overlaps zoom pill {zoom_pill:?}"
                );
                if canvas_rect.left() >= 185.0 {
                    assert!(
                        !bar_rect.intersects(side_panel_rect),
                        "{name}/{dock:?}: bar {bar_rect:?} overlaps side pages"
                    );
                }
            }
        }
    }

    #[test]
    fn test_nearest_dock_side_detects_all_four_edges() {
        let canvas = Rect::from_min_max(Pos2::new(185.0, 50.0), Pos2::new(1280.0, 800.0));
        assert_eq!(
            nearest_dock_side(Pos2::new(700.0, 75.0), canvas),
            FloatingToolBarDock::Top
        );
        assert_eq!(
            nearest_dock_side(Pos2::new(700.0, 775.0), canvas),
            FloatingToolBarDock::Bottom
        );
        assert_eq!(
            nearest_dock_side(Pos2::new(205.0, 420.0), canvas),
            FloatingToolBarDock::Left
        );
        assert_eq!(
            nearest_dock_side(Pos2::new(1255.0, 420.0), canvas),
            FloatingToolBarDock::Right
        );
    }

    #[test]
    fn test_floating_tool_bar_renders_all_tools_toggle_and_no_popup_collision() {
        let ctx = egui::Context::default();
        let mut app = RustNotesApp::default();
        let nb = Notebook::new("Floating Bar Test", PageTemplate::Blank, Color32::WHITE);
        app.active_notebook_id = Some(nb.id.clone());
        app.library.notebooks = vec![nb];
        app.settings.floating_tool_bar_visible = true;
        app.show_thumbnail_sidebar = true;

        let all_tools = [
            ActiveTool::Pen,
            ActiveTool::Pencil,
            ActiveTool::Highlighter,
            ActiveTool::Eraser,
            ActiveTool::Shape(ShapeKind::Rectangle),
            ActiveTool::Connector,
            ActiveTool::Lasso,
            ActiveTool::Text,
            ActiveTool::StickyNote,
            ActiveTool::Equation,
            ActiveTool::Laser,
            ActiveTool::Hand,
        ];

        for dock in FloatingToolBarDock::ALL {
            app.settings.floating_tool_bar_dock = dock;
            app.floating_tool_bar_last_rect = None;
            for tool in all_tools {
                app.canvas_state.current_tool = tool;
                let _ = ctx.run(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(
                            Pos2::ZERO,
                            Vec2::new(1280.0, 800.0),
                        )),
                        ..Default::default()
                    },
                    |ctx| {
                        app.render_editor_view(ctx);
                    },
                );
                let rect = app
                    .floating_tool_bar_last_rect
                    .expect("floating bar rect must be recorded when visible");
                assert!(
                    rect.left() >= 185.0,
                    "{dock:?}/{tool:?}: floating bar must never overlap open Side Pages (<185px), got left={}",
                    rect.left()
                );
            }
        }

        // Toggle sembunyikan bar
        app.settings.floating_tool_bar_visible = false;
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
                ..Default::default()
            },
            |ctx| {
                app.render_editor_view(ctx);
            },
        );
        assert!(app.floating_tool_bar_last_rect.is_none());

        // Nyalakan bar kembali bersamaan dengan pop-up & panel cari di kanan -> merender mulus tanpa tabrakan
        app.settings.floating_tool_bar_visible = true;
        app.settings.floating_tool_bar_dock = FloatingToolBarDock::Right;
        app.show_more_menu_popup = true;
        app.show_in_doc_search = true;
        let out = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
                ..Default::default()
            },
            |ctx| {
                app.render_editor_view(ctx);
            },
        );
        assert!(!out.shapes.is_empty());
        assert!(app.floating_tool_bar_last_rect.is_some());
    }
}

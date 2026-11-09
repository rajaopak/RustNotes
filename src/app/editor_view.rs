use super::state::RustNotesApp;
use super::theme::resolve_theme_mode;
use super::types::{ViewMode, get_windows_clipboard_sequence};
use crate::canvas::{ActiveTool, render_page_canvas, render_zoom_window_canvas};
use crate::model::{ShapeKind, ShortcutAction, ThemeMode};
use crate::storage::{app_storage_dir, save_settings_to_dir};
use eframe::egui::{
    self, Align, Button, Color32, Layout, Pos2, Rect, RichText, ScrollArea, Stroke, Vec2,
};
use std::time::Instant;

impl RustNotesApp {
    pub(crate) fn render_editor_view(&mut self, ctx: &egui::Context) {
        let notebook_id = match &self.active_notebook_id {
            Some(id) => id.clone(),
            None => return,
        };

        let nb_idx = match self
            .library
            .notebooks
            .iter()
            .position(|n| n.id == notebook_id)
        {
            Some(idx) => idx,
            None => {
                self.active_notebook_id = None;
                return;
            }
        };

        if let Some(target_pos) = self.canvas_state.request_paste_image.take() {
            if self.try_paste_clipboard_image(nb_idx, Some(target_pos)) {
                self.mark_dirty(&notebook_id);
            } else {
                self.toast_message = Some("⚠️ Tidak ada gambar di clipboard.".to_string());
            }
        }

        // --- Fase 4: Keyboard Shortcuts & Gestures ---
        let current_seq = get_windows_clipboard_sequence();
        if current_seq != self.win_clipboard_seq {
            self.win_clipboard_seq = current_seq;
            self.win_clipboard_timestamp = Instant::now();
        }

        // 1. Zoom gestures: Ctrl+Scroll or Touchpad Pinch (Zoom sesuai arah/posisi kursor mouse)
        let (zoom_delta, pointer_pos, raw_scroll_delta) = ctx.input(|i| {
            let zd = if i.modifiers.command && i.smooth_scroll_delta.y.abs() > 0.0 {
                // Ctrl+MouseWheel
                (i.smooth_scroll_delta.y * 0.002).clamp(-0.25, 0.25)
            } else if (i.zoom_delta() - 1.0).abs() > 0.001 {
                // Touchpad pinch
                i.zoom_delta() - 1.0
            } else {
                0.0
            };
            let ptr = i.pointer.hover_pos().or_else(|| i.pointer.latest_pos());
            let sd = if !i.modifiers.command {
                i.smooth_scroll_delta
            } else {
                Vec2::ZERO
            };
            (zd, ptr, sd)
        });
        if zoom_delta != 0.0 {
            let target_zoom = (self.canvas_state.zoom + zoom_delta).clamp(0.4, 2.5);
            self.apply_zoom_anchored(nb_idx, target_zoom, pointer_pos);
        }

        // Terapkan pergeseran dari Hand tool (`✋`) jika pengguna melakukan drag dengan Hand tool
        if self.canvas_state.pan_offset != Vec2::ZERO {
            let hand_delta = self.canvas_state.pan_offset;
            self.canvas_state.pan_offset = Vec2::ZERO;
            self.canvas_pan_offset.x += hand_delta.x;
            match self.view_mode {
                ViewMode::SinglePage => {
                    self.canvas_pan_offset.y += hand_delta.y;
                }
                ViewMode::ContinuousVertical => {
                    let new_sy = (self.canvas_scroll_offset.y - hand_delta.y).max(0.0);
                    self.canvas_scroll_offset.y = new_sy;
                    self.pending_scroll_offset = Some(Vec2::new(0.0, new_sy));
                }
            }
        }

        // Pada SinglePage mode, izinkan scroll roda mouse / touchpad menggeser kertas di dalam halaman tunggal
        // (tanpa berpindah halaman), dan pada ContinuousVertical izinkan scroll horizontal touchpad
        if raw_scroll_delta != Vec2::ZERO
            && !self.show_more_menu_popup
            && !self.show_tool_popup
            && !self.show_doc_title_popup
            && !self.show_page_resize_modal
            && let Some(ptr) = pointer_pos
            && self.last_canvas_viewport_rect.contains(ptr)
        {
            match self.view_mode {
                ViewMode::SinglePage => {
                    self.canvas_pan_offset += raw_scroll_delta;
                }
                ViewMode::ContinuousVertical => {
                    if raw_scroll_delta.x.abs() > 0.0 {
                        self.canvas_pan_offset.x += raw_scroll_delta.x;
                    }
                }
            }
        }

        // 2. Global editor shortcuts (active when not editing text field)
        let is_text_editing = self.show_in_doc_search
            || self.show_add_page_modal
            || self.show_save_template_modal
            || self.show_page_resize_modal
            || self.show_new_folder_modal
            || self.show_add_outline_modal
            || self.show_new_notebook_modal
            || self.canvas_state.editing_text_id.is_some();

        if !is_text_editing {
            // Help / Shortcuts cheat sheet: F1 or Shift+Slash (?)
            if ctx.input(|i| {
                i.key_pressed(egui::Key::F1)
                    || (i.modifiers.shift && i.key_pressed(egui::Key::Slash))
            }) {
                self.show_shortcut_modal = !self.show_shortcut_modal;
            }

            // In-document search
            if ctx.input(|i| {
                crate::app::is_action_triggered(ShortcutAction::Search, &self.settings.shortcuts, i)
            }) {
                self.show_in_doc_search = !self.show_in_doc_search;
                self.doc_search_current_match = 0;
            }

            // Save notebook immediately
            if ctx.input(|i| {
                crate::app::is_action_triggered(ShortcutAction::Save, &self.settings.shortcuts, i)
            }) {
                let nb_id = notebook_id.clone();
                self.force_save_notebook(&nb_id);
                self.toast_message = Some("💾 Dokumen berhasil disimpan.".to_string());
            }

            // Reset zoom to 100% & center pan
            if ctx.input(|i| {
                crate::app::is_action_triggered(
                    ShortcutAction::ResetZoom,
                    &self.settings.shortcuts,
                    i,
                )
            }) {
                self.apply_zoom_anchored(nb_idx, 1.0, None);
                self.canvas_pan_offset = Vec2::ZERO;
                self.toast_message = Some("🔍 Zoom di-reset ke 100%.".to_string());
            }

            // Select all objects on current page
            if ctx.input(|i| {
                crate::app::is_action_triggered(
                    ShortcutAction::SelectAll,
                    &self.settings.shortcuts,
                    i,
                )
            }) {
                let p = &self.library.notebooks[nb_idx].pages[self.current_page_index];
                self.canvas_state.selected_ids =
                    p.objects.iter().map(|o| o.id().to_string()).collect();
                self.canvas_state.current_tool = ActiveTool::Lasso;
            }

            // Shortcuts: Copy, Cut, Duplicate, Paste
            let is_copy = ctx.input(|i| {
                crate::app::is_action_triggered(ShortcutAction::Copy, &self.settings.shortcuts, i)
            }) || ctx.input_mut(|i| {
                i.consume_key(egui::Modifiers::COMMAND, egui::Key::C)
                    || i.events.iter().any(|e| matches!(e, egui::Event::Copy))
            });

            let is_cut = ctx.input(|i| {
                crate::app::is_action_triggered(ShortcutAction::Cut, &self.settings.shortcuts, i)
            }) || ctx.input_mut(|i| {
                i.consume_key(egui::Modifiers::COMMAND, egui::Key::X)
                    || i.events.iter().any(|e| matches!(e, egui::Event::Cut))
            });

            let is_duplicate = ctx.input(|i| {
                crate::app::is_action_triggered(
                    ShortcutAction::Duplicate,
                    &self.settings.shortcuts,
                    i,
                )
            }) || ctx
                .input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::D));

            let is_paste = ctx.input(|i| {
                crate::app::is_action_triggered(ShortcutAction::Paste, &self.settings.shortcuts, i)
            }) || ctx.input_mut(|i| {
                i.consume_key(egui::Modifiers::COMMAND, egui::Key::V)
                    || i.events.iter().any(|e| matches!(e, egui::Event::Paste(_)))
            });

            // Ctrl+C: Copy selected objects across any page of active notebook
            if is_copy {
                let mut copied = Vec::new();
                for page in &self.library.notebooks[nb_idx].pages {
                    for obj in &page.objects {
                        if self
                            .canvas_state
                            .selected_ids
                            .contains(&obj.id().to_string())
                        {
                            copied.push(obj.clone());
                        }
                    }
                }
                if !copied.is_empty() {
                    self.canvas_state.object_clipboard = copied;
                    self.canvas_state.app_clipboard_timestamp = Some(Instant::now());
                    self.toast_message = Some(format!(
                        "📋 {} objek disalin.",
                        self.canvas_state.object_clipboard.len()
                    ));
                }
            }

            // Ctrl+X: Cut selected objects
            if is_cut && !self.canvas_state.selected_ids.is_empty() {
                let p = &mut self.library.notebooks[nb_idx].pages[self.current_page_index];
                self.canvas_state.cut_selected(p);
                self.mark_dirty(&notebook_id);
                self.toast_message = Some(format!(
                    "✂ {} objek dipotong.",
                    self.canvas_state.object_clipboard.len()
                ));
            }

            // Ctrl+D: Duplicate selected objects
            if is_duplicate {
                let p = &mut self.library.notebooks[nb_idx].pages[self.current_page_index];
                self.canvas_state.duplicate_selected(p);
                self.mark_dirty(&notebook_id);
                self.toast_message = Some("📄 Objek digandakan.".to_string());
            }

            // Ctrl+V: Paste system clipboard image or paste object clipboard
            if is_paste {
                let app_time = self.canvas_state.app_clipboard_timestamp;
                let win_time = self.win_clipboard_timestamp;

                // Tentukan prioritas berdasarkan timestamp:
                // Jika objek aplikasi disalin LEBIH BARU daripada aksi copy di Windows:
                let app_is_newer = match app_time {
                    Some(t) => t >= win_time,
                    None => false,
                };

                let mut pasted = false;

                if app_is_newer && !self.canvas_state.object_clipboard.is_empty() {
                    let p = &mut self.library.notebooks[nb_idx].pages[self.current_page_index];
                    self.canvas_state.paste_clipboard(p, None);
                    self.mark_dirty(&notebook_id);
                    self.toast_message = Some("📋 Objek berhasil ditempel.".to_string());
                    pasted = true;
                } else {
                    // Windows clipboard lebih baru (atau belum pernah copy objek di aplikasi): coba tempel gambar
                    if self.try_paste_clipboard_image(nb_idx, None) {
                        self.mark_dirty(&notebook_id);
                        pasted = true;
                    } else if !self.canvas_state.object_clipboard.is_empty() {
                        // Jika tidak ada gambar di Windows clipboard, gunakan objek aplikasi
                        let p = &mut self.library.notebooks[nb_idx].pages[self.current_page_index];
                        self.canvas_state.paste_clipboard(p, None);
                        self.mark_dirty(&notebook_id);
                        self.toast_message = Some("📋 Objek berhasil ditempel.".to_string());
                        pasted = true;
                    }
                }

                if !pasted {
                    self.toast_message =
                        Some("⚠️ Tidak ada gambar atau objek untuk ditempel.".to_string());
                }
            }

            // Delete / Backspace: Delete selected objects
            if ctx.input(|i| {
                !self.canvas_state.selected_ids.is_empty()
                    && (crate::app::is_action_triggered(
                        ShortcutAction::Delete,
                        &self.settings.shortcuts,
                        i,
                    ) || i.key_pressed(egui::Key::Delete)
                        || i.key_pressed(egui::Key::Backspace))
            }) {
                let p = &mut self.library.notebooks[nb_idx].pages[self.current_page_index];
                self.canvas_state.delete_selected(p);
                self.mark_dirty(&notebook_id);
                self.toast_message = Some("🗑 Objek dihapus.".to_string());
            }

            // Lock / Unlock selected shape(s) & connector(s)
            if ctx.input(|i| {
                crate::app::is_action_triggered(
                    ShortcutAction::ToggleLock,
                    &self.settings.shortcuts,
                    i,
                )
            }) {
                let (toggled, is_locked) = {
                    let p = &mut self.library.notebooks[nb_idx].pages[self.current_page_index];
                    let ok = self.canvas_state.toggle_lock_selected(p);
                    let locked = self.canvas_state.is_selected_locked(p);
                    (ok, locked)
                };
                if toggled {
                    self.mark_dirty(&notebook_id);
                    self.toast_message = Some(if is_locked {
                        "🔒 Objek dikunci (Locked).".to_string()
                    } else {
                        "🔓 Objek dibuka kuncinya (Unlocked).".to_string()
                    });
                }
            }

            // Toggle Smart Alignment Guides
            if ctx.input(|i| {
                crate::app::is_action_triggered(
                    ShortcutAction::ToggleGrid,
                    &self.settings.shortcuts,
                    i,
                )
            }) {
                self.canvas_state.smart_guides = !self.canvas_state.smart_guides;
                let active = self.canvas_state.smart_guides;
                self.toast_message = Some(if active {
                    "🧲 Smart Alignment Guides: AKTIF".to_string()
                } else {
                    "🧲 Smart Alignment Guides: NONAKTIF".to_string()
                });
            }

            // Undo
            if ctx.input(|i| {
                crate::app::is_action_triggered(ShortcutAction::Undo, &self.settings.shortcuts, i)
            }) {
                self.perform_undo(nb_idx);
            }

            // Redo
            if ctx.input(|i| {
                crate::app::is_action_triggered(ShortcutAction::Redo, &self.settings.shortcuts, i)
                    || (i.modifiers.command && i.modifiers.shift && i.key_pressed(egui::Key::Z))
            }) {
                self.perform_redo(nb_idx);
            }

            // Tool shortcuts (dapat disesuaikan lewat Settings)
            ctx.input(|i| {
                if crate::app::is_action_triggered(
                    ShortcutAction::ToolPen,
                    &self.settings.shortcuts,
                    i,
                ) {
                    self.canvas_state.current_tool = ActiveTool::Pen;
                } else if crate::app::is_action_triggered(
                    ShortcutAction::ToolHighlighter,
                    &self.settings.shortcuts,
                    i,
                ) {
                    self.canvas_state.current_tool = ActiveTool::Highlighter;
                } else if crate::app::is_action_triggered(
                    ShortcutAction::ToolEraser,
                    &self.settings.shortcuts,
                    i,
                ) {
                    self.canvas_state.current_tool = ActiveTool::Eraser;
                } else if crate::app::is_action_triggered(
                    ShortcutAction::ToolLasso,
                    &self.settings.shortcuts,
                    i,
                ) {
                    self.canvas_state.current_tool = ActiveTool::Lasso;
                } else if crate::app::is_action_triggered(
                    ShortcutAction::ToolText,
                    &self.settings.shortcuts,
                    i,
                ) {
                    self.canvas_state.current_tool = ActiveTool::Text;
                } else if crate::app::is_action_triggered(
                    ShortcutAction::ToolShape,
                    &self.settings.shortcuts,
                    i,
                ) {
                    self.canvas_state.current_tool = ActiveTool::Shape(ShapeKind::Rectangle);
                } else if crate::app::is_action_triggered(
                    ShortcutAction::ToolConnector,
                    &self.settings.shortcuts,
                    i,
                ) {
                    self.canvas_state.current_tool = ActiveTool::Connector;
                } else if crate::app::is_action_triggered(
                    ShortcutAction::ToolStickyNote,
                    &self.settings.shortcuts,
                    i,
                ) {
                    self.canvas_state.current_tool = ActiveTool::StickyNote;
                }
            });

            // Menggeser kanvas menggunakan Arrow Keys (ArrowUp, ArrowDown, ArrowLeft, ArrowRight)
            // ketika tidak sedang menekan Alt / Ctrl / Command
            let arrow_pan = ctx.input(|i| {
                if !i.modifiers.alt && !i.modifiers.command && !i.modifiers.ctrl {
                    let step = if i.modifiers.shift { 96.0 } else { 48.0 };
                    let mut d = Vec2::ZERO;
                    if i.key_pressed(egui::Key::ArrowLeft) {
                        d.x += step;
                    }
                    if i.key_pressed(egui::Key::ArrowRight) {
                        d.x -= step;
                    }
                    if i.key_pressed(egui::Key::ArrowUp) {
                        d.y += step;
                    }
                    if i.key_pressed(egui::Key::ArrowDown) {
                        d.y -= step;
                    }
                    d
                } else {
                    Vec2::ZERO
                }
            });
            if arrow_pan != Vec2::ZERO {
                self.canvas_pan_offset.x += arrow_pan.x;
                match self.view_mode {
                    ViewMode::SinglePage => {
                        self.canvas_pan_offset.y += arrow_pan.y;
                    }
                    ViewMode::ContinuousVertical => {
                        if arrow_pan.y != 0.0 {
                            let new_sy = (self.canvas_scroll_offset.y - arrow_pan.y).max(0.0);
                            self.canvas_scroll_offset.y = new_sy;
                            self.pending_scroll_offset = Some(Vec2::new(0.0, new_sy));
                        }
                    }
                }
            }

            // Fase 8.9: Navigasi Keyboard Menyeluruh (Halaman, Panel, Zoom Window, Escape)
            let page_count = self.library.notebooks[nb_idx].pages.len();
            if ctx.input(|i| {
                i.key_pressed(egui::Key::PageDown)
                    || (i.modifiers.alt && i.key_pressed(egui::Key::ArrowRight))
            }) && self.current_page_index + 1 < page_count
            {
                let next_idx = self.current_page_index + 1;
                self.navigate_to_page(nb_idx, next_idx);
            }
            if ctx.input(|i| {
                i.key_pressed(egui::Key::PageUp)
                    || (i.modifiers.alt && i.key_pressed(egui::Key::ArrowLeft))
            }) && self.current_page_index > 0
            {
                let prev_idx = self.current_page_index - 1;
                self.navigate_to_page(nb_idx, prev_idx);
            }
            if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Home)) {
                self.navigate_to_page(nb_idx, 0);
            }
            if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::End)) && page_count > 0
            {
                self.navigate_to_page(nb_idx, page_count - 1);
            }
            if ctx.input(|i| {
                crate::app::is_action_triggered(
                    ShortcutAction::ToggleSidebar,
                    &self.settings.shortcuts,
                    i,
                ) || i.key_pressed(egui::Key::F6)
                    || (i.modifiers.command && i.key_pressed(egui::Key::B))
            }) {
                self.show_thumbnail_sidebar = !self.show_thumbnail_sidebar;
            }
            if ctx.input(|i| {
                crate::app::is_action_triggered(
                    ShortcutAction::ToggleZoomWindow,
                    &self.settings.shortcuts,
                    i,
                ) || (i.modifiers.alt && i.key_pressed(egui::Key::Z))
            }) {
                self.canvas_state.show_zoom_window = !self.canvas_state.show_zoom_window;
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.show_shortcut_modal = false;
                self.show_in_doc_search = false;
                self.show_ocr_modal = false;
                self.show_more_menu_popup = false;
                self.show_tool_popup = false;
                self.show_doc_title_popup = false;
                self.canvas_state.selected_ids.clear();
            }
        }

        // Navbar & Popups
        if !self.render_editor_navbar(ctx, nb_idx) {
            return;
        }

        // Sidebar Panel (Pages / Bookmarks / Outline)
        self.render_editor_sidebar(ctx, nb_idx);
        // Fase 8.3: Floating / Bottom Zoom Window Panel for detailed writing with auto-advance
        if self.canvas_state.show_zoom_window {
            let scale = self.settings.zoom_window_scale.clamp(1.5, 4.0);
            let panel_height = self.canvas_state.zoom_target_rect.height() * scale + 48.0;
            egui::TopBottomPanel::bottom("zoom_window_panel")
                .exact_height(panel_height)
                .show(ctx, |ui| {
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.add_space(12.0);
                        ui.label(
                            RichText::new("🔍 Zoom Window (Auto-Advance)")
                                .strong()
                                .size(12.0),
                        );
                        ui.separator();
                        let cur_idx = self.current_page_index;
                        let (pw, ph) = {
                            let p = &self.library.notebooks[nb_idx].pages[cur_idx];
                            (p.width, p.height)
                        };
                        if ui.small_button("◀").on_hover_text("Geser Kiri").clicked() {
                            let w = self.canvas_state.zoom_target_rect.width();
                            let h = self.canvas_state.zoom_target_rect.height();
                            let mut min = self.canvas_state.zoom_target_rect.min;
                            min.x = (min.x - 40.0).max(0.0);
                            self.canvas_state.zoom_target_rect =
                                Rect::from_min_size(min, Vec2::new(w, h));
                        }
                        if ui.small_button("▶").on_hover_text("Geser Kanan").clicked() {
                            self.canvas_state.advance_zoom_target(pw, ph);
                        }
                        if ui.small_button("▲").on_hover_text("Geser Atas").clicked() {
                            let w = self.canvas_state.zoom_target_rect.width();
                            let h = self.canvas_state.zoom_target_rect.height();
                            let mut min = self.canvas_state.zoom_target_rect.min;
                            min.y = (min.y - 30.0).max(0.0);
                            self.canvas_state.zoom_target_rect =
                                Rect::from_min_size(min, Vec2::new(w, h));
                        }
                        if ui.small_button("▼").on_hover_text("Geser Bawah").clicked() {
                            let w = self.canvas_state.zoom_target_rect.width();
                            let h = self.canvas_state.zoom_target_rect.height();
                            let mut min = self.canvas_state.zoom_target_rect.min;
                            min.y = (min.y + 30.0).min((ph - h).max(0.0));
                            self.canvas_state.zoom_target_rect =
                                Rect::from_min_size(min, Vec2::new(w, h));
                        }
                        if ui
                            .small_button("⏎ Baris Baru")
                            .on_hover_text("Pindah ke awal baris berikutnya")
                            .clicked()
                        {
                            self.canvas_state.newline_zoom_target(pw, ph);
                        }
                        ui.separator();
                        ui.label(RichText::new("Skala:").size(11.0).color(Color32::GRAY));
                        for &s in &[2.0_f32, 2.5_f32, 3.0_f32] {
                            let is_sel = (self.settings.zoom_window_scale - s).abs() < 0.1;
                            if ui.selectable_label(is_sel, format!("{s:.1}x")).clicked() {
                                self.settings.zoom_window_scale = s;
                                let _ = save_settings_to_dir(&app_storage_dir(), &self.settings);
                            }
                        }
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.add_space(12.0);
                            if ui.small_button("✕ Tutup").clicked() {
                                self.canvas_state.show_zoom_window = false;
                            }
                        });
                    });
                    ui.separator();
                    ui.vertical_centered(|ui| {
                        let cur_idx = self.current_page_index;
                        let page = &mut self.library.notebooks[nb_idx].pages[cur_idx];
                        if render_zoom_window_canvas(ui, page, &mut self.canvas_state, scale) {
                            self.mark_dirty(&notebook_id);
                        }
                    });
                });
        }

        // Drawing Canvas Central View (Desk Background mengikuti tema Dark / Light / System OS Windows)
        let mut canvas_top_rect = Rect::NOTHING;
        egui::CentralPanel::default().show(ctx, |ui| {
            canvas_top_rect = ui.max_rect();
            self.last_canvas_viewport_rect = canvas_top_rect;
            let desk_bg = match resolve_theme_mode(ctx, self.settings.theme_mode) {
                ThemeMode::Dark | ThemeMode::System => Color32::from_rgb(29, 28, 26), // #1d1c1a
                ThemeMode::Light => Color32::from_rgb(224, 221, 215),                 // #e0ddd7
            };
            ui.painter().rect_filled(ui.max_rect(), 0.0, desk_bg);

            match self.view_mode {
                ViewMode::SinglePage => {
                    // Dalam SinglePage mode, halaman tunggal berdiri sendiri (tidak nyambung/scroll ke halaman lain).
                    // Pergeseran kamera (Arrow keys, Hand tool, atau zoom ke arah mouse) diatur melalui `self.canvas_pan_offset`.
                    let _ = self.pending_scroll_offset.take();
                    let cur_idx = self
                        .current_page_index
                        .min(self.library.notebooks[nb_idx].pages.len().saturating_sub(1));
                    self.current_page_index = cur_idx;
                    let prev_obj_count =
                        self.library.notebooks[nb_idx].pages[cur_idx].objects.len();
                    let (pw, ph) = {
                        let p = &self.library.notebooks[nb_idx].pages[cur_idx];
                        (p.width, p.height)
                    };
                    let desired_size =
                        Vec2::new(pw * self.canvas_state.zoom, ph * self.canvas_state.zoom);
                    let default_left = canvas_top_rect.center().x - desired_size.x * 0.5;
                    let default_top = canvas_top_rect.top() + 24.0;

                    // Batasi pan agar kertas tidak pernah hilang seluruhnya keluar layar
                    let min_pan_x = (canvas_top_rect.left() + 80.0 - desired_size.x) - default_left;
                    let max_pan_x = (canvas_top_rect.right() - 80.0) - default_left;
                    if min_pan_x <= max_pan_x {
                        self.canvas_pan_offset.x =
                            self.canvas_pan_offset.x.clamp(min_pan_x, max_pan_x);
                    }
                    let min_pan_y = (canvas_top_rect.top() + 80.0 - desired_size.y) - default_top;
                    let max_pan_y = (canvas_top_rect.bottom() - 80.0) - default_top;
                    if min_pan_y <= max_pan_y {
                        self.canvas_pan_offset.y =
                            self.canvas_pan_offset.y.clamp(min_pan_y, max_pan_y);
                    }

                    let page_rect = Rect::from_min_size(
                        Pos2::new(
                            default_left + self.canvas_pan_offset.x,
                            default_top + self.canvas_pan_offset.y,
                        ),
                        desired_size,
                    );
                    let page = &mut self.library.notebooks[nb_idx].pages[cur_idx];
                    let mut page_ui = ui.new_child(
                        egui::UiBuilder::new()
                            .max_rect(page_rect)
                            .layout(Layout::top_down(Align::Min)),
                    );
                    page_ui.set_clip_rect(canvas_top_rect);
                    page_ui.push_id(cur_idx, |ui| {
                        render_page_canvas(ui, page, &mut self.canvas_state);
                    });
                    if self.library.notebooks[nb_idx].pages[cur_idx].objects.len() != prev_obj_count
                    {
                        self.mark_dirty(&notebook_id);
                    }
                }
                ViewMode::ContinuousVertical => {
                    let mut scroll_area = ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .scroll_bar_visibility(
                            egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded,
                        );
                    let just_jumped = if let Some(off) = self.pending_scroll_offset.take() {
                        scroll_area = scroll_area.vertical_scroll_offset(off.y);
                        self.canvas_scroll_offset = off;
                        true
                    } else {
                        false
                    };
                    let scroll_out = scroll_area.show(ui, |ui| {
                        ui.set_min_width(canvas_top_rect.width());
                        ui.vertical_centered(|ui| {
                            ui.add_space(24.0);
                            let pages_count = self.library.notebooks[nb_idx].pages.len();
                            let first_page_h = self.library.notebooks[nb_idx]
                                .pages
                                .first()
                                .map(|p| (p.height * self.canvas_state.zoom).max(80.0))
                                .unwrap_or(400.0);
                            let anchor_offset = (canvas_top_rect.height() * 0.22)
                                .clamp(48.0, 140.0)
                                .min(first_page_h * 0.5);
                            let anchor_y = canvas_top_rect.top() + 24.0 + anchor_offset;
                            let mut best_page_idx =
                                self.current_page_index.min(pages_count.saturating_sub(1));
                            let mut best_dist = f32::MAX;

                            for p_i in 0..pages_count {
                                let prev_obj_count =
                                    self.library.notebooks[nb_idx].pages[p_i].objects.len();
                                let (pw, ph, paper_col) = {
                                    let p = &self.library.notebooks[nb_idx].pages[p_i];
                                    (p.width, p.height, p.paper_color.to_color32())
                                };
                                let desired_size = Vec2::new(
                                    pw * self.canvas_state.zoom,
                                    ph * self.canvas_state.zoom,
                                );
                                let default_left =
                                    canvas_top_rect.center().x - desired_size.x * 0.5;
                                let min_pan_x =
                                    (canvas_top_rect.left() + 80.0 - desired_size.x) - default_left;
                                let max_pan_x = (canvas_top_rect.right() - 80.0) - default_left;
                                if min_pan_x <= max_pan_x {
                                    self.canvas_pan_offset.x =
                                        self.canvas_pan_offset.x.clamp(min_pan_x, max_pan_x);
                                }
                                let row_top_y = ui.cursor().top();
                                let next_page_rect = Rect::from_min_size(
                                    Pos2::new(default_left + self.canvas_pan_offset.x, row_top_y),
                                    desired_size,
                                );

                                // Hitung jarak vertikal halaman terhadap garis baca (`anchor_y`)
                                let dist = if anchor_y >= next_page_rect.top()
                                    && anchor_y <= next_page_rect.bottom()
                                {
                                    0.0
                                } else {
                                    (next_page_rect.top() - anchor_y)
                                        .abs()
                                        .min((anchor_y - next_page_rect.bottom()).abs())
                                };
                                if dist < best_dist {
                                    best_dist = dist;
                                    best_page_idx = p_i;
                                }

                                // Virtualization: render hanya halaman yang terlihat di layar
                                if canvas_top_rect.intersects(next_page_rect) {
                                    let page = &mut self.library.notebooks[nb_idx].pages[p_i];
                                    let mut page_ui = ui.new_child(
                                        egui::UiBuilder::new()
                                            .max_rect(next_page_rect)
                                            .layout(Layout::top_down(Align::Min)),
                                    );
                                    page_ui.set_clip_rect(canvas_top_rect);
                                    page_ui.push_id(p_i, |ui| {
                                        render_page_canvas(ui, page, &mut self.canvas_state);
                                    });
                                    if self.library.notebooks[nb_idx].pages[p_i].objects.len()
                                        != prev_obj_count
                                    {
                                        self.mark_dirty(&notebook_id);
                                    }
                                } else {
                                    ui.painter().rect_filled(next_page_rect, 4.0, paper_col);
                                }
                                ui.add_space(desired_size.y + 24.0 * self.canvas_state.zoom);
                            }

                            // Sinkronkan nomor halaman aktif dengan halaman yang sedang tertampil saat scroll
                            if !just_jumped && best_page_idx < pages_count {
                                self.current_page_index = best_page_idx;
                            }

                            // Ruang bawah ekstra agar halaman terakhir tetap bisa di-scroll ke posisi baca atas
                            ui.add_space((canvas_top_rect.height() * 0.65).max(160.0));
                        });
                    });
                    if !just_jumped {
                        self.canvas_scroll_offset = scroll_out.state.offset;
                    }
                }
            }
        });

        // Authentic GoodNotes Floating Top-Left Capsule Over Canvas: `( ↩ | ↪ )` + `( ◀ 1/N ▶ )`
        if canvas_top_rect.is_positive() {
            let pill_pos = canvas_top_rect.left_top() + Vec2::new(12.0, 10.0);
            egui::Area::new(egui::Id::new("goodnotes_floating_undo_pill"))
                .fixed_pos(pill_pos)
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        // Pill 1: `( ↩ | ↪ )` Undo & Redo
                        egui::Frame::NONE
                            .fill(Color32::from_rgba_unmultiplied(30, 30, 32, 225))
                            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(65, 65, 70)))
                            .corner_radius(16.0)
                            .inner_margin(egui::Margin::symmetric(8, 4))
                            .show(ui, |ui| {
                                ui.spacing_mut().item_spacing.x = 4.0;
                                let current_page_id = self.library.notebooks[nb_idx].pages
                                    [self.current_page_index]
                                    .id
                                    .clone();
                                let can_undo_stroke = self.canvas_state.can_undo(&current_page_id)
                                    || self.canvas_state.last_undo_page_id().is_some();
                                let can_undo_doc = self
                                    .doc_undo_stacks
                                    .get(&notebook_id)
                                    .map(|s| !s.is_empty())
                                    .unwrap_or(false);
                                if ui
                                    .add_enabled(
                                        can_undo_stroke || can_undo_doc,
                                        Button::new(
                                            RichText::new("↩").size(14.0).color(Color32::WHITE),
                                        )
                                        .frame(false),
                                    )
                                    .on_hover_text("Undo (Ctrl+Z)")
                                    .clicked()
                                {
                                    self.perform_undo(nb_idx);
                                }

                                ui.separator();

                                let can_redo_stroke = self.canvas_state.can_redo(&current_page_id)
                                    || self.canvas_state.last_redo_page_id().is_some();
                                let can_redo_doc = self
                                    .doc_redo_stacks
                                    .get(&notebook_id)
                                    .map(|s| !s.is_empty())
                                    .unwrap_or(false);
                                if ui
                                    .add_enabled(
                                        can_redo_stroke || can_redo_doc,
                                        Button::new(
                                            RichText::new("↪").size(14.0).color(Color32::WHITE),
                                        )
                                        .frame(false),
                                    )
                                    .on_hover_text("Redo (Ctrl+Y)")
                                    .clicked()
                                {
                                    self.perform_redo(nb_idx);
                                }
                            });

                        // Pill 2: `( ◀ 1/N ▶ )` Page Navigation Stepper
                        let page_count = self.library.notebooks[nb_idx].pages.len();
                        egui::Frame::NONE
                            .fill(Color32::from_rgba_unmultiplied(30, 30, 32, 225))
                            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(65, 65, 70)))
                            .corner_radius(16.0)
                            .inner_margin(egui::Margin::symmetric(8, 4))
                            .show(ui, |ui| {
                                ui.spacing_mut().item_spacing.x = 4.0;
                                if ui
                                    .add_enabled(
                                        self.current_page_index > 0,
                                        Button::new(
                                            RichText::new("◀").size(11.5).color(Color32::WHITE),
                                        )
                                        .frame(false),
                                    )
                                    .on_hover_text("Halaman Sebelumnya (PageUp)")
                                    .clicked()
                                {
                                    let prev_idx = self.current_page_index - 1;
                                    self.navigate_to_page(nb_idx, prev_idx);
                                }
                                ui.label(
                                    RichText::new(format!(
                                        "{}/{}",
                                        self.current_page_index + 1,
                                        page_count
                                    ))
                                    .size(11.5)
                                    .strong()
                                    .color(Color32::WHITE),
                                );
                                if ui
                                    .add_enabled(
                                        self.current_page_index + 1 < page_count,
                                        Button::new(
                                            RichText::new("▶").size(11.5).color(Color32::WHITE),
                                        )
                                        .frame(false),
                                    )
                                    .on_hover_text("Halaman Berikutnya (PageDown)")
                                    .clicked()
                                {
                                    let next_idx = self.current_page_index + 1;
                                    self.navigate_to_page(nb_idx, next_idx);
                                }
                            });
                    });
                });

            // Floating Bottom-Left Collapsible Zoom Capsule (Ikon ringkas saat tertutup, membesar menampilkan slider saat diklik)
            let zoom_pill_pos = canvas_top_rect.left_bottom() + Vec2::new(12.0, -42.0);
            egui::Area::new(egui::Id::new("goodnotes_floating_zoom_pill"))
                .fixed_pos(zoom_pill_pos)
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    egui::Frame::NONE
                        .fill(Color32::from_rgba_unmultiplied(30, 30, 32, 225))
                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(65, 65, 70)))
                        .corner_radius(16.0)
                        .inner_margin(if self.show_zoom_slider_expanded {
                            egui::Margin::symmetric(10, 5)
                        } else {
                            egui::Margin::symmetric(8, 5)
                        })
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 6.0;

                                let zoom_pct = (self.canvas_state.zoom * 100.0).round() as i32;
                                let toggle_btn = Button::new(
                                    RichText::new("🔍").size(13.5).color(
                                        if self.show_zoom_slider_expanded {
                                            Color32::from_rgb(130, 195, 255)
                                        } else {
                                            Color32::WHITE
                                        },
                                    ),
                                )
                                .frame(false);

                                if ui
                                    .add(toggle_btn)
                                    .on_hover_text(if self.show_zoom_slider_expanded {
                                        format!("Zoom: {zoom_pct}% — Klik lagi untuk mengecilkan menjadi ikon")
                                    } else {
                                        format!("Zoom: {zoom_pct}% — Klik untuk menampilkan slider Zoom")
                                    })
                                    .clicked()
                                {
                                    self.show_zoom_slider_expanded =
                                        !self.show_zoom_slider_expanded;
                                }

                                if self.show_zoom_slider_expanded {
                                    ui.separator();

                                    if ui
                                        .add(
                                            Button::new(
                                                RichText::new("-")
                                                    .size(13.5)
                                                    .strong()
                                                    .color(Color32::WHITE),
                                            )
                                            .frame(false),
                                        )
                                        .on_hover_text("Perkecil Zoom (-10%)")
                                        .clicked()
                                    {
                                        let z = (self.canvas_state.zoom - 0.1).max(0.4);
                                        self.apply_zoom_anchored(nb_idx, z, None);
                                    }

                                    ui.spacing_mut().slider_width = 95.0;
                                    let mut slider_zoom = self.canvas_state.zoom;
                                    if ui
                                        .add(
                                            egui::Slider::new(&mut slider_zoom, 0.4..=2.5)
                                                .show_value(false),
                                        )
                                        .on_hover_text("Geser untuk mengatur tingkat Zoom kanvas")
                                        .changed()
                                    {
                                        self.apply_zoom_anchored(nb_idx, slider_zoom, None);
                                    }

                                    if ui
                                        .add(
                                            Button::new(
                                                RichText::new("+")
                                                    .size(13.5)
                                                    .strong()
                                                    .color(Color32::WHITE),
                                            )
                                            .frame(false),
                                        )
                                        .on_hover_text("Perbesar Zoom (+10%)")
                                        .clicked()
                                    {
                                        let z = (self.canvas_state.zoom + 0.1).min(2.5);
                                        self.apply_zoom_anchored(nb_idx, z, None);
                                    }

                                    ui.separator();

                                    if ui
                                        .add(
                                            Button::new(
                                                RichText::new(format!("{zoom_pct}%"))
                                                    .size(11.5)
                                                    .strong()
                                                    .color(Color32::WHITE),
                                            )
                                            .frame(false),
                                        )
                                        .on_hover_text("Klik untuk Reset Zoom ke 100% (Ctrl+0)")
                                        .clicked()
                                    {
                                        self.apply_zoom_anchored(nb_idx, 1.0, None);
                                        self.canvas_pan_offset = Vec2::ZERO;
                                    }
                                }
                            });
                        });
                });
        }

        // Editor Modal Dialogs
        self.render_editor_modals(ctx, nb_idx);
    }
}

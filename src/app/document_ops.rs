use super::state::RustNotesApp;
use super::types::{AddPagePosition, ViewMode};
use crate::canvas::ActiveTool;
use crate::model::{Notebook, PageOrientation, PageTemplate, PaperSizePreset};
use crate::storage::{
    app_storage_dir, import_pdf_to_pages, save_asset_to_notebook, save_library_index_to_dir,
    save_notebook_to_dir,
};
use eframe::egui::{Color32, Pos2, Vec2};
use std::time::Instant;

impl RustNotesApp {
    pub(crate) fn open_page_resize_modal(&mut self, nb_idx: usize, page_idx: usize) {
        if let Some(nb) = self.library.notebooks.get(nb_idx)
            && let Some(p) = nb.pages.get(page_idx)
        {
            self.resize_target_page_index = page_idx;
            self.resize_custom_width = p.width;
            self.resize_custom_height = p.height;
            self.resize_target_template = p.template;
            self.resize_apply_all_pages = false;
            self.resize_target_orientation = if p.width > p.height {
                PageOrientation::Landscape
            } else {
                PageOrientation::Portrait
            };
            // Cek apakah cocok dengan salah satu preset
            let mut matched_preset = None;
            for preset in PaperSizePreset::ALL {
                let (pw, ph) = preset.dimensions(self.resize_target_orientation);
                if (p.width - pw).abs() < 1.0 && (p.height - ph).abs() < 1.0 {
                    matched_preset = Some(preset);
                    break;
                }
            }
            if let Some(preset) = matched_preset {
                self.resize_target_preset = preset;
                self.resize_use_custom_size = false;
            } else {
                self.resize_use_custom_size = true;
            }
            self.show_page_resize_modal = true;
        }
    }

    /// Buka modal penambahan halaman baru dengan menampilkan template aktif dan opsi kustomisasi.
    pub(crate) fn open_add_page_modal(&mut self, nb_idx: usize) {
        if let Some(nb) = self.library.notebooks.get(nb_idx)
            && let Some(p) = nb.pages.get(self.current_page_index)
        {
            self.add_page_template = p.template;
            self.add_page_orientation = if p.width > p.height {
                PageOrientation::Landscape
            } else {
                PageOrientation::Portrait
            };
            self.add_page_preset = PaperSizePreset::A4;
            self.add_page_target_pos = AddPagePosition::AfterCurrent;
            self.show_add_page_modal = true;
        }
    }

    /// Hitung posisi vertikal (offset Y dalam `ScrollArea`) untuk halaman `page_idx`
    /// pada mode `ViewMode::ContinuousVertical`.
    pub(crate) fn page_y_offset(&self, nb_idx: usize, page_idx: usize, zoom: f32) -> f32 {
        if let Some(nb) = self.library.notebooks.get(nb_idx) {
            let mut y = 0.0_f32;
            for p in nb.pages.iter().take(page_idx) {
                y += p.height * zoom + 24.0 * zoom;
            }
            y
        } else {
            0.0
        }
    }

    /// Navigasi ke halaman `page_idx` dan sinkronkan posisi scroll kanvas.
    pub(crate) fn navigate_to_page(&mut self, nb_idx: usize, page_idx: usize) {
        let Some(nb) = self.library.notebooks.get(nb_idx) else {
            return;
        };
        if nb.pages.is_empty() {
            return;
        }
        let target_idx = page_idx.min(nb.pages.len() - 1);
        if self.current_page_index != target_idx {
            self.canvas_state.selected_ids.clear();
        }
        self.current_page_index = target_idx;
        match self.view_mode {
            ViewMode::ContinuousVertical => {
                let target_y = self.page_y_offset(nb_idx, target_idx, self.canvas_state.zoom);
                let off = Vec2::new(0.0, target_y);
                self.canvas_scroll_offset = off;
                self.pending_scroll_offset = Some(off);
            }
            ViewMode::SinglePage => {
                self.canvas_pan_offset.y = 0.0;
                self.canvas_scroll_offset = Vec2::ZERO;
                self.pending_scroll_offset = Some(Vec2::ZERO);
            }
        }
    }

    /// Ubah mode tampilan (`SinglePage` <-> `ContinuousVertical`) sambil mempertahankan
    /// halaman yang sedang tampil di layar.
    pub(crate) fn set_view_mode(&mut self, nb_idx: usize, new_mode: ViewMode) {
        if self.view_mode == new_mode {
            return;
        }
        self.view_mode = new_mode;
        match new_mode {
            ViewMode::SinglePage => {
                // `self.current_page_index` sudah mengarah ke halaman yang tertampil saat scroll;
                // reset pan/scroll agar halaman tunggal tersebut tampil rapi di tengah.
                self.canvas_pan_offset = Vec2::ZERO;
                self.canvas_scroll_offset = Vec2::ZERO;
                self.pending_scroll_offset = Some(Vec2::ZERO);
            }
            ViewMode::ContinuousVertical => {
                // Lompat langsung ke posisi vertikal `self.current_page_index` agar tidak kembali ke halaman 1.
                let target_y =
                    self.page_y_offset(nb_idx, self.current_page_index, self.canvas_state.zoom);
                self.canvas_pan_offset = Vec2::ZERO;
                let off = Vec2::new(0.0, target_y);
                self.canvas_scroll_offset = off;
                self.pending_scroll_offset = Some(off);
            }
        }
    }

    /// Ubah tingkat zoom dengan titik jangkar (`anchor_screen_pos`, misalnya posisi kursor mouse)
    /// sehingga titik pada kertas di bawah kursor tetap berada di posisi layar yang sama.
    pub(crate) fn apply_zoom_anchored(
        &mut self,
        nb_idx: usize,
        new_zoom: f32,
        anchor_screen_pos: Option<Pos2>,
    ) {
        let old_zoom = self.canvas_state.zoom;
        let clamped_zoom = new_zoom.clamp(0.4, 2.5);
        if (clamped_zoom - old_zoom).abs() < 1e-4 {
            return;
        }
        let v_rect = self.last_canvas_viewport_rect;
        if v_rect.is_positive()
            && let Some(nb) = self.library.notebooks.get(nb_idx)
            && !nb.pages.is_empty()
        {
            let raw_anchor = anchor_screen_pos.unwrap_or_else(|| v_rect.center());
            let anchor = Pos2::new(
                raw_anchor.x.clamp(v_rect.left(), v_rect.right()),
                raw_anchor.y.clamp(v_rect.top(), v_rect.bottom()),
            );
            let cur_idx = self.current_page_index.min(nb.pages.len() - 1);
            let pw = nb.pages[cur_idx].width;

            // Horizontal anchor (berlaku untuk SinglePage maupun ContinuousVertical)
            let old_origin_x = v_rect.center().x - pw * old_zoom * 0.5 + self.canvas_pan_offset.x;
            let page_pt_x = (anchor.x - old_origin_x) / old_zoom;
            let new_default_origin_x = v_rect.center().x - pw * clamped_zoom * 0.5;
            self.canvas_pan_offset.x = (anchor.x - page_pt_x * clamped_zoom) - new_default_origin_x;

            // Vertical anchor
            match self.view_mode {
                ViewMode::SinglePage => {
                    let old_origin_y = v_rect.top() + 24.0 + self.canvas_pan_offset.y;
                    let page_pt_y = (anchor.y - old_origin_y) / old_zoom;
                    let new_default_origin_y = v_rect.top() + 24.0;
                    self.canvas_pan_offset.y =
                        (anchor.y - page_pt_y * clamped_zoom) - new_default_origin_y;
                }
                ViewMode::ContinuousVertical => {
                    let rel_y = anchor.y - (v_rect.top() + 24.0);
                    let doc_y_old = self.canvas_scroll_offset.y + rel_y;
                    let doc_y_new = doc_y_old * (clamped_zoom / old_zoom);
                    let new_scroll_y = (doc_y_new - rel_y).max(0.0);
                    self.canvas_scroll_offset.y = new_scroll_y;
                    self.pending_scroll_offset = Some(Vec2::new(0.0, new_scroll_y));
                }
            }
        }
        self.canvas_state.zoom = clamped_zoom;
    }

    /// Fase 6: Tandai notebook sebagai perlu disimpan (debounced).
    pub(crate) fn mark_dirty(&mut self, notebook_id: &str) {
        if self.is_read_only {
            return;
        }
        self.dirty_notebooks.insert(notebook_id.to_string());
        self.last_modified
            .insert(notebook_id.to_string(), Instant::now());
    }

    /// Fase 6: Simpan satu notebook segera (force-save, bypass debounce).
    pub(crate) fn force_save_notebook(&mut self, notebook_id: &str) {
        if self.is_read_only {
            return;
        }
        let storage_dir = app_storage_dir();
        if let Some(nb) = self.library.notebooks.iter().find(|n| n.id == notebook_id) {
            let result = save_notebook_to_dir(&storage_dir, nb);
            if result.is_err() {
                self.toast_message = Some(format!(
                    "⚠ Gagal menyimpan notebook '{}'. Data mungkin belum tersimpan.",
                    nb.title
                ));
            }
        }
        self.dirty_notebooks.remove(notebook_id);
        self.last_modified.remove(notebook_id);
        self.sync_library_index();
    }

    /// Fase 6: Force-save semua notebook yang dirty (dipanggil saat tutup/pindah tab).
    pub(crate) fn flush_all_dirty(&mut self) {
        if self.is_read_only {
            return;
        }
        let ids: Vec<String> = self.dirty_notebooks.iter().cloned().collect();
        for id in ids {
            self.force_save_notebook(&id);
        }
    }

    /// Fase 6: Sinkronkan library_index.json dari state library saat ini.
    pub(crate) fn sync_library_index(&self) {
        if self.is_read_only {
            return;
        }
        let storage_dir = app_storage_dir();
        let mut index = crate::model::LibraryIndex::default();
        for nb in &self.library.notebooks {
            index
                .notebooks
                .push(crate::model::NotebookMeta::from_notebook(nb));
        }
        index.folders = self.library.folders.clone();
        let _ = save_library_index_to_dir(&storage_dir, &index);
    }

    /// Lama: persist seluruh library sekaligus (dipakai untuk operasi metadata non-notebook).
    pub(crate) fn persist_library(&mut self) {
        if self.is_read_only {
            return;
        }
        // Untuk operasi metadata (favorite/trash/reorder), simpan index + force-save semua dirty
        self.flush_all_dirty();
        self.sync_library_index();
    }

    /// Fase 7: Buka berkas eksternal (.rnote atau .zip) dari argumen baris perintah atau file dialog.
    pub fn open_external_file(&mut self, path: &std::path::Path) {
        let storage_dir = app_storage_dir();
        match crate::storage::import_notebook_from_rnote(&storage_dir, path) {
            Ok(nb) => {
                let id = nb.id.clone();
                if !self.library.notebooks.iter().any(|n| n.id == id) {
                    self.library.notebooks.push(nb);
                }
                if !self.open_notebook_ids.contains(&id) {
                    self.open_notebook_ids.push(id.clone());
                }
                self.active_notebook_id = Some(id);
                self.current_page_index = 0;
                self.toast_message = Some(format!(
                    "✅ Berkas catatan '{}' berhasil dibuka.",
                    path.file_name().unwrap_or_default().to_string_lossy()
                ));
            }
            Err(e) => {
                self.toast_message = Some(format!("❌ Gagal membuka berkas catatan: {e}"));
            }
        }
    }

    pub(crate) fn push_doc_undo(&mut self, nb_idx: usize) {
        if let Some(nb) = self.library.notebooks.get(nb_idx) {
            let nb_id = nb.id.clone();
            let pages = nb.pages.clone();
            let cur_idx = self.current_page_index;
            let stack = self.doc_undo_stacks.entry(nb_id.clone()).or_default();
            if stack.len() >= 50 {
                stack.remove(0);
            }
            stack.push((pages, cur_idx));
            self.doc_redo_stacks.remove(&nb_id);
        }
    }

    pub(crate) fn undo_document(&mut self, nb_id: &str) -> bool {
        if let Some(stack) = self.doc_undo_stacks.get_mut(nb_id)
            && let Some((prev_pages, prev_idx)) = stack.pop()
            && let Some(nb) = self.library.notebooks.iter_mut().find(|n| n.id == nb_id)
        {
            let current_pages = nb.pages.clone();
            let current_idx = self.current_page_index;
            let redo_stack = self.doc_redo_stacks.entry(nb_id.to_string()).or_default();
            redo_stack.push((current_pages, current_idx));

            nb.pages = prev_pages;
            self.current_page_index = prev_idx.min(nb.pages.len() - 1);
            let nb_id_owned = nb_id.to_string();
            self.mark_dirty(&nb_id_owned);
            return true;
        }
        false
    }

    pub(crate) fn redo_document(&mut self, nb_id: &str) -> bool {
        if let Some(stack) = self.doc_redo_stacks.get_mut(nb_id)
            && let Some((next_pages, next_idx)) = stack.pop()
            && let Some(nb) = self.library.notebooks.iter_mut().find(|n| n.id == nb_id)
        {
            let current_pages = nb.pages.clone();
            let current_idx = self.current_page_index;
            let undo_stack = self.doc_undo_stacks.entry(nb_id.to_string()).or_default();
            undo_stack.push((current_pages, current_idx));

            nb.pages = next_pages;
            self.current_page_index = next_idx.min(nb.pages.len() - 1);
            let nb_id_owned = nb_id.to_string();
            self.mark_dirty(&nb_id_owned);
            return true;
        }
        false
    }

    pub(crate) fn perform_undo(&mut self, nb_idx: usize) {
        if self.canvas_state.editing_text_id.is_some() {
            return;
        }
        let notebook = &self.library.notebooks[nb_idx];
        let notebook_id = notebook.id.clone();

        // 1. Cek apakah ada stroke/canvas undo yang terakhir dilakukan
        let target_page_id = if let Some(last_id) = self.canvas_state.last_undo_page_id() {
            Some(last_id.to_string())
        } else {
            let cur_id = notebook
                .pages
                .get(self.current_page_index)
                .map(|p| p.id.clone());
            if cur_id
                .as_ref()
                .is_some_and(|id| self.canvas_state.can_undo(id))
            {
                cur_id
            } else {
                None
            }
        };

        if let Some(page_id) = target_page_id
            && let Some(p_idx) = notebook.pages.iter().position(|p| p.id == page_id)
        {
            self.current_page_index = p_idx;
            let page = &mut self.library.notebooks[nb_idx].pages[p_idx];
            self.canvas_state.undo(page);
            self.mark_dirty(&notebook_id);
            return;
        }

        // 2. Jika tidak ada stroke undo di halaman mana pun, cek doc-level undo
        if self.undo_document(&notebook_id) {
            self.mark_dirty(&notebook_id);
        }
    }

    pub(crate) fn perform_redo(&mut self, nb_idx: usize) {
        if self.canvas_state.editing_text_id.is_some() {
            return;
        }
        let notebook = &self.library.notebooks[nb_idx];
        let notebook_id = notebook.id.clone();

        // 1. Cek apakah ada stroke/canvas redo yang terakhir dilakukan
        let target_page_id = if let Some(last_id) = self.canvas_state.last_redo_page_id() {
            Some(last_id.to_string())
        } else {
            let cur_id = notebook
                .pages
                .get(self.current_page_index)
                .map(|p| p.id.clone());
            if cur_id
                .as_ref()
                .is_some_and(|id| self.canvas_state.can_redo(id))
            {
                cur_id
            } else {
                None
            }
        };

        if let Some(page_id) = target_page_id
            && let Some(p_idx) = notebook.pages.iter().position(|p| p.id == page_id)
        {
            self.current_page_index = p_idx;
            let page = &mut self.library.notebooks[nb_idx].pages[p_idx];
            self.canvas_state.redo(page);
            self.mark_dirty(&notebook_id);
            return;
        }

        // 2. Jika tidak ada stroke redo, cek doc-level redo
        if self.redo_document(&notebook_id) {
            self.mark_dirty(&notebook_id);
        }
    }

    // Fase 3: Helper menyisipkan gambar ke halaman aktif
    pub(crate) fn insert_image_to_page(
        &mut self,
        nb_idx: usize,
        img_bytes: &[u8],
        extension: &str,
        target_pos: Option<Pos2>,
    ) -> bool {
        let storage_dir = app_storage_dir();
        let nb_id = self.library.notebooks[nb_idx].id.clone();
        let cur_page_idx = self.current_page_index;
        if cur_page_idx >= self.library.notebooks[nb_idx].pages.len() {
            return false;
        }

        if let Ok(asset_id) = save_asset_to_notebook(&storage_dir, &nb_id, img_bytes, extension)
            && let Ok(dyn_img) = image::load_from_memory(img_bytes)
        {
            let (orig_w, orig_h) = (dyn_img.width() as f32, dyn_img.height() as f32);
            let (page_w, page_h) = {
                let p = &self.library.notebooks[nb_idx].pages[cur_page_idx];
                (p.width, p.height)
            };

            let target_w = orig_w.min(page_w * 0.75).max(100.0);
            let target_h = target_w * (orig_h / orig_w.max(1.0));

            let (pos_x, pos_y) = if let Some(target) = target_pos {
                let x =
                    (target.x - target_w / 2.0).clamp(10.0, (page_w - target_w - 10.0).max(10.0));
                let y =
                    (target.y - target_h / 2.0).clamp(10.0, (page_h - target_h - 10.0).max(10.0));
                (x, y)
            } else {
                let x = (page_w - target_w) / 2.0;
                let y = ((page_h - target_h) / 2.0).max(40.0);
                (x, y)
            };

            let page = &mut self.library.notebooks[nb_idx].pages[cur_page_idx];
            self.canvas_state.push_undo(&page.id, &page.objects);
            let new_img_id = uuid::Uuid::new_v4().to_string();
            let img_obj = crate::model::DrawObject::Image {
                id: new_img_id.clone(),
                asset_id,
                x: pos_x,
                y: pos_y,
                width: target_w,
                height: target_h,
                rotation: 0.0,
                lock_aspect: true,
            };

            page.objects.push(img_obj);
            self.canvas_state.selected_ids = vec![new_img_id];
            self.canvas_state.current_tool = ActiveTool::Lasso;
            self.canvas_state.spatial_grid.rebuild(&page.objects);
            self.mark_dirty(&nb_id);
            return true;
        }
        false
    }

    // Fase 3: Menempel gambar dari clipboard (Ctrl+V)
    pub(crate) fn try_paste_clipboard_image(
        &mut self,
        nb_idx: usize,
        target_pos: Option<Pos2>,
    ) -> bool {
        for _ in 0..2 {
            if let Ok(mut clipboard) = arboard::Clipboard::new() {
                // 1. Coba baca bitmap/image langsung dari clipboard
                if let Ok(img) = clipboard.get_image()
                    && let Some(buf) = image::RgbaImage::from_raw(
                        img.width as u32,
                        img.height as u32,
                        img.bytes.into_owned(),
                    )
                {
                    let dyn_img = image::DynamicImage::ImageRgba8(buf);
                    let mut cursor = std::io::Cursor::new(Vec::new());
                    if dyn_img
                        .write_to(&mut cursor, image::ImageFormat::Png)
                        .is_ok()
                    {
                        let bytes = cursor.into_inner();
                        if self.insert_image_to_page(nb_idx, &bytes, "png", target_pos) {
                            self.toast_message =
                                Some("✅ Gambar berhasil ditempel dari clipboard.".to_string());
                            return true;
                        }
                    }
                }

                // 2. Jika bukan bitmap mentah, periksa apakah clipboard berisi teks path berkas gambar
                if let Ok(txt) = clipboard.get_text() {
                    let trimmed = txt.trim().trim_matches('"').trim_matches('\'');
                    let path_str = trimmed.strip_prefix("file:///").unwrap_or(trimmed);
                    let candidate_path = std::path::Path::new(path_str);
                    if candidate_path.is_file()
                        && let Some(ext) = candidate_path.extension().and_then(|s| s.to_str())
                    {
                        let ext_lower = ext.to_lowercase();
                        if ["png", "jpg", "jpeg", "webp", "bmp", "gif"]
                            .contains(&ext_lower.as_str())
                            && let Ok(bytes) = std::fs::read(candidate_path)
                            && self.insert_image_to_page(nb_idx, &bytes, &ext_lower, target_pos)
                        {
                            self.toast_message =
                                Some("✅ Gambar berhasil ditempel dari berkas.".to_string());
                            return true;
                        }
                    }
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        false
    }

    // Fase 3: Import PDF sebagai notebook baru
    pub(crate) fn import_pdf_as_notebook(&mut self, pdf_path: &std::path::Path) -> bool {
        if let Ok(pdf_bytes) = std::fs::read(pdf_path) {
            let file_stem = pdf_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Imported Document");

            let storage_dir = app_storage_dir();
            let mut nb = Notebook::new(
                file_stem,
                PageTemplate::Blank,
                Color32::from_rgb(49, 85, 119),
            );
            let nb_id = nb.id.clone();

            match import_pdf_to_pages(&storage_dir, &nb_id, &pdf_bytes) {
                Ok(pages) => {
                    nb.pages = pages;
                    self.library.notebooks.push(nb);
                    self.force_save_notebook(&nb_id);
                    if !self.open_notebook_ids.contains(&nb_id) {
                        self.open_notebook_ids.push(nb_id.clone());
                    }
                    self.active_notebook_id = Some(nb_id);
                    self.current_page_index = 0;
                    self.toast_message =
                        Some(format!("✅ Dokumen PDF '{file_stem}' berhasil diimpor."));
                    return true;
                }
                Err(e) => {
                    self.toast_message = Some(format!("⚠ Gagal mengimpor PDF: {e}"));
                }
            }
        }
        false
    }

    // Fase 3: Import halaman PDF ke dalam notebook yang sedang dibuka
    pub(crate) fn import_pdf_pages_to_active(
        &mut self,
        nb_idx: usize,
        pdf_path: &std::path::Path,
    ) -> bool {
        if let Ok(pdf_bytes) = std::fs::read(pdf_path) {
            let storage_dir = app_storage_dir();
            let nb_id = self.library.notebooks[nb_idx].id.clone();

            match import_pdf_to_pages(&storage_dir, &nb_id, &pdf_bytes) {
                Ok(new_pages) => {
                    let count = new_pages.len();
                    self.push_doc_undo(nb_idx);
                    let insert_pos = (self.current_page_index + 1)
                        .min(self.library.notebooks[nb_idx].pages.len());
                    for (i, p) in new_pages.into_iter().enumerate() {
                        self.library.notebooks[nb_idx]
                            .pages
                            .insert(insert_pos + i, p);
                    }
                    self.current_page_index = insert_pos;
                    self.mark_dirty(&nb_id);
                    self.toast_message =
                        Some(format!("✅ {count} halaman PDF berhasil disisipkan."));
                    return true;
                }
                Err(e) => {
                    self.toast_message = Some(format!("⚠ Gagal mengimpor halaman PDF: {e}"));
                }
            }
        }
        false
    }
}

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
            if let Some((preset, orient)) = PaperSizePreset::match_dimensions(p.width, p.height) {
                self.resize_target_preset = preset;
                self.resize_target_orientation = orient;
                self.resize_use_custom_size = false;
            } else {
                self.resize_use_custom_size = true;
                self.resize_target_preset = PaperSizePreset::A4;
                self.resize_target_orientation = if p.width > p.height {
                    PageOrientation::Landscape
                } else {
                    PageOrientation::Portrait
                };
            }
            self.show_page_resize_modal = true;
        }
    }

    /// Buka modal penambahan halaman baru dengan menampilkan template aktif dan opsi kustomisasi.
    pub(crate) fn open_add_page_modal(&mut self, nb_idx: usize) {
        if let Some(nb) = self.library.notebooks.get(nb_idx) {
            // Prioritaskan format halaman yang sedang aktif (atau halaman terakhir jika di luar batas)
            let page_opt = nb
                .pages
                .get(self.current_page_index)
                .or_else(|| nb.pages.last());

            if let Some(p) = page_opt {
                self.add_page_template = p.template;
                self.add_page_paper_color = p.paper_color.to_color32();
                self.add_page_custom_width = p.width;
                self.add_page_custom_height = p.height;

                if let Some((preset, orient)) = PaperSizePreset::match_dimensions(p.width, p.height)
                {
                    self.add_page_preset = preset;
                    self.add_page_orientation = orient;
                    self.add_page_use_custom_size = false;
                } else {
                    self.add_page_use_custom_size = true;
                    self.add_page_preset = PaperSizePreset::A4;
                    self.add_page_orientation = if p.width > p.height {
                        PageOrientation::Landscape
                    } else {
                        PageOrientation::Portrait
                    };
                }
            } else {
                // Fallback ke memori pembuatan halaman / notebook terakhir
                self.add_page_template = self.last_created_template;
                self.add_page_preset = self.last_created_preset;
                self.add_page_orientation = self.last_created_orientation;
                self.add_page_use_custom_size = self.last_created_use_custom;
                self.add_page_custom_width = self.last_created_width;
                self.add_page_custom_height = self.last_created_height;
                self.add_page_paper_color = self.last_created_paper_color;
            }

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
    pub(crate) fn force_save_notebook(&mut self, notebook_id: &str) -> bool {
        if self.is_read_only {
            return false;
        }
        let storage_dir = app_storage_dir();
        if let Some(nb) = self.library.notebooks.iter().find(|n| n.id == notebook_id) {
            let result = save_notebook_to_dir(&storage_dir, nb);
            if let Err(e) = result {
                eprintln!("[Storage] Force-save failed for {notebook_id}: {e}");
                self.toast_message = Some(format!(
                    "⚠ Gagal menyimpan notebook '{}'. Data mungkin belum tersimpan.",
                    nb.title
                ));
                return false;
            }
        }
        self.dirty_notebooks.remove(notebook_id);
        self.last_modified.remove(notebook_id);
        self.sync_library_index();
        true
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
        self.sync_library_index_to_dir(&app_storage_dir());
    }

    pub(crate) fn sync_library_index_to_dir(&self, storage_dir: &std::path::Path) {
        if self.is_read_only {
            return;
        }
        let mut index = crate::model::LibraryIndex::default();
        let mut seen_ids = std::collections::HashSet::new();
        for nb in &self.library.notebooks {
            seen_ids.insert(nb.id.clone());
            index
                .notebooks
                .push(crate::model::NotebookMeta::from_notebook(nb));
        }
        // Pertahankan metadata notebook yang gagal dimuat saat startup agar tidak terhapus
        for meta in &self.unloaded_notebook_metas {
            if seen_ids.insert(meta.id.clone()) {
                index.notebooks.push(meta.clone());
            }
        }
        index.folders = self.library.folders.clone();
        let _ = save_library_index_to_dir(storage_dir, &index);
    }

    /// Impor cadangan library (.json) secara aman melalui jalur validasi & migrasi standar.
    pub(crate) fn import_library_backup_from_path(
        &mut self,
        path: &std::path::Path,
    ) -> Result<(), String> {
        self.import_library_backup_to_dir(path, &app_storage_dir())
    }

    pub(crate) fn import_library_backup_to_dir(
        &mut self,
        path: &std::path::Path,
        storage_dir: &std::path::Path,
    ) -> Result<(), String> {
        if self.is_read_only {
            return Err(
                "Mode Baca-Saja aktif: tidak dapat mengimpor cadangan library.".to_string(),
            );
        }

        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Gagal membaca berkas cadangan: {e}"))?;
        let raw_value: serde_json::Value = serde_json::from_str(&content)
            .map_err(|e| format!("Format JSON cadangan tidak valid: {e}"))?;

        let lib = crate::storage::migrate(raw_value)?;

        for nb in &lib.notebooks {
            if nb.schema_version > crate::model::CURRENT_NOTEBOOK_SCHEMA_VERSION {
                return Err(format!(
                    "Notebook '{}' menggunakan skema versi {} yang lebih baru dari dukungan aplikasi (v{}).",
                    nb.title,
                    nb.schema_version,
                    crate::model::CURRENT_NOTEBOOK_SCHEMA_VERSION
                ));
            }
        }

        // Simpan seluruh notebook ke disk terlebih dahulu sebelum mengganti state di memori
        for nb in &lib.notebooks {
            save_notebook_to_dir(storage_dir, nb)?;
        }

        let mut new_index = crate::model::LibraryIndex::default();
        for nb in &lib.notebooks {
            new_index
                .notebooks
                .push(crate::model::NotebookMeta::from_notebook(nb));
        }
        new_index.folders = lib.folders.clone();
        save_library_index_to_dir(storage_dir, &new_index)?;

        let mut missing_assets_count = 0usize;
        for nb in &lib.notebooks {
            missing_assets_count += crate::storage::validate_notebook_assets(storage_dir, nb).len();
        }

        // Segarkan state aplikasi secara konsisten
        self.open_notebook_ids.clear();
        self.active_notebook_id = None;
        self.current_page_index = 0;
        self.current_folder_id = None;
        self.dirty_notebooks.clear();
        self.last_modified.clear();
        self.doc_undo_stacks.clear();
        self.doc_redo_stacks.clear();
        self.unloaded_notebook_metas.clear();
        self.canvas_state.selected_ids.clear();
        self.canvas_state.mesh_cache.clear();
        self.canvas_state.texture_cache.clear();
        self.canvas_state.failed_texture_loads.clear();
        self.library = lib;

        if missing_assets_count > 0 {
            self.toast_message = Some(format!(
                "⚠ Library diimpor, namun {missing_assets_count} aset gambar/PDF tidak ditemukan di disk."
            ));
        } else {
            self.toast_message = Some("✅ Cadangan library berhasil diimpor.".to_string());
        }

        Ok(())
    }

    /// Menghapus permanen satu buku catatan yang berada di Sampah beserta direktori disknya.
    pub(crate) fn permanently_delete_notebook(&mut self, nb_id: &str) -> Result<(), String> {
        self.permanently_delete_notebook_from_dir(nb_id, &app_storage_dir())
    }

    pub(crate) fn permanently_delete_notebook_from_dir(
        &mut self,
        nb_id: &str,
        storage_dir: &std::path::Path,
    ) -> Result<(), String> {
        if self.is_read_only {
            return Err("Mode Baca-Saja aktif: tidak dapat menghapus buku catatan.".to_string());
        }
        let Some(pos) = self.library.notebooks.iter().position(|n| n.id == nb_id) else {
            return Err(format!("Buku catatan '{nb_id}' tidak ditemukan."));
        };
        if !self.library.notebooks[pos].trashed {
            return Err(
                "Hanya buku catatan di dalam Sampah yang dapat dihapus permanen.".to_string(),
            );
        }

        let safe_id = crate::storage::sanitize_path_component(nb_id)?;
        let nb_dir = storage_dir.join("notebooks").join(safe_id);
        if nb_dir.exists() {
            std::fs::remove_dir_all(&nb_dir)
                .map_err(|e| format!("Gagal menghapus berkas buku catatan di disk: {e}"))?;
        }

        self.library.notebooks.remove(pos);
        self.open_notebook_ids.retain(|id| id != nb_id);
        if self.active_notebook_id.as_deref() == Some(nb_id) {
            self.active_notebook_id = self.open_notebook_ids.last().cloned();
            self.current_page_index = 0;
        }
        self.dirty_notebooks.remove(nb_id);
        self.last_modified.remove(nb_id);
        self.doc_undo_stacks.remove(nb_id);
        self.doc_redo_stacks.remove(nb_id);
        self.canvas_state
            .texture_cache
            .retain(|(nid, _), _| nid != nb_id);
        self.canvas_state
            .failed_texture_loads
            .retain(|(nid, _)| nid != nb_id);

        self.sync_library_index_to_dir(storage_dir);
        Ok(())
    }

    /// Mengosongkan seluruh buku catatan yang berada di Sampah secara permanen.
    pub(crate) fn empty_trash(&mut self) -> Result<usize, String> {
        self.empty_trash_from_dir(&app_storage_dir())
    }

    pub(crate) fn empty_trash_from_dir(
        &mut self,
        storage_dir: &std::path::Path,
    ) -> Result<usize, String> {
        if self.is_read_only {
            return Err("Mode Baca-Saja aktif: tidak dapat mengosongkan Sampah.".to_string());
        }
        let trashed_ids: Vec<String> = self
            .library
            .notebooks
            .iter()
            .filter(|n| n.trashed)
            .map(|n| n.id.clone())
            .collect();
        let mut deleted = 0usize;
        for id in trashed_ids {
            self.permanently_delete_notebook_from_dir(&id, storage_dir)?;
            deleted += 1;
        }
        Ok(deleted)
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

    // Fase 3: Helper menyisipkan gambar ke halaman tertentu (atau halaman aktif)
    pub(crate) fn insert_image_to_page(
        &mut self,
        nb_idx: usize,
        page_idx: usize,
        img_bytes: &[u8],
        extension: &str,
        target_pos: Option<Pos2>,
    ) -> bool {
        let storage_dir = app_storage_dir();
        let nb_id = self.library.notebooks[nb_idx].id.clone();
        if page_idx >= self.library.notebooks[nb_idx].pages.len() {
            return false;
        }

        if let Ok(asset_id) = save_asset_to_notebook(&storage_dir, &nb_id, img_bytes, extension)
            && let Ok(dyn_img) = image::load_from_memory(img_bytes)
        {
            let (orig_w, orig_h) = (dyn_img.width() as f32, dyn_img.height() as f32);
            let (page_w, page_h) = {
                let p = &self.library.notebooks[nb_idx].pages[page_idx];
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

            let page = &mut self.library.notebooks[nb_idx].pages[page_idx];
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

    // Fase 3: Menempel gambar dari clipboard (Ctrl+V) ke halaman tertentu
    pub(crate) fn try_paste_clipboard_image(
        &mut self,
        nb_idx: usize,
        page_idx: usize,
        target_pos: Option<Pos2>,
    ) -> bool {
        for attempt in 0..4 {
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
                        if self.insert_image_to_page(nb_idx, page_idx, &bytes, "png", target_pos) {
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
                            && self.insert_image_to_page(
                                nb_idx, page_idx, &bytes, &ext_lower, target_pos,
                            )
                        {
                            self.toast_message =
                                Some("✅ Gambar berhasil ditempel dari berkas.".to_string());
                            return true;
                        }
                    }
                }
            }

            // 3. Di Windows: periksa apakah ada daftar file CF_HDROP (misal dicopy langsung dari Windows File Explorer)
            #[cfg(target_os = "windows")]
            {
                let hdrop_files = try_get_clipboard_hdrop_files();
                for candidate_path in hdrop_files {
                    if candidate_path.is_file()
                        && let Some(ext) = candidate_path.extension().and_then(|s| s.to_str())
                    {
                        let ext_lower = ext.to_lowercase();
                        if ["png", "jpg", "jpeg", "webp", "bmp", "gif"]
                            .contains(&ext_lower.as_str())
                            && let Ok(bytes) = std::fs::read(&candidate_path)
                            && self.insert_image_to_page(
                                nb_idx, page_idx, &bytes, &ext_lower, target_pos,
                            )
                        {
                            self.toast_message =
                                Some("✅ Gambar berhasil ditempel dari File Explorer.".to_string());
                            return true;
                        }
                    }
                }
            }

            if attempt < 3 {
                std::thread::sleep(std::time::Duration::from_millis(15));
            }
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

#[cfg(target_os = "windows")]
fn try_get_clipboard_hdrop_files() -> Vec<std::path::PathBuf> {
    unsafe extern "system" {
        fn OpenClipboard(hWndNewOwner: *mut std::ffi::c_void) -> i32;
        fn CloseClipboard() -> i32;
        fn GetClipboardData(uFormat: u32) -> *mut std::ffi::c_void;
        fn DragQueryFileW(
            hDrop: *mut std::ffi::c_void,
            iFile: u32,
            lpszFile: *mut u16,
            cch: u32,
        ) -> u32;
    }
    const CF_HDROP: u32 = 15;
    let mut paths = Vec::new();
    unsafe {
        if OpenClipboard(std::ptr::null_mut()) != 0 {
            let hdrop = GetClipboardData(CF_HDROP);
            if !hdrop.is_null() {
                let count = DragQueryFileW(hdrop, 0xFFFFFFFF, std::ptr::null_mut(), 0);
                for i in 0..count {
                    let len = DragQueryFileW(hdrop, i, std::ptr::null_mut(), 0);
                    if len > 0 {
                        let mut buf = vec![0u16; (len + 1) as usize];
                        if DragQueryFileW(hdrop, i, buf.as_mut_ptr(), len + 1) > 0 {
                            let s = String::from_utf16_lossy(&buf[..len as usize]);
                            paths.push(std::path::PathBuf::from(s));
                        }
                    }
                }
            }
            CloseClipboard();
        }
    }
    paths
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Notebook, PageTemplate};
    use eframe::egui::Color32;

    #[test]
    fn test_mark_notebook_dirty() {
        let mut app = RustNotesApp::default();
        let test_id = "test-nb-dirty-1";
        assert!(!app.dirty_notebooks.contains(test_id));
        app.mark_dirty(test_id);
        assert!(app.dirty_notebooks.contains(test_id));
        assert!(app.last_modified.contains_key(test_id));
    }

    #[test]
    fn test_force_save_read_only_preserves_dirty() {
        let mut app = RustNotesApp::default();
        let test_id = "test-nb-ro";
        app.is_read_only = true;
        app.dirty_notebooks.insert(test_id.to_string());
        let saved = app.force_save_notebook(test_id);
        assert!(!saved);
        assert!(app.dirty_notebooks.contains(test_id));
    }

    #[test]
    fn test_flush_all_dirty() {
        let mut app = RustNotesApp::default();
        let nb = Notebook::new("Test NB", PageTemplate::Blank, Color32::WHITE);
        let id = nb.id.clone();
        app.library.notebooks.push(nb);
        app.mark_dirty(&id);
        assert!(app.dirty_notebooks.contains(&id));

        app.flush_all_dirty();
        // Saving succeeds against the isolated per-test storage root, so dirty is cleared
        assert!(!app.dirty_notebooks.contains(&id));

        // Regression: the saved test notebook must land in the isolated test storage,
        // never in the user's real library used by the release application.
        let nb_json = app_storage_dir()
            .join("notebooks")
            .join(&id)
            .join("notebook.json");
        assert!(nb_json.exists());
        assert!(nb_json.starts_with(std::env::temp_dir()));
    }

    #[test]
    fn test_open_add_page_modal_inherits_slide16x9_and_template() {
        let mut app = RustNotesApp::default();
        let (slide_w, slide_h) = PaperSizePreset::Slide16x9.dimensions(PageOrientation::Landscape);
        let mut nb = Notebook::new_with_size(
            "Slide Notebook",
            PageTemplate::Isometric,
            Color32::DARK_BLUE,
            slide_w,
            slide_h,
        );
        let custom_color = Color32::from_rgb(240, 245, 235);
        nb.pages[0].paper_color = custom_color.into();

        let id = nb.id.clone();
        app.library.notebooks.push(nb);
        app.active_notebook_id = Some(id);
        app.current_page_index = 0;

        app.open_add_page_modal(0);

        assert!(app.show_add_page_modal);
        assert_eq!(app.add_page_template, PageTemplate::Isometric);
        assert_eq!(app.add_page_preset, PaperSizePreset::Slide16x9);
        assert_eq!(app.add_page_orientation, PageOrientation::Landscape);
        assert!(!app.add_page_use_custom_size);
        assert_eq!(app.add_page_paper_color, custom_color);
    }

    #[test]
    fn test_open_add_page_modal_inherits_custom_dimensions() {
        let mut app = RustNotesApp::default();
        let mut nb = Notebook::new_with_size(
            "Custom Size NB",
            PageTemplate::RuledNarrow,
            Color32::DARK_GREEN,
            723.0,
            945.0,
        );
        let custom_color = Color32::from_rgb(45, 45, 48);
        nb.pages[0].paper_color = custom_color.into();

        let id = nb.id.clone();
        app.library.notebooks.push(nb);
        app.active_notebook_id = Some(id);
        app.current_page_index = 0;

        app.open_add_page_modal(0);

        assert!(app.show_add_page_modal);
        assert_eq!(app.add_page_template, PageTemplate::RuledNarrow);
        assert!(app.add_page_use_custom_size);
        assert_eq!(app.add_page_custom_width, 723.0);
        assert_eq!(app.add_page_custom_height, 945.0);
        assert_eq!(app.add_page_paper_color, custom_color);
    }

    #[test]
    fn test_open_add_page_modal_fallback_last_created() {
        let mut app = RustNotesApp::default();
        let mut nb = Notebook::new("Empty NB", PageTemplate::Blank, Color32::BLACK);
        nb.pages.clear();
        let id = nb.id.clone();
        app.library.notebooks.push(nb);
        app.active_notebook_id = Some(id);
        app.last_created_template = PageTemplate::GridFine;
        app.last_created_preset = PaperSizePreset::Standard4x3;
        app.last_created_orientation = PageOrientation::Landscape;
        app.last_created_use_custom = false;
        app.last_created_paper_color = Color32::from_rgb(26, 26, 28);

        app.open_add_page_modal(0);

        assert!(app.show_add_page_modal);
        assert_eq!(app.add_page_template, PageTemplate::GridFine);
        assert_eq!(app.add_page_preset, PaperSizePreset::Standard4x3);
        assert_eq!(app.add_page_orientation, PageOrientation::Landscape);
        assert_eq!(app.add_page_paper_color, Color32::from_rgb(26, 26, 28));
    }

    #[test]
    fn test_failed_notebook_load_preserves_library_index_metadata() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path();

        let mut nb_ok = Notebook::new("Healthy NB", PageTemplate::Blank, Color32::WHITE);
        nb_ok.folder_id = Some("folder-a".to_string());
        let nb_ok_id = nb_ok.id.clone();
        save_notebook_to_dir(dir, &nb_ok).unwrap();

        let mut nb_fail = Notebook::new("Broken NB", PageTemplate::Ruled, Color32::DARK_BLUE);
        nb_fail.folder_id = Some("folder-b".to_string());
        let nb_fail_id = nb_fail.id.clone();
        // Tulis JSON korup ke notebook.json tanpa backup agar load_notebook_from_dir gagal
        let fail_dir = dir.join("notebooks").join(&nb_fail_id);
        std::fs::create_dir_all(&fail_dir).unwrap();
        std::fs::write(fail_dir.join("notebook.json"), "{corrupt_json").unwrap();

        let mut index = crate::model::LibraryIndex::default();
        index
            .notebooks
            .push(crate::model::NotebookMeta::from_notebook(&nb_ok));
        index
            .notebooks
            .push(crate::model::NotebookMeta::from_notebook(&nb_fail));
        save_library_index_to_dir(dir, &index).unwrap();

        let app = RustNotesApp::load_from_dir(dir);
        assert_eq!(app.library.notebooks.len(), 1);
        assert_eq!(app.library.notebooks[0].id, nb_ok_id);
        assert_eq!(app.unloaded_notebook_metas.len(), 1);
        assert_eq!(app.unloaded_notebook_metas[0].id, nb_fail_id);
        assert_eq!(
            app.unloaded_notebook_metas[0].folder_id.as_deref(),
            Some("folder-b")
        );

        // Simulasikan save/sync index di tengah sesi: metadata notebook yang gagal dimuat harus tetap ada!
        app.sync_library_index_to_dir(dir);
        let reloaded_index = crate::storage::load_library_index_from_dir(dir);
        assert_eq!(reloaded_index.index.notebooks.len(), 2);
        let preserved = reloaded_index
            .index
            .notebooks
            .iter()
            .find(|m| m.id == nb_fail_id)
            .expect("failed notebook metadata must be preserved in library_index.json");
        assert_eq!(preserved.folder_id.as_deref(), Some("folder-b"));
    }

    #[test]
    fn test_load_status_reflects_corrupt_and_newer_version_legacy_library() {
        // 1. Corrupt library.json -> CorruptedQuarantined
        let temp_corrupt = tempfile::tempdir().unwrap();
        let dir_corrupt = temp_corrupt.path();
        std::fs::write(dir_corrupt.join("library.json"), "{not-valid-json").unwrap();
        // Pastikan library_index.json belum ada sehingga fallback ke load_library_from_dir?
        // Catatan: load_library_index_from_dir memanggil load_library_from_dir saat migrasi legacy;
        // di sini kita uji langsung load_from_dir ketika FreshIndex + legacy library.json dengan versi lebih baru.
        let temp_future = tempfile::tempdir().unwrap();
        let dir_future = temp_future.path();
        let future_lib = serde_json::json!({
            "schema_version": crate::model::CURRENT_SCHEMA_VERSION + 5,
            "notebooks": [],
            "folders": []
        });
        std::fs::write(dir_future.join("library.json"), future_lib.to_string()).unwrap();
        // Buat library_index.json kosong agar tidak dianggap migrasi, atau uji load_library_from_dir
        let lr = crate::storage::load_library_from_dir(dir_future);
        assert!(matches!(
            lr.status,
            crate::storage::LoadStatus::NewerVersionReadOnly { .. }
        ));

        // Uji langsung RustNotesApp::load_from_dir ketika index dipulihkan dari previous
        let mut idx = crate::model::LibraryIndex::default();
        let nb = Notebook::new("Idx NB", PageTemplate::Blank, Color32::WHITE);
        save_notebook_to_dir(dir_corrupt, &nb).unwrap();
        idx.notebooks
            .push(crate::model::NotebookMeta::from_notebook(&nb));
        std::fs::write(
            dir_corrupt.join("library_index.previous.json"),
            serde_json::to_string_pretty(&idx).unwrap(),
        )
        .unwrap();
        let app = RustNotesApp::load_from_dir(dir_corrupt);
        assert_eq!(
            app.load_status,
            crate::storage::LoadStatus::RecoveredFromPrevious
        );
    }

    #[test]
    fn test_import_library_backup_validates_migrates_and_guards_errors() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path();

        let mut app = RustNotesApp::load_from_dir(dir);
        let original_nb = Notebook::new("Existing NB", PageTemplate::Blank, Color32::WHITE);
        let original_id = original_nb.id.clone();
        app.library.notebooks.push(original_nb);
        app.open_notebook_ids.push(original_id.clone());
        app.active_notebook_id = Some(original_id.clone());

        // 1. JSON korup tidak boleh mengubah library yang ada
        let bad_file = dir.join("bad_backup.json");
        std::fs::write(&bad_file, "{bad_json").unwrap();
        assert!(app.import_library_backup_to_dir(&bad_file, dir).is_err());
        assert_eq!(app.library.notebooks.len(), 1);
        assert_eq!(app.library.notebooks[0].id, original_id);

        // 2. Schema versi masa depan harus ditolak tanpa mengubah library
        let future_file = dir.join("future_backup.json");
        let future_json = serde_json::json!({
            "schema_version": crate::model::CURRENT_SCHEMA_VERSION + 9,
            "notebooks": [],
            "folders": []
        });
        std::fs::write(&future_file, future_json.to_string()).unwrap();
        assert!(app.import_library_backup_to_dir(&future_file, dir).is_err());
        assert_eq!(app.library.notebooks.len(), 1);
        assert_eq!(app.library.notebooks[0].id, original_id);

        // 3. Mode read-only harus menolak import
        let valid_v1_file = dir.join("valid_v1_backup.json");
        let v1_json = serde_json::json!({
            "version": 1,
            "notebooks": [
                {
                    "id": "nb-imported-v1",
                    "title": "Imported v1 Notebook",
                    "cover_color": { "r": 20, "g": 40, "b": 80, "a": 255 },
                    "pages": [],
                    "created_at": 1700000000,
                    "updated_at": 1700000000
                }
            ],
            "folders": []
        });
        std::fs::write(&valid_v1_file, v1_json.to_string()).unwrap();

        app.is_read_only = true;
        assert!(
            app.import_library_backup_to_dir(&valid_v1_file, dir)
                .is_err()
        );
        assert_eq!(app.library.notebooks[0].id, original_id);

        // 4. Import valid v1 dalam mode normal harus migrasi, simpan ke disk, dan reset state sesi
        app.is_read_only = false;
        app.import_library_backup_to_dir(&valid_v1_file, dir)
            .expect("valid v1 backup import must succeed");
        assert_eq!(
            app.library.schema_version,
            crate::model::CURRENT_SCHEMA_VERSION
        );
        assert_eq!(app.library.notebooks.len(), 1);
        assert_eq!(app.library.notebooks[0].id, "nb-imported-v1");
        assert!(app.open_notebook_ids.is_empty());
        assert!(app.active_notebook_id.is_none());
        assert!(
            dir.join("notebooks")
                .join("nb-imported-v1")
                .join("notebook.json")
                .exists(),
            "imported notebook must be persisted to disk"
        );
    }

    #[test]
    fn test_permanent_delete_and_empty_trash() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path();

        let mut app = RustNotesApp::load_from_dir(dir);
        let mut nb_trash = Notebook::new("Trashed NB", PageTemplate::Blank, Color32::WHITE);
        nb_trash.trashed = true;
        let trash_id = nb_trash.id.clone();
        save_notebook_to_dir(dir, &nb_trash).unwrap();

        let nb_keep = Notebook::new("Active NB", PageTemplate::Blank, Color32::WHITE);
        let keep_id = nb_keep.id.clone();
        save_notebook_to_dir(dir, &nb_keep).unwrap();

        app.library.notebooks.push(nb_trash);
        app.library.notebooks.push(nb_keep);
        app.sync_library_index_to_dir(dir);

        // Notebook yang tidak di-trash tidak boleh dihapus permanen
        assert!(
            app.permanently_delete_notebook_from_dir(&keep_id, dir)
                .is_err()
        );
        assert!(dir.join("notebooks").join(&keep_id).exists());

        // Kosongkan sampah harus menghapus nb_trash di memori & disk, mempertahankan nb_keep
        let count = app.empty_trash_from_dir(dir).expect("empty trash");
        assert_eq!(count, 1);
        assert!(!dir.join("notebooks").join(&trash_id).exists());
        assert!(dir.join("notebooks").join(&keep_id).exists());
        assert_eq!(app.library.notebooks.len(), 1);
        assert_eq!(app.library.notebooks[0].id, keep_id);
    }
}

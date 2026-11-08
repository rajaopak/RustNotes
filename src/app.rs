use crate::canvas::{
    ActiveTool, CanvasState, EraserMode, render_page_canvas, render_zoom_window_canvas,
};
use crate::model::{
    AppSettings, CustomTemplate, Library, Notebook, OutlineItem, Page, PageOrientation,
    PageTemplate, PaperSizePreset, ShapeKind, ThemeMode,
};
use crate::storage::{
    LoadStatus, app_storage_dir, export_notebook_to_pdf, export_page_to_png, import_pdf_to_pages,
    load_custom_templates, load_library, load_library_index_from_dir, load_notebook_from_dir,
    load_settings_from_dir, save_asset_to_notebook, save_custom_template,
    save_library_index_to_dir, save_notebook_to_dir, save_settings_to_dir,
};
use eframe::egui::{
    self, Align, Button, Color32, CornerRadius, FontId, Layout, Painter, Pos2, Rect, RichText,
    ScrollArea, Sense, Stroke, StrokeKind, Ui, Vec2, Visuals,
};
use std::collections::{HashMap, HashSet};
use std::time::Instant;

#[cfg(target_os = "windows")]
unsafe extern "system" {
    fn GetClipboardSequenceNumber() -> u32;
}

fn get_windows_clipboard_sequence() -> u32 {
    #[cfg(target_os = "windows")]
    unsafe {
        GetClipboardSequenceNumber()
    }
    #[cfg(not(target_os = "windows"))]
    0
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewMode {
    SinglePage,
    ContinuousVertical,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SidebarTab {
    Thumbnails,
    Bookmarks,
    Outline,
}

pub struct RustNotesApp {
    library: Library,
    open_notebook_ids: Vec<String>,
    active_notebook_id: Option<String>,
    current_page_index: usize,
    canvas_state: CanvasState,
    load_status: LoadStatus,
    is_read_only: bool,
    toast_message: Option<String>,

    // Library UI state
    current_nav_view: NavView,
    search_query: String,
    show_new_notebook_modal: bool,
    new_notebook_title: String,
    new_notebook_template: PageTemplate,
    new_notebook_cover: Color32,
    #[allow(dead_code)]
    new_notebook_paper_color: Color32,
    new_notebook_preset: PaperSizePreset,
    new_notebook_orientation: PageOrientation,
    new_notebook_use_custom_size: bool,
    new_notebook_custom_width: f32,
    new_notebook_custom_height: f32,

    // Fase 2: Page Management & View Mode
    view_mode: ViewMode,
    show_thumbnail_sidebar: bool,
    show_page_resize_modal: bool,
    resize_target_page_index: usize,
    resize_target_preset: PaperSizePreset,
    resize_target_orientation: PageOrientation,
    resize_use_custom_size: bool,
    resize_custom_width: f32,
    resize_custom_height: f32,
    resize_target_template: PageTemplate,
    resize_apply_all_pages: bool,
    doc_undo_stacks: HashMap<String, Vec<(Vec<Page>, usize)>>,
    doc_redo_stacks: HashMap<String, Vec<(Vec<Page>, usize)>>,

    // GoodNotes-style Pop-ups & Viewport Camera State
    show_more_menu_popup: bool,
    show_tool_popup: bool,
    show_doc_title_popup: bool,
    show_zoom_slider_expanded: bool,
    canvas_scroll_offset: Vec2,
    pending_scroll_offset: Option<Vec2>,
    canvas_pan_offset: Vec2,
    last_canvas_viewport_rect: Rect,

    // Fase 6: Dirty flag + debounced per-notebook save
    dirty_notebooks: HashSet<String>,
    last_modified: HashMap<String, Instant>,

    // Fase 4: Shortcut cheat sheet modal
    show_shortcut_modal: bool,

    // Fase 5: Nested Folders, Outline/Bookmarks, Search, Custom Templates
    current_folder_id: Option<String>,
    show_new_folder_modal: bool,
    new_folder_name: String,
    new_folder_color: Color32,
    moving_notebook_id: Option<String>,

    sidebar_tab: SidebarTab,
    show_add_outline_modal: bool,
    new_outline_title: String,

    show_in_doc_search: bool,
    doc_search_query: String,
    doc_search_current_match: usize,

    custom_templates: Vec<CustomTemplate>,
    show_save_template_modal: bool,
    new_template_name: String,

    // Fase 7: About dialog modal
    show_about_modal: bool,

    // Fase 8.3, 8.4, 8.5, 8.7, 8.8: User settings (theme, pen presets, color swatches, zoom window scale, palm rejection, language)
    settings: AppSettings,

    // Fase 8.6: Handwriting OCR inspector modal
    show_ocr_modal: bool,

    // Add Page Modal
    show_add_page_modal: bool,
    add_page_template: PageTemplate,
    add_page_preset: PaperSizePreset,
    add_page_orientation: PageOrientation,
    add_page_target_pos: AddPagePosition,

    // Thumbnail Multi-Select & Drag-Drop Reorder
    thumb_selected_pages: std::collections::BTreeSet<usize>,
    thumb_select_mode: bool,
    dragged_thumb_page: Option<usize>,

    // Windows Clipboard Sequence & Timestamp Tracking
    win_clipboard_seq: u32,
    win_clipboard_timestamp: Instant,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AddPagePosition {
    AfterCurrent,
    BeforeCurrent,
    AtEnd,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum NavView {
    Documents,
    Favorites,
    Trash,
}

const COVER_PALETTE: [Color32; 8] = [
    Color32::from_rgb(189, 173, 150), // Sand/Tan
    Color32::from_rgb(49, 85, 119),   // Classic Navy
    Color32::from_rgb(112, 153, 141), // Sage Green
    Color32::from_rgb(197, 131, 120), // Dusty Rose
    Color32::from_rgb(198, 167, 95),  // Mustard Yellow
    Color32::from_rgb(112, 102, 140), // Purple Slate
    Color32::from_rgb(52, 57, 71),    // Charcoal Gray
    Color32::from_rgb(185, 200, 214), // Sky Blue
];

impl Default for RustNotesApp {
    fn default() -> Self {
        // Fase 6: Coba load dari library_index (format baru) dulu.
        // Jika tidak ada, fallback ke legacy load_library() untuk backward compat.
        let storage_dir = app_storage_dir();
        let index_res = load_library_index_from_dir(&storage_dir);

        // Bangun Library dari index: muat setiap notebook dari disk
        let mut library = Library::default();
        let mut load_errors: Vec<String> = Vec::new();
        for meta in &index_res.index.notebooks {
            match load_notebook_from_dir(&storage_dir, &meta.id) {
                Ok(nb) => library.notebooks.push(nb),
                Err(e) => load_errors.push(e),
            }
        }
        library.folders = index_res.index.folders;

        // Jika index kosong dan tidak ada migrasi, cek apakah ada legacy library.json
        let (is_read_only, toast) = if !load_errors.is_empty() {
            (
                false,
                Some(format!(
                    "Peringatan: {} notebook gagal dimuat (mungkin rusak).",
                    load_errors.len()
                )),
            )
        } else {
            match index_res.status {
                crate::storage::LoadIndexStatus::MigratedFromLegacy => (
                    false,
                    Some("Data berhasil dimigrasikan ke format baru per-notebook.".to_string()),
                ),
                crate::storage::LoadIndexStatus::RecoveredFromPrevious => (
                    false,
                    Some("Index library dipulihkan dari cadangan.".to_string()),
                ),
                _ => {
                    // Juga cek load status legacy (untuk kasus file lama yg belum dimigrasikan)
                    let legacy_res = {
                        // Jika index fresh dan library kosong, cek apakah ada error dari load lama
                        if library.notebooks.is_empty()
                            && index_res.status == crate::storage::LoadIndexStatus::FreshIndex
                        {
                            // Coba load library lama (kalau ternyata ada)
                            Some(load_library())
                        } else {
                            None
                        }
                    };
                    if let Some(lr) = legacy_res {
                        match &lr.status {
                            LoadStatus::RecoveredFromPrevious => {
                                library = lr.library;
                                (
                                    false,
                                    Some("Data dipulihkan dari cadangan otomatis.".to_string()),
                                )
                            }
                            LoadStatus::CorruptedQuarantined { path } => {
                                library = lr.library;
                                (
                                    false,
                                    Some(format!(
                                        "Berkas library rusak dikarantina ke: {}",
                                        path.display()
                                    )),
                                )
                            }
                            LoadStatus::NewerVersionReadOnly { version } => {
                                library = lr.library;
                                (
                                    true,
                                    Some(format!(
                                        "Mode Baca-Saja: Berkas versi {} lebih baru dari aplikasi (v{}).",
                                        version,
                                        crate::model::CURRENT_SCHEMA_VERSION
                                    )),
                                )
                            }
                            _ => (false, None),
                        }
                    } else {
                        (false, None)
                    }
                }
            }
        };

        // Simpan initial index agar konsisten
        let mut init_index = crate::model::LibraryIndex::default();
        for nb in &library.notebooks {
            init_index
                .notebooks
                .push(crate::model::NotebookMeta::from_notebook(nb));
        }
        init_index.folders = library.folders.clone();
        let _ = save_library_index_to_dir(&storage_dir, &init_index);

        Self {
            library,
            open_notebook_ids: Vec::new(),
            active_notebook_id: None,
            current_page_index: 0,
            canvas_state: CanvasState::default(),
            load_status: LoadStatus::LoadedDirect,
            is_read_only,
            toast_message: toast,
            current_nav_view: NavView::Documents,
            search_query: String::new(),
            show_new_notebook_modal: false,
            new_notebook_title: String::new(),
            new_notebook_template: PageTemplate::Ruled,
            new_notebook_cover: COVER_PALETTE[0],
            new_notebook_paper_color: Color32::from_rgb(255, 254, 250),
            new_notebook_preset: PaperSizePreset::A4,
            new_notebook_orientation: PageOrientation::Portrait,
            new_notebook_use_custom_size: false,
            new_notebook_custom_width: 595.0,
            new_notebook_custom_height: 842.0,
            view_mode: ViewMode::SinglePage,
            show_thumbnail_sidebar: false,
            show_page_resize_modal: false,
            resize_target_page_index: 0,
            resize_target_preset: PaperSizePreset::A4,
            resize_target_orientation: PageOrientation::Portrait,
            resize_use_custom_size: false,
            resize_custom_width: 595.0,
            resize_custom_height: 842.0,
            resize_target_template: PageTemplate::Ruled,
            resize_apply_all_pages: false,
            doc_undo_stacks: HashMap::new(),
            doc_redo_stacks: HashMap::new(),
            show_more_menu_popup: false,
            show_tool_popup: false,
            show_doc_title_popup: false,
            show_zoom_slider_expanded: false,
            canvas_scroll_offset: Vec2::ZERO,
            pending_scroll_offset: None,
            canvas_pan_offset: Vec2::ZERO,
            last_canvas_viewport_rect: Rect::NOTHING,
            dirty_notebooks: HashSet::new(),
            last_modified: HashMap::new(),
            show_shortcut_modal: false,

            // Fase 5
            current_folder_id: None,
            show_new_folder_modal: false,
            new_folder_name: String::new(),
            new_folder_color: Color32::from_rgb(136, 179, 213),
            moving_notebook_id: None,
            sidebar_tab: SidebarTab::Thumbnails,
            show_add_outline_modal: false,
            new_outline_title: String::new(),
            show_in_doc_search: false,
            doc_search_query: String::new(),
            doc_search_current_match: 0,
            custom_templates: load_custom_templates(&storage_dir),
            show_save_template_modal: false,
            new_template_name: String::new(),
            show_about_modal: false,
            settings: load_settings_from_dir(&storage_dir),
            show_ocr_modal: false,
            show_add_page_modal: false,
            add_page_template: PageTemplate::Ruled,
            add_page_preset: PaperSizePreset::A4,
            add_page_orientation: PageOrientation::Portrait,
            add_page_target_pos: AddPagePosition::AfterCurrent,
            thumb_selected_pages: std::collections::BTreeSet::new(),
            thumb_select_mode: false,
            dragged_thumb_page: None,
            win_clipboard_seq: get_windows_clipboard_sequence(),
            win_clipboard_timestamp: Instant::now(),
        }
    }
}

impl eframe::App for RustNotesApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Fase 8.9: Aktifkan AccessKit bawaan egui untuk pembaca layar & aksesibilitas
        ctx.enable_accesskit();

        // Fase 8.5: Sinkronkan status Palm Rejection ke CanvasState
        self.canvas_state.palm_rejection_enabled = self.settings.palm_rejection;

        // Apply Goodnotes theme styling (Dark / Light / System OS Windows)
        setup_goodnotes_theme(ctx, self.settings.theme_mode);

        // Fase 6: Debounced per-notebook save (1.5 detik setelah modifikasi terakhir)
        if !self.dirty_notebooks.is_empty() && !self.is_read_only {
            let debounce = std::time::Duration::from_millis(1500);
            let storage_dir = app_storage_dir();
            let now = Instant::now();
            let ready_to_save: Vec<String> = self
                .dirty_notebooks
                .iter()
                .filter(|id| {
                    self.last_modified
                        .get(*id)
                        .map(|t| now.duration_since(*t) >= debounce)
                        .unwrap_or(true)
                })
                .cloned()
                .collect();

            for nb_id in &ready_to_save {
                if let Some(nb) = self.library.notebooks.iter().find(|n| &n.id == nb_id) {
                    let result = save_notebook_to_dir(&storage_dir, nb);
                    if result.is_err() {
                        self.toast_message = Some(format!(
                            "⚠ Gagal menyimpan notebook '{}'. Coba lagi.",
                            nb.title
                        ));
                    }
                }
                self.dirty_notebooks.remove(nb_id);
                self.last_modified.remove(nb_id);
            }

            // Juga perbarui index jika ada yang berubah
            if !ready_to_save.is_empty() {
                self.sync_library_index();
            }

            // Minta repaint jika masih ada yang menunggu debounce
            if !self.dirty_notebooks.is_empty() {
                ctx.request_repaint_after(std::time::Duration::from_millis(300));
            }
        }

        // Render toast / banner if active
        let mut dismiss_toast = false;
        if let Some(msg) = &self.toast_message {
            egui::TopBottomPanel::bottom("status_banner")
                .exact_height(34.0)
                .show(ctx, |ui| {
                    ui.horizontal_centered(|ui| {
                        let is_err = self.is_read_only
                            || matches!(self.load_status, LoadStatus::CorruptedQuarantined { .. });
                        let text_color = if is_err {
                            Color32::from_rgb(255, 120, 120)
                        } else {
                            Color32::from_rgb(120, 220, 140)
                        };
                        ui.label(RichText::new(msg).color(text_color).strong().size(12.0));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.button("✕").clicked() {
                                dismiss_toast = true;
                            }
                        });
                    });
                });
        }
        if dismiss_toast {
            self.toast_message = None;
        }

        self.canvas_state.active_notebook_id = self.active_notebook_id.clone();

        // Fase 3: Handle Drag & Drop Files
        let dropped_files = ctx.input(|i| i.raw.dropped_files.clone());
        for file in dropped_files {
            if let Some(path) = &file.path {
                let ext = path
                    .extension()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                if ext == "pdf" {
                    if let Some(id) = &self.active_notebook_id
                        && let Some(idx) = self.library.notebooks.iter().position(|n| &n.id == id)
                    {
                        self.import_pdf_pages_to_active(idx, path);
                    } else {
                        self.import_pdf_as_notebook(path);
                    }
                } else if ["png", "jpg", "jpeg", "webp"].contains(&ext.as_str())
                    && let Some(id) = &self.active_notebook_id
                    && let Some(idx) = self.library.notebooks.iter().position(|n| &n.id == id)
                    && let Ok(bytes) = std::fs::read(path)
                {
                    self.insert_image_to_page(idx, &bytes, &ext, None);
                    self.toast_message =
                        Some("✅ Gambar berhasil disisipkan via Drag & Drop.".to_string());
                }
            }
        }

        if self.active_notebook_id.is_some() {
            self.render_editor_view(ctx);
        } else {
            self.render_library_view(ctx);
        }
    }
}

fn resolve_theme_mode(ctx: &egui::Context, theme_mode: ThemeMode) -> ThemeMode {
    match theme_mode {
        ThemeMode::Dark => ThemeMode::Dark,
        ThemeMode::Light => ThemeMode::Light,
        ThemeMode::System => match ctx.system_theme() {
            Some(egui::Theme::Light) => ThemeMode::Light,
            _ => ThemeMode::Dark,
        },
    }
}

fn setup_goodnotes_theme(ctx: &egui::Context, theme_mode: ThemeMode) {
    match resolve_theme_mode(ctx, theme_mode) {
        ThemeMode::Dark | ThemeMode::System => {
            let mut visuals = Visuals::dark();
            visuals.panel_fill = Color32::from_rgb(29, 27, 28); // #1d1b1c
            visuals.window_fill = Color32::from_rgb(45, 43, 44); // #2d2b2c
            visuals.extreme_bg_color = Color32::from_rgb(20, 19, 19); // #141313
            visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(45, 43, 44);
            visuals.widgets.noninteractive.bg_stroke =
                Stroke::new(1.0_f32, Color32::from_rgb(60, 57, 57));
            visuals.widgets.inactive.bg_fill = Color32::from_rgb(45, 43, 44);
            visuals.widgets.inactive.bg_stroke =
                Stroke::new(1.0_f32, Color32::from_rgb(60, 57, 57));
            visuals.widgets.hovered.bg_fill = Color32::from_rgb(56, 53, 54);
            visuals.widgets.hovered.bg_stroke =
                Stroke::new(1.5_f32, Color32::from_rgb(76, 147, 236));
            visuals.widgets.active.bg_fill = Color32::from_rgb(0, 114, 206);
            visuals.selection.bg_fill = Color32::from_rgb(39, 54, 91); // #27365b
            visuals.selection.stroke = Stroke::new(1.5_f32, Color32::from_rgb(76, 147, 236));
            ctx.set_visuals(visuals);
        }
        ThemeMode::Light => {
            let mut visuals = Visuals::light();
            visuals.panel_fill = Color32::from_rgb(244, 243, 239); // #f4f3ef
            visuals.window_fill = Color32::from_rgb(252, 251, 249);
            visuals.extreme_bg_color = Color32::from_rgb(255, 255, 255);
            visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(235, 233, 228);
            visuals.widgets.noninteractive.bg_stroke =
                Stroke::new(1.0_f32, Color32::from_rgb(205, 202, 196));
            visuals.widgets.inactive.bg_fill = Color32::from_rgb(232, 230, 225);
            visuals.widgets.inactive.bg_stroke =
                Stroke::new(1.0_f32, Color32::from_rgb(198, 195, 188));
            visuals.widgets.hovered.bg_fill = Color32::from_rgb(218, 224, 235);
            visuals.widgets.hovered.bg_stroke =
                Stroke::new(1.5_f32, Color32::from_rgb(0, 114, 206));
            visuals.widgets.active.bg_fill = Color32::from_rgb(0, 114, 206);
            visuals.selection.bg_fill = Color32::from_rgb(195, 220, 248);
            visuals.selection.stroke = Stroke::new(1.5_f32, Color32::from_rgb(0, 114, 206));
            ctx.set_visuals(visuals);
        }
    }
}

impl RustNotesApp {
    /// Buka modal pengaturan ukuran & template halaman untuk halaman tertentu (`page_idx`).
    fn open_page_resize_modal(&mut self, nb_idx: usize, page_idx: usize) {
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
    fn open_add_page_modal(&mut self, nb_idx: usize) {
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
    fn page_y_offset(&self, nb_idx: usize, page_idx: usize, zoom: f32) -> f32 {
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
    fn navigate_to_page(&mut self, nb_idx: usize, page_idx: usize) {
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
    fn set_view_mode(&mut self, nb_idx: usize, new_mode: ViewMode) {
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
    fn apply_zoom_anchored(
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
    fn mark_dirty(&mut self, notebook_id: &str) {
        if self.is_read_only {
            return;
        }
        self.dirty_notebooks.insert(notebook_id.to_string());
        self.last_modified
            .insert(notebook_id.to_string(), Instant::now());
    }

    /// Fase 6: Simpan satu notebook segera (force-save, bypass debounce).
    fn force_save_notebook(&mut self, notebook_id: &str) {
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
    fn flush_all_dirty(&mut self) {
        if self.is_read_only {
            return;
        }
        let ids: Vec<String> = self.dirty_notebooks.iter().cloned().collect();
        for id in ids {
            self.force_save_notebook(&id);
        }
    }

    /// Fase 6: Sinkronkan library_index.json dari state library saat ini.
    fn sync_library_index(&self) {
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
    fn persist_library(&mut self) {
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

    fn push_doc_undo(&mut self, nb_idx: usize) {
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

    fn undo_document(&mut self, nb_id: &str) -> bool {
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

    fn redo_document(&mut self, nb_id: &str) -> bool {
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

    fn perform_undo(&mut self, nb_idx: usize) {
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

    fn perform_redo(&mut self, nb_idx: usize) {
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
    fn insert_image_to_page(
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

            self.push_doc_undo(nb_idx);
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

            self.library.notebooks[nb_idx].pages[cur_page_idx]
                .objects
                .push(img_obj);
            self.canvas_state.selected_ids = vec![new_img_id];
            self.canvas_state.current_tool = ActiveTool::Lasso;
            self.canvas_state
                .spatial_grid
                .rebuild(&self.library.notebooks[nb_idx].pages[cur_page_idx].objects);
            self.mark_dirty(&nb_id);
            return true;
        }
        false
    }

    // Fase 3: Menempel gambar dari clipboard (Ctrl+V)
    fn try_paste_clipboard_image(&mut self, nb_idx: usize, target_pos: Option<Pos2>) -> bool {
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
    fn import_pdf_as_notebook(&mut self, pdf_path: &std::path::Path) -> bool {
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
    fn import_pdf_pages_to_active(&mut self, nb_idx: usize, pdf_path: &std::path::Path) -> bool {
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

    fn render_add_page_modal(&mut self, ctx: &egui::Context, nb_idx: usize) {
        if !self.show_add_page_modal {
            return;
        }
        let (cur_tmpl, cur_w, cur_h) = {
            if let Some(p) = self.library.notebooks[nb_idx]
                .pages
                .get(self.current_page_index)
            {
                (p.template, p.width, p.height)
            } else {
                (PageTemplate::Ruled, 595.0, 842.0)
            }
        };

        let mut close_modal = false;

        egui::Window::new("➕ Atur & Tambah Halaman Baru")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.set_width(380.0);
                ui.add_space(4.0);

                // Info Template Saat Ini
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("📄 Halaman Aktif:").strong());
                        ui.label(format!(
                            "Hal {} dari {}",
                            self.current_page_index + 1,
                            self.library.notebooks[nb_idx].pages.len()
                        ));
                    });
                    ui.label(
                        RichText::new(format!(
                            "Template: {:?}  |  Ukuran: {:.0} × {:.0} pt",
                            cur_tmpl, cur_w, cur_h
                        ))
                        .size(11.5)
                        .color(Color32::from_rgb(130, 195, 255)),
                    );
                });

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(6.0);

                // Opsi Penempatan Halaman
                ui.label(RichText::new("Posisi Penyisipan:").strong());
                ui.horizontal(|ui| {
                    ui.radio_value(
                        &mut self.add_page_target_pos,
                        AddPagePosition::AfterCurrent,
                        "Setelah Hal Ini",
                    );
                    ui.radio_value(
                        &mut self.add_page_target_pos,
                        AddPagePosition::BeforeCurrent,
                        "Sebelum Hal Ini",
                    );
                    ui.radio_value(
                        &mut self.add_page_target_pos,
                        AddPagePosition::AtEnd,
                        "Di Akhir Dokumen",
                    );
                });

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(6.0);

                // Opsi Jenis Halaman
                ui.label(RichText::new("Pilihan Halaman Baru:").strong());

                // 1. Template Standar
                ui.group(|ui| {
                    ui.label(RichText::new("1. Buat dengan Template Pola Kertas:").strong());
                    ui.horizontal_wrapped(|ui| {
                        let tmpls = [
                            (PageTemplate::Ruled, "Ruled"),
                            (PageTemplate::Grid, "Grid"),
                            (PageTemplate::Dotted, "Dotted"),
                            (PageTemplate::Blank, "Blank"),
                            (PageTemplate::Cornell, "Cornell"),
                        ];
                        for (t, label) in tmpls {
                            if ui
                                .selectable_label(self.add_page_template == t, label)
                                .clicked()
                            {
                                self.add_page_template = t;
                            }
                        }
                    });

                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.label("Ukuran:");
                        for preset in PaperSizePreset::ALL {
                            let is_sel = self.add_page_preset == preset;
                            let short_name = preset.name().split(" (").next().unwrap_or("");
                            if ui.selectable_label(is_sel, short_name).clicked() {
                                self.add_page_preset = preset;
                            }
                        }
                    });

                    ui.horizontal(|ui| {
                        ui.label("Orientasi:");
                        ui.radio_value(
                            &mut self.add_page_orientation,
                            PageOrientation::Portrait,
                            "Portrait",
                        );
                        ui.radio_value(
                            &mut self.add_page_orientation,
                            PageOrientation::Landscape,
                            "Landscape",
                        );
                    });
                });

                ui.add_space(6.0);

                // 2. Impor PDF sebagai Halaman
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("2. Sisipkan Berkas PDF:").strong());
                        if ui.button("📥 Pilih PDF...").clicked()
                            && let Some(path) = rfd::FileDialog::new()
                                .add_filter("PDF Document", &["pdf"])
                                .pick_file()
                        {
                            self.import_pdf_pages_to_active(nb_idx, &path);
                            close_modal = true;
                        }
                    });
                });

                ui.add_space(6.0);

                // 3. Gunakan Gambar sebagai Halaman
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("3. Gunakan Gambar Latar:").strong());
                        if ui.button("🖼 Pilih Gambar...").clicked()
                            && let Some(path) = rfd::FileDialog::new()
                                .add_filter("Image", &["png", "jpg", "jpeg", "webp"])
                                .pick_file()
                            && let Ok(img_bytes) = std::fs::read(&path)
                        {
                            let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("png");
                            let storage_dir = app_storage_dir();
                            let nb_id = self.library.notebooks[nb_idx].id.clone();
                            if let Ok(asset_id) =
                                save_asset_to_notebook(&storage_dir, &nb_id, &img_bytes, ext)
                                && let Ok(dyn_img) = image::load_from_memory(&img_bytes)
                            {
                                let page_w = dyn_img.width() as f32;
                                let page_h = dyn_img.height() as f32;
                                let mut new_page =
                                    Page::with_size(PageTemplate::Blank, page_w, page_h);
                                new_page.background =
                                    Some(crate::model::PageBackground::Image { asset_id });

                                self.push_doc_undo(nb_idx);
                                let insert_pos = match self.add_page_target_pos {
                                    AddPagePosition::BeforeCurrent => self.current_page_index,
                                    AddPagePosition::AfterCurrent => self.current_page_index + 1,
                                    AddPagePosition::AtEnd => {
                                        self.library.notebooks[nb_idx].pages.len()
                                    }
                                };
                                let clamped_pos =
                                    insert_pos.min(self.library.notebooks[nb_idx].pages.len());
                                self.library.notebooks[nb_idx]
                                    .pages
                                    .insert(clamped_pos, new_page);
                                self.navigate_to_page(nb_idx, clamped_pos);
                                self.mark_dirty(&nb_id);
                                self.toast_message = Some(
                                    "✅ Halaman berlatar gambar berhasil ditambahkan.".to_string(),
                                );
                                close_modal = true;
                            }
                        }
                    });
                });

                ui.add_space(10.0);
                ui.separator();
                ui.add_space(6.0);

                // Tombol Batal & Tambah
                ui.horizontal(|ui| {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let add_btn = Button::new(
                            RichText::new("➕ Tambah Halaman")
                                .strong()
                                .color(Color32::WHITE),
                        )
                        .fill(Color32::from_rgb(0, 114, 206));

                        if ui.add(add_btn).clicked() {
                            let (w, h) = self.add_page_preset.dimensions(self.add_page_orientation);
                            self.push_doc_undo(nb_idx);
                            let target_idx = match self.add_page_target_pos {
                                AddPagePosition::BeforeCurrent => self.current_page_index,
                                AddPagePosition::AfterCurrent => self.current_page_index + 1,
                                AddPagePosition::AtEnd => {
                                    self.library.notebooks[nb_idx].pages.len()
                                }
                            };
                            let new_idx = self.library.notebooks[nb_idx].insert_page(
                                target_idx,
                                self.add_page_template,
                                w,
                                h,
                            );
                            self.navigate_to_page(nb_idx, new_idx);
                            let nb_id = self.library.notebooks[nb_idx].id.clone();
                            self.mark_dirty(&nb_id);
                            close_modal = true;
                        }

                        if ui.button("Batal").clicked() {
                            close_modal = true;
                        }
                    });
                });
            });

        if close_modal {
            self.show_add_page_modal = false;
        }
    }

    fn render_library_view(&mut self, ctx: &egui::Context) {
        // Document Tabs Bar if any documents are opened
        if !self.open_notebook_ids.is_empty() {
            egui::TopBottomPanel::top("library_tabs")
                .exact_height(38.0)
                .show(ctx, |ui| {
                    ui.painter().rect_filled(
                        ui.max_rect(),
                        0.0,
                        Color32::from_rgb(40, 72, 114), // #284872
                    );
                    ui.horizontal_centered(|ui| {
                        ui.add_space(8.0);
                        let mut to_close = None;
                        for id in &self.open_notebook_ids {
                            if let Some(note) = self.library.notebooks.iter().find(|n| &n.id == id)
                            {
                                let is_active = self.active_notebook_id.as_ref() == Some(id);
                                let tab_color = if is_active {
                                    Color32::from_rgb(54, 95, 145) // #365f91
                                } else {
                                    Color32::TRANSPARENT
                                };
                                let (rect, resp) =
                                    ui.allocate_exact_size(Vec2::new(180.0, 32.0), Sense::click());
                                ui.painter().rect_filled(
                                    rect,
                                    CornerRadius {
                                        nw: 6,
                                        ne: 6,
                                        se: 0,
                                        sw: 0,
                                    },
                                    tab_color,
                                );
                                ui.painter().text(
                                    rect.left_center() + Vec2::new(12.0, 0.0),
                                    egui::Align2::LEFT_CENTER,
                                    format!("📖  {}", note.title),
                                    FontId::proportional(13.0),
                                    Color32::WHITE,
                                );
                                if resp.clicked() {
                                    self.active_notebook_id = Some(id.clone());
                                }
                                // Tab close X button
                                let close_rect = Rect::from_center_size(
                                    rect.right_center() - Vec2::new(16.0, 0.0),
                                    Vec2::new(18.0, 18.0),
                                );
                                if ui.put(close_rect, Button::new("✕").frame(false)).clicked() {
                                    to_close = Some(id.clone());
                                }
                            }
                        }
                        if let Some(id) = to_close {
                            self.open_notebook_ids.retain(|i| i != &id);
                            if self.active_notebook_id.as_ref() == Some(&id) {
                                self.active_notebook_id = self.open_notebook_ids.last().cloned();
                            }
                        }
                    });
                });
        }

        // Left Sidebar
        egui::SidePanel::left("library_sidebar")
            .resizable(false)
            .exact_width(240.0)
            .show(ctx, |ui| {
                ui.add_space(20.0);
                // Brand Header
                ui.horizontal(|ui| {
                    ui.add_space(10.0);
                    ui.label(RichText::new("📖").size(24.0));
                    ui.heading(
                        RichText::new("RustNotes")
                            .size(24.0)
                            .strong()
                            .color(Color32::WHITE),
                    );
                });
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.add_space(42.0);
                    ui.label(
                        RichText::new("Pure Native Windows Notebook")
                            .size(11.0)
                            .color(Color32::from_rgb(160, 156, 155)),
                    );
                });

                ui.add_space(32.0);
                ui.horizontal(|ui| {
                    ui.add_space(12.0);
                    ui.label(
                        RichText::new("MY LIBRARY")
                            .size(11.0)
                            .strong()
                            .color(Color32::from_rgb(140, 136, 135)),
                    );
                });
                ui.add_space(8.0);

                let nav_btn = |ui: &mut Ui,
                               selected: bool,
                               icon: &str,
                               label: &str,
                               count: Option<usize>|
                 -> bool {
                    let (rect, resp) =
                        ui.allocate_exact_size(Vec2::new(216.0, 42.0), Sense::click());
                    let bg = if selected {
                        Color32::from_rgb(39, 54, 91) // #27365b
                    } else if resp.hovered() {
                        Color32::from_rgb(50, 48, 48)
                    } else {
                        Color32::TRANSPARENT
                    };
                    let text_color = if selected {
                        Color32::from_rgb(76, 147, 236) // #4c93ec
                    } else {
                        Color32::from_rgb(240, 240, 240)
                    };
                    ui.painter().rect_filled(rect, 8.0, bg);
                    ui.painter().text(
                        rect.left_center() + Vec2::new(14.0, 0.0),
                        egui::Align2::LEFT_CENTER,
                        format!("{icon}  {label}"),
                        FontId::proportional(14.0),
                        text_color,
                    );
                    if let Some(c) = count
                        && c > 0
                    {
                        ui.painter().text(
                            rect.right_center() - Vec2::new(14.0, 0.0),
                            egui::Align2::RIGHT_CENTER,
                            format!("{c}"),
                            FontId::proportional(12.0),
                            Color32::GRAY,
                        );
                    }
                    resp.clicked()
                };

                let lang = self.settings.language;
                let docs_count = self.library.notebooks.iter().filter(|n| !n.trashed).count();
                if nav_btn(
                    ui,
                    self.current_nav_view == NavView::Documents,
                    "📁",
                    lang.tr("Dokumen", "Documents"),
                    Some(docs_count),
                ) {
                    self.current_nav_view = NavView::Documents;
                }
                let fav_count = self
                    .library
                    .notebooks
                    .iter()
                    .filter(|n| !n.trashed && n.favorite)
                    .count();
                if nav_btn(
                    ui,
                    self.current_nav_view == NavView::Favorites,
                    "⭐",
                    lang.tr("Favorit", "Favorites"),
                    Some(fav_count),
                ) {
                    self.current_nav_view = NavView::Favorites;
                }
                let trash_count = self.library.notebooks.iter().filter(|n| n.trashed).count();
                if nav_btn(
                    ui,
                    self.current_nav_view == NavView::Trash,
                    "🗑",
                    lang.tr("Sampah", "Trash"),
                    Some(trash_count),
                ) {
                    self.current_nav_view = NavView::Trash;
                }

                // Bottom Sidebar status & export/import
                ui.with_layout(Layout::bottom_up(Align::LEFT), |ui| {
                    ui.add_space(16.0);
                    ui.horizontal(|ui| {
                        ui.add_space(12.0);
                        ui.label(
                            RichText::new("⚡ 100% Rust & GPU Accelerated")
                                .size(11.0)
                                .color(Color32::from_rgb(120, 116, 115)),
                        );
                    });
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.add_space(12.0);
                        ui.label(
                            RichText::new("💾 Saved locally on this device")
                                .size(11.0)
                                .color(Color32::from_rgb(120, 116, 115)),
                        );
                    });
                    ui.add_space(12.0);

                    // Backup buttons
                    if ui.button("📥 Export Library Backup (.json)").clicked()
                        && let Some(path) = rfd::FileDialog::new()
                            .set_file_name("RustNotes-backup.json")
                            .save_file()
                        && let Ok(data) = serde_json::to_string_pretty(&self.library)
                    {
                        let _ = std::fs::write(path, data);
                    }
                    if ui.button("📤 Import Library Backup (.json)").clicked()
                        && let Some(path) = rfd::FileDialog::new()
                            .add_filter("JSON", &["json"])
                            .pick_file()
                        && let Ok(content) = std::fs::read_to_string(path)
                        && let Ok(lib) = serde_json::from_str::<Library>(&content)
                    {
                        self.library = lib;
                        self.persist_library();
                    }
                });
            });

        // Main Library Content
        egui::CentralPanel::default().show(ctx, |ui| {
            let lang = self.settings.language;
            ui.add_space(16.0);
            // Baris 1 Header: Judul di kiri, Pencarian & Pengaturan Cepat di kanan (tanpa tabrakan)
            ui.horizontal_wrapped(|ui| {
                ui.add_space(20.0);
                let title = match self.current_nav_view {
                    NavView::Documents => lang.tr("Dokumen", "Documents"),
                    NavView::Favorites => lang.tr("Favorit", "Favorites"),
                    NavView::Trash => lang.tr("Sampah", "Trash"),
                };
                ui.heading(
                    RichText::new(title)
                        .size(32.0)
                        .strong()
                        .color(Color32::WHITE),
                );

                ui.add_space(16.0);
                // Search bar
                ui.add(
                    egui::TextEdit::singleline(&mut self.search_query)
                        .hint_text(lang.tr("🔍 Cari catatan / OCR...", "🔍 Search notes / OCR..."))
                        .desired_width(200.0),
                );

                // Fase 8.7: Theme Mode toggle button (Dark / Light / System OS Windows)
                let theme_label = match self.settings.theme_mode {
                    ThemeMode::Dark => "🌙 Dark",
                    ThemeMode::Light => "☀ Light",
                    ThemeMode::System => "💻 System",
                };
                let theme_btn =
                    Button::new(RichText::new(theme_label).size(12.5).color(Color32::WHITE))
                        .fill(Color32::from_rgb(65, 65, 78))
                        .corner_radius(16.0)
                        .min_size(Vec2::new(76.0, 32.0));
                if ui
                    .add(theme_btn)
                    .on_hover_text(lang.tr(
                        "Siklus Tema: Gelap -> Terang -> Ikuti Sistem Windows",
                        "Cycle Theme: Dark -> Light -> Follow Windows System",
                    ))
                    .clicked()
                {
                    self.settings.theme_mode = self.settings.theme_mode.next();
                    let _ = save_settings_to_dir(&app_storage_dir(), &self.settings);
                }

                // Fase 8.8: Language Switcher (Bahasa Indonesia / Bahasa Inggris)
                let lang_btn =
                    Button::new(RichText::new(lang.badge()).size(12.5).color(Color32::WHITE))
                        .fill(Color32::from_rgb(58, 72, 92))
                        .corner_radius(16.0)
                        .min_size(Vec2::new(60.0, 32.0));
                if ui
                    .add(lang_btn)
                    .on_hover_text(lang.tr(
                        "Ganti Bahasa (Indonesia / Inggris)",
                        "Switch Language (Indonesian / English)",
                    ))
                    .clicked()
                {
                    self.settings.language = self.settings.language.toggle();
                    let _ = save_settings_to_dir(&app_storage_dir(), &self.settings);
                }

                // Fase 7: "About" button
                let about_btn = Button::new(
                    RichText::new(lang.tr("ℹ Tentang", "ℹ About"))
                        .size(12.5)
                        .color(Color32::WHITE),
                )
                .fill(Color32::from_rgb(40, 50, 65))
                .corner_radius(16.0)
                .min_size(Vec2::new(74.0, 32.0));
                if ui.add(about_btn).clicked() {
                    self.show_about_modal = true;
                }
            });

            // Baris 2 Header: Bilah Aksi Responsif (Wrap otomatis jika jendela kecil, tidak menabrak judul)
            if self.current_nav_view != NavView::Trash {
                ui.add_space(10.0);
                ui.horizontal_wrapped(|ui| {
                    ui.add_space(20.0);
                    let btn = Button::new(
                        RichText::new(lang.tr("+  Buku Baru", "+  New Notebook"))
                            .size(13.5)
                            .strong()
                            .color(Color32::WHITE),
                    )
                    .fill(Color32::from_rgb(0, 114, 206)) // Goodnotes Blue
                    .corner_radius(18.0)
                    .min_size(Vec2::new(124.0, 34.0));
                    if ui.add(btn).clicked() {
                        self.show_new_notebook_modal = true;
                        self.new_notebook_title = String::new();
                    }

                    // Fase 5: "New Folder" button
                    let folder_btn = Button::new(
                        RichText::new(lang.tr("📁+ Folder Baru", "📁+ New Folder"))
                            .size(13.0)
                            .color(Color32::WHITE),
                    )
                    .fill(Color32::from_rgb(55, 75, 100))
                    .corner_radius(18.0)
                    .min_size(Vec2::new(110.0, 34.0));
                    if ui.add(folder_btn).clicked() {
                        self.show_new_folder_modal = true;
                        self.new_folder_name.clear();
                    }

                    // Fase 3: "Import PDF" button
                    let import_btn = Button::new(
                        RichText::new(lang.tr("📥 Impor PDF", "📥 Import PDF"))
                            .size(13.0)
                            .color(Color32::WHITE),
                    )
                    .fill(Color32::from_rgb(45, 60, 85))
                    .corner_radius(18.0)
                    .min_size(Vec2::new(105.0, 34.0));
                    if ui.add(import_btn).clicked()
                        && let Some(path) = rfd::FileDialog::new()
                            .add_filter("PDF Document", &["pdf"])
                            .pick_file()
                    {
                        self.import_pdf_as_notebook(&path);
                    }

                    // Fase 7: "Import .rnote" button
                    let rnote_btn = Button::new(
                        RichText::new(lang.tr("📦 Impor .rnote", "📦 Import .rnote"))
                            .size(13.0)
                            .color(Color32::WHITE),
                    )
                    .fill(Color32::from_rgb(50, 70, 95))
                    .corner_radius(18.0)
                    .min_size(Vec2::new(110.0, 34.0));
                    if ui.add(rnote_btn).clicked()
                        && let Some(path) = rfd::FileDialog::new()
                            .add_filter("RustNotes Document (*.rnote)", &["rnote", "zip"])
                            .pick_file()
                    {
                        self.open_external_file(&path);
                    }
                });
            }

            ui.add_space(14.0);
            ui.separator();
            ui.add_space(14.0);

            // Fase 5: Breadcrumb navigation for Documents view
            if self.current_nav_view == NavView::Documents {
                ui.horizontal(|ui| {
                    ui.add_space(20.0);
                    let is_root = self.current_folder_id.is_none();
                    if ui.selectable_label(is_root, "📁 All Documents").clicked() {
                        self.current_folder_id = None;
                    }
                    if let Some(fid) = &self.current_folder_id {
                        let mut chain = Vec::new();
                        let mut curr = Some(fid.clone());
                        while let Some(c_id) = curr {
                            if let Some(f) = self.library.folders.iter().find(|f| f.id == c_id) {
                                chain.push((f.id.clone(), f.name.clone()));
                                curr = f.parent_id.clone();
                            } else {
                                break;
                            }
                        }
                        chain.reverse();
                        for (id, name) in chain {
                            ui.label(" / ");
                            let is_active = self.current_folder_id.as_ref() == Some(&id);
                            if ui
                                .selectable_label(is_active, format!("📁 {name}"))
                                .clicked()
                            {
                                self.current_folder_id = Some(id);
                            }
                        }
                    }
                });
                ui.add_space(8.0);
            }

            // Fase 5: Subfolders list in current folder
            let mut action_open_folder = None;
            let mut action_delete_folder = None;
            let query = self.search_query.trim().to_lowercase();

            if self.current_nav_view == NavView::Documents && query.is_empty() {
                let current_subfolders: Vec<crate::model::Folder> = self
                    .library
                    .folders
                    .iter()
                    .filter(|f| f.parent_id == self.current_folder_id)
                    .cloned()
                    .collect();

                if !current_subfolders.is_empty() {
                    ui.horizontal_wrapped(|ui| {
                        ui.add_space(20.0);
                        for folder in &current_subfolders {
                            let count = self
                                .library
                                .notebooks
                                .iter()
                                .filter(|n| {
                                    !n.trashed && n.folder_id.as_deref() == Some(&folder.id)
                                })
                                .count();
                            let (rect, resp) =
                                ui.allocate_exact_size(Vec2::new(130.0, 70.0), Sense::click());
                            let painter = ui.painter_at(rect);

                            let bg_col = if resp.hovered() {
                                Color32::from_rgb(55, 52, 53)
                            } else {
                                Color32::from_rgb(42, 40, 41)
                            };
                            painter.rect_filled(rect, 8.0, bg_col);
                            painter.rect_stroke(
                                rect,
                                8.0,
                                Stroke::new(1.0_f32, Color32::from_rgb(70, 67, 67)),
                                StrokeKind::Outside,
                            );

                            // Top color bar
                            painter.rect_filled(
                                Rect::from_min_size(rect.min, Vec2::new(rect.width(), 4.0)),
                                CornerRadius {
                                    nw: 8,
                                    ne: 8,
                                    se: 0,
                                    sw: 0,
                                },
                                folder.color.to_color32(),
                            );

                            painter.text(
                                rect.left_top() + Vec2::new(10.0, 14.0),
                                egui::Align2::LEFT_TOP,
                                "📁",
                                FontId::proportional(22.0),
                                Color32::WHITE,
                            );
                            painter.text(
                                rect.left_top() + Vec2::new(42.0, 14.0),
                                egui::Align2::LEFT_TOP,
                                &folder.name,
                                FontId::proportional(14.0),
                                Color32::WHITE,
                            );
                            painter.text(
                                rect.left_top() + Vec2::new(42.0, 38.0),
                                egui::Align2::LEFT_TOP,
                                format!("{count} items"),
                                FontId::proportional(11.0),
                                Color32::GRAY,
                            );

                            if resp.clicked() {
                                action_open_folder = Some(folder.id.clone());
                            }
                            resp.context_menu(|ui| {
                                if ui.button("🗑 Hapus Folder").clicked() {
                                    action_delete_folder = Some(folder.id.clone());
                                    ui.close_menu();
                                }
                            });
                        }
                    });
                    ui.add_space(14.0);
                }
            }

            if let Some(fid) = action_open_folder {
                self.current_folder_id = Some(fid);
            }
            if let Some(del_id) = action_delete_folder {
                let parent = self
                    .library
                    .folders
                    .iter()
                    .find(|f| f.id == del_id)
                    .and_then(|f| f.parent_id.clone());
                let mut moved_nb_ids = Vec::new();
                for nb in &mut self.library.notebooks {
                    if nb.folder_id.as_deref() == Some(&del_id) {
                        nb.folder_id = parent.clone();
                        moved_nb_ids.push(nb.id.clone());
                    }
                }
                for id in moved_nb_ids {
                    self.mark_dirty(&id);
                }
                for f in &mut self.library.folders {
                    if f.parent_id.as_deref() == Some(&del_id) {
                        f.parent_id = parent.clone();
                    }
                }
                self.library.folders.retain(|f| f.id != del_id);
                if self.current_folder_id.as_deref() == Some(&del_id) {
                    self.current_folder_id = parent;
                }
                self.persist_library();
            }

            // Fase 5: Smart In-Memory Search across titles, text objects, and sticky notes
            let mut notebooks_to_show: Vec<(usize, Notebook, Option<String>)> = Vec::new();
            for (i, n) in self.library.notebooks.iter().enumerate() {
                let matches_view = match self.current_nav_view {
                    NavView::Documents => {
                        !n.trashed
                            && (query.is_empty() && n.folder_id == self.current_folder_id
                                || !query.is_empty())
                    }
                    NavView::Favorites => !n.trashed && n.favorite,
                    NavView::Trash => n.trashed,
                };
                if !matches_view {
                    continue;
                }
                if query.is_empty() || n.title.to_lowercase().contains(&query) {
                    notebooks_to_show.push((i, n.clone(), None));
                } else {
                    let mut found_snippet = None;
                    for (p_idx, page) in n.pages.iter().enumerate() {
                        // Fase 8.6: Cek kecocokan pada indeks OCR tulisan tangan halaman
                        if !page.ocr_text.is_empty()
                            && page.ocr_text.to_lowercase().contains(&query)
                        {
                            let snip: String = page.ocr_text.chars().take(28).collect();
                            found_snippet = Some(format!("✍ OCR Hal. {}: \"{snip}\"", p_idx + 1));
                            break;
                        }
                        for obj in &page.objects {
                            match obj {
                                crate::model::DrawObject::Text { text, .. }
                                | crate::model::DrawObject::StickyNote { text, .. }
                                    if text.to_lowercase().contains(&query) =>
                                {
                                    let snip: String = text.chars().take(28).collect();
                                    found_snippet = Some(format!("Hal. {}: \"{snip}\"", p_idx + 1));
                                    break;
                                }
                                _ => {}
                            }
                        }
                        if found_snippet.is_some() {
                            break;
                        }
                    }
                    if let Some(snippet) = found_snippet {
                        notebooks_to_show.push((i, n.clone(), Some(snippet)));
                    }
                }
            }

            if notebooks_to_show.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space(90.0);
                    ui.label(RichText::new("📖").size(56.0));
                    ui.add_space(10.0);
                    ui.heading(
                        RichText::new(lang.tr("Belum ada buku catatan", "No notebooks here yet"))
                            .size(22.0)
                            .color(Color32::WHITE),
                    );
                    ui.label(
                        RichText::new(lang.tr(
                            "Buat buku catatan baru untuk mulai menulis dan menggambar.",
                            "Create a notebook to begin sketching and writing.",
                        ))
                        .size(14.0)
                        .color(Color32::GRAY),
                    );
                    if self.current_nav_view != NavView::Trash {
                        ui.add_space(16.0);
                        let btn = Button::new(
                            RichText::new(lang.tr("+  Buat Buku Catatan", "+  Create Notebook"))
                                .size(14.0)
                                .strong()
                                .color(Color32::WHITE),
                        )
                        .fill(Color32::from_rgb(0, 114, 206))
                        .corner_radius(18.0)
                        .min_size(Vec2::new(150.0, 38.0));
                        if ui.add(btn).clicked() {
                            self.show_new_notebook_modal = true;
                            self.new_notebook_title = String::new();
                        }
                    }
                });
            } else {
                ScrollArea::vertical().show(ui, |ui| {
                    ui.add_space(8.0);
                    ui.horizontal_wrapped(|ui| {
                        ui.add_space(20.0);
                        let mut to_open = None;
                        let mut to_toggle_fav = None;
                        let mut to_trash = None;
                        let mut to_restore = None;
                        let mut to_duplicate = None;
                        let mut action_move_nb = None;
                        // Fase 6: Export ZIP & Import ZIP
                        let mut to_export_zip: Option<(String, String)> = None; // (nb_id, nb_title)

                        for (orig_idx, note, snippet) in &notebooks_to_show {
                            ui.vertical(|ui| {
                                ui.set_width(136.0);

                                // Realistic Goodnotes Cover Card
                                let (rect, resp) =
                                    ui.allocate_exact_size(Vec2::new(130.0, 168.0), Sense::click());
                                let painter = ui.painter_at(rect);

                                // Draw Book Cover with realistic spine, label badge, watermark lines and star!
                                draw_goodnotes_cover(
                                    &painter,
                                    rect,
                                    &note.title,
                                    note.cover_color.to_color32(),
                                    note.favorite,
                                );

                                if resp.hovered() {
                                    painter.rect_stroke(
                                        rect,
                                        CornerRadius {
                                            nw: 0,
                                            ne: 12,
                                            se: 12,
                                            sw: 0,
                                        },
                                        Stroke::new(2.0_f32, Color32::from_rgb(76, 147, 236)),
                                        StrokeKind::Outside,
                                    );
                                }

                                if resp.clicked() && !note.trashed {
                                    to_open = Some(note.id.clone());
                                }

                                resp.context_menu(|ui| {
                                    if note.trashed {
                                        if ui.button("↩ Restore Notebook").clicked() {
                                            to_restore = Some(*orig_idx);
                                            ui.close_menu();
                                        }
                                    } else {
                                        if ui.button("📖 Open").clicked() {
                                            to_open = Some(note.id.clone());
                                            ui.close_menu();
                                        }
                                        let fav_text = if note.favorite {
                                            "★ Remove Favorite"
                                        } else {
                                            "☆ Add to Favorites"
                                        };
                                        if ui.button(fav_text).clicked() {
                                            to_toggle_fav = Some(*orig_idx);
                                            ui.close_menu();
                                        }
                                        if ui.button("📄 Duplicate").clicked() {
                                            to_duplicate = Some(*orig_idx);
                                            ui.close_menu();
                                        }
                                        ui.separator();
                                        // Fase 5: Move to Folder
                                        if ui.button("📁 Move to Folder...").clicked() {
                                            action_move_nb = Some(note.id.clone());
                                            ui.close_menu();
                                        }
                                        ui.separator();
                                        // Fase 6 & 7: Export as .rnote / ZIP
                                        if ui.button("📦 Export as .rnote...").clicked() {
                                            to_export_zip =
                                                Some((note.id.clone(), note.title.clone()));
                                            ui.close_menu();
                                        }
                                        ui.separator();
                                        if ui.button("🗑 Move to Trash").clicked() {
                                            to_trash = Some(*orig_idx);
                                            ui.close_menu();
                                        }
                                    }
                                });

                                ui.add_space(8.0);
                                ui.label(
                                    RichText::new(&note.title)
                                        .strong()
                                        .size(14.0)
                                        .color(Color32::WHITE),
                                );
                                let page_str = if note.pages.len() == 1 {
                                    lang.tr("halaman", "page")
                                } else {
                                    lang.tr("halaman", "pages")
                                };
                                ui.label(
                                    RichText::new(format!("{} {}", note.pages.len(), page_str))
                                        .size(11.0)
                                        .color(Color32::from_rgb(140, 136, 135)),
                                );
                                if let Some(snip) = snippet {
                                    ui.label(
                                        RichText::new(format!("🔍 {snip}"))
                                            .size(10.0)
                                            .color(Color32::from_rgb(76, 147, 236)),
                                    );
                                }
                                ui.add_space(20.0);
                            });
                            ui.add_space(14.0);
                        }

                        if let Some(id) = action_move_nb {
                            self.moving_notebook_id = Some(id);
                        }

                        if let Some(id) = to_open {
                            if !self.open_notebook_ids.contains(&id) {
                                self.open_notebook_ids.push(id.clone());
                            }
                            self.active_notebook_id = Some(id);
                            self.current_page_index = 0;
                        }
                        if let Some(idx) = to_toggle_fav {
                            self.library.notebooks[idx].favorite =
                                !self.library.notebooks[idx].favorite;
                            self.persist_library();
                        }
                        if let Some(idx) = to_duplicate {
                            let mut clone = self.library.notebooks[idx].clone();
                            clone.id = uuid::Uuid::new_v4().to_string();
                            clone.title = format!("{} copy", clone.title);
                            self.library.notebooks.push(clone);
                            self.persist_library();
                        }
                        if let Some(idx) = to_trash {
                            self.library.notebooks[idx].trashed = true;
                            self.persist_library();
                        }
                        if let Some(idx) = to_restore {
                            self.library.notebooks[idx].trashed = false;
                            self.persist_library();
                        }
                        // Fase 6: Handle Export ZIP
                        if let Some((nb_id, nb_title)) = to_export_zip {
                            let storage_dir = app_storage_dir();
                            // Force-save dulu kalau ada perubahan belum tersimpan
                            if self.dirty_notebooks.contains(&nb_id)
                                && let Some(nb) =
                                    self.library.notebooks.iter().find(|n| n.id == nb_id)
                            {
                                let _ = save_notebook_to_dir(&storage_dir, nb);
                                self.dirty_notebooks.remove(&nb_id);
                            }
                            let safe_title = nb_title
                                .replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");
                            let default_name = format!("{safe_title}.rnote");
                            if let Some(path) = rfd::FileDialog::new()
                                .set_file_name(&default_name)
                                .add_filter("RustNotes Document (*.rnote)", &["rnote"])
                                .add_filter("ZIP Archive (*.zip)", &["zip"])
                                .save_file()
                            {
                                match crate::storage::export_notebook_to_rnote(
                                    &storage_dir,
                                    &nb_id,
                                    &path,
                                ) {
                                    Ok(()) => {
                                        self.toast_message = Some(format!(
                                            "✅ Notebook '{nb_title}' berhasil diekspor ke .rnote."
                                        ));
                                    }
                                    Err(e) => {
                                        self.toast_message =
                                            Some(format!("⚠ Gagal ekspor .rnote: {e}"));
                                    }
                                }
                            }
                        }
                    });
                });
            }
        });

        // Goodnotes 2-Column New Notebook Modal Dialog
        if self.show_new_notebook_modal {
            egui::Window::new("Create Notebook")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.set_width(580.0);
                    ui.set_height(420.0);
                    ui.add_space(8.0);

                    ui.columns(2, |cols| {
                        // Left Column: Cover & Title
                        cols[0].vertical(|ui| {
                            ui.label(RichText::new("Title").strong());
                            ui.text_edit_singleline(&mut self.new_notebook_title);
                            ui.add_space(14.0);

                            ui.label(RichText::new("Cover Preview").strong());
                            let (rect, _) =
                                ui.allocate_exact_size(Vec2::new(120.0, 155.0), Sense::hover());
                            let title_preview = if self.new_notebook_title.trim().is_empty() {
                                "Untitled Notebook"
                            } else {
                                self.new_notebook_title.trim()
                            };
                            draw_goodnotes_cover(
                                &ui.painter_at(rect),
                                rect,
                                title_preview,
                                self.new_notebook_cover,
                                false,
                            );

                            ui.add_space(12.0);
                            ui.label(RichText::new("Cover Color").strong());
                            ui.horizontal_wrapped(|ui| {
                                for c in COVER_PALETTE {
                                    let (rect, resp) = ui
                                        .allocate_exact_size(Vec2::new(22.0, 22.0), Sense::click());
                                    let stroke = if self.new_notebook_cover == c {
                                        Stroke::new(2.5_f32, Color32::WHITE)
                                    } else {
                                        Stroke::NONE
                                    };
                                    ui.painter_at(rect).rect(
                                        rect,
                                        4.0,
                                        c,
                                        stroke,
                                        StrokeKind::Outside,
                                    );
                                    if resp.clicked() {
                                        self.new_notebook_cover = c;
                                    }
                                }
                            });
                            ui.add_space(10.0);
                            ui.label(RichText::new("Paper Size").strong());
                            ui.horizontal_wrapped(|ui| {
                                for preset in PaperSizePreset::ALL {
                                    let is_sel = !self.new_notebook_use_custom_size
                                        && self.new_notebook_preset == preset;
                                    let name = match preset {
                                        PaperSizePreset::A4 => "A4",
                                        PaperSizePreset::Letter => "Letter",
                                        PaperSizePreset::A5 => "A5",
                                        PaperSizePreset::Legacy => "Legacy",
                                    };
                                    if ui.selectable_label(is_sel, name).clicked() {
                                        self.new_notebook_use_custom_size = false;
                                        self.new_notebook_preset = preset;
                                        let (w, h) =
                                            preset.dimensions(self.new_notebook_orientation);
                                        self.new_notebook_custom_width = w;
                                        self.new_notebook_custom_height = h;
                                    }
                                }
                                let is_slide = self.new_notebook_use_custom_size
                                    && (self.new_notebook_custom_width - 1280.0).abs() < 1.0
                                    && (self.new_notebook_custom_height - 720.0).abs() < 1.0;
                                if ui.selectable_label(is_slide, "16:9 Slide").clicked() {
                                    self.new_notebook_use_custom_size = true;
                                    self.new_notebook_custom_width = 1280.0;
                                    self.new_notebook_custom_height = 720.0;
                                }
                                if ui
                                    .selectable_label(
                                        self.new_notebook_use_custom_size && !is_slide,
                                        "Custom Size",
                                    )
                                    .clicked()
                                {
                                    self.new_notebook_use_custom_size = true;
                                }
                            });
                            ui.horizontal(|ui| {
                                if ui
                                    .selectable_label(
                                        !self.new_notebook_use_custom_size
                                            && self.new_notebook_orientation
                                                == PageOrientation::Portrait,
                                        "Portrait",
                                    )
                                    .clicked()
                                {
                                    self.new_notebook_use_custom_size = false;
                                    self.new_notebook_orientation = PageOrientation::Portrait;
                                    let (w, h) = self
                                        .new_notebook_preset
                                        .dimensions(self.new_notebook_orientation);
                                    self.new_notebook_custom_width = w;
                                    self.new_notebook_custom_height = h;
                                }
                                if ui
                                    .selectable_label(
                                        !self.new_notebook_use_custom_size
                                            && self.new_notebook_orientation
                                                == PageOrientation::Landscape,
                                        "Landscape",
                                    )
                                    .clicked()
                                {
                                    self.new_notebook_use_custom_size = false;
                                    self.new_notebook_orientation = PageOrientation::Landscape;
                                    let (w, h) = self
                                        .new_notebook_preset
                                        .dimensions(self.new_notebook_orientation);
                                    self.new_notebook_custom_width = w;
                                    self.new_notebook_custom_height = h;
                                }
                                if ui
                                    .small_button("⇄ Swap W×H")
                                    .on_hover_text("Tukar Lebar dan Tinggi kertas")
                                    .clicked()
                                {
                                    self.new_notebook_use_custom_size = true;
                                    std::mem::swap(
                                        &mut self.new_notebook_custom_width,
                                        &mut self.new_notebook_custom_height,
                                    );
                                }
                            });
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("W:").size(11.5).color(Color32::LIGHT_GRAY));
                                if ui
                                    .add(
                                        egui::DragValue::new(&mut self.new_notebook_custom_width)
                                            .range(200.0..=4000.0)
                                            .speed(2.0)
                                            .suffix(" pt"),
                                    )
                                    .changed()
                                {
                                    self.new_notebook_use_custom_size = true;
                                }
                                ui.label(
                                    RichText::new("× H:").size(11.5).color(Color32::LIGHT_GRAY),
                                );
                                if ui
                                    .add(
                                        egui::DragValue::new(&mut self.new_notebook_custom_height)
                                            .range(200.0..=4000.0)
                                            .speed(2.0)
                                            .suffix(" pt"),
                                    )
                                    .changed()
                                {
                                    self.new_notebook_use_custom_size = true;
                                }
                            });
                            let (pw, ph) = if self.new_notebook_use_custom_size {
                                (
                                    self.new_notebook_custom_width.clamp(200.0, 4000.0),
                                    self.new_notebook_custom_height.clamp(200.0, 4000.0),
                                )
                            } else {
                                self.new_notebook_preset
                                    .dimensions(self.new_notebook_orientation)
                            };
                            ui.label(
                                RichText::new(format!("Ukuran Kertas: {:.0} × {:.0} pt", pw, ph))
                                    .size(11.0)
                                    .color(Color32::GRAY),
                            );
                        });

                        // Right Column: Paper Templates
                        cols[1].vertical(|ui| {
                            ui.label(RichText::new("Paper Template").strong());
                            ui.add_space(6.0);

                            let templates = [
                                (
                                    PageTemplate::Ruled,
                                    "Ruled Narrow",
                                    "Horizontal writing lines",
                                ),
                                (
                                    PageTemplate::Grid,
                                    "Squared (Grid)",
                                    "Math & structure grid",
                                ),
                                (
                                    PageTemplate::Dotted,
                                    "Dotted Paper",
                                    "Bullet journaling dots",
                                ),
                                (
                                    PageTemplate::Blank,
                                    "Blank Paper",
                                    "Plain unlined sketch paper",
                                ),
                                (
                                    PageTemplate::Cornell,
                                    "Cornell Notes",
                                    "Summary & cue column template",
                                ),
                            ];

                            for (tmpl, label, desc) in templates {
                                let is_selected = self.new_notebook_template == tmpl;
                                let (rect, resp) =
                                    ui.allocate_exact_size(Vec2::new(270.0, 48.0), Sense::click());
                                let bg = if is_selected {
                                    Color32::from_rgb(39, 54, 91)
                                } else if resp.hovered() {
                                    Color32::from_rgb(52, 50, 51)
                                } else {
                                    Color32::from_rgb(38, 36, 37)
                                };
                                let border_stroke = if is_selected {
                                    Stroke::new(1.5_f32, Color32::from_rgb(76, 147, 236))
                                } else {
                                    Stroke::new(1.0_f32, Color32::from_rgb(60, 57, 57))
                                };
                                ui.painter().rect(
                                    rect,
                                    6.0,
                                    bg,
                                    border_stroke,
                                    StrokeKind::Outside,
                                );
                                ui.painter().text(
                                    rect.left_top() + Vec2::new(12.0, 10.0),
                                    egui::Align2::LEFT_TOP,
                                    label,
                                    FontId::proportional(14.0),
                                    if is_selected {
                                        Color32::from_rgb(76, 147, 236)
                                    } else {
                                        Color32::WHITE
                                    },
                                );
                                ui.painter().text(
                                    rect.left_top() + Vec2::new(12.0, 28.0),
                                    egui::Align2::LEFT_TOP,
                                    desc,
                                    FontId::proportional(11.0),
                                    Color32::GRAY,
                                );
                                if resp.clicked() {
                                    self.new_notebook_template = tmpl;
                                }
                            }

                            // Fase 5: Custom Templates section if any exist
                            if !self.custom_templates.is_empty() {
                                ui.add_space(8.0);
                                ui.label(
                                    RichText::new("⭐ Template Kustom:")
                                        .strong()
                                        .size(12.0)
                                        .color(Color32::from_rgb(255, 204, 0)),
                                );
                                for ctmpl in &self.custom_templates {
                                    let is_cur = self.new_notebook_template == ctmpl.template;
                                    let (rect, resp) = ui.allocate_exact_size(
                                        Vec2::new(270.0, 36.0),
                                        Sense::click(),
                                    );
                                    let bg = if is_cur {
                                        Color32::from_rgb(39, 54, 91)
                                    } else if resp.hovered() {
                                        Color32::from_rgb(52, 50, 51)
                                    } else {
                                        Color32::from_rgb(38, 36, 37)
                                    };
                                    ui.painter().rect(
                                        rect,
                                        6.0,
                                        bg,
                                        Stroke::new(1.0_f32, Color32::from_rgb(60, 57, 57)),
                                        StrokeKind::Outside,
                                    );
                                    ui.painter().text(
                                        rect.left_top() + Vec2::new(10.0, 10.0),
                                        egui::Align2::LEFT_TOP,
                                        format!("📄 {}", ctmpl.name),
                                        FontId::proportional(12.0),
                                        if is_cur {
                                            Color32::from_rgb(76, 147, 236)
                                        } else {
                                            Color32::WHITE
                                        },
                                    );
                                    if resp.clicked() {
                                        self.new_notebook_template = ctmpl.template;
                                    }
                                }
                            }
                        });
                    });

                    ui.add_space(20.0);
                    ui.separator();
                    ui.add_space(10.0);

                    // Modal Footer Buttons
                    ui.horizontal(|ui| {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let create_btn = Button::new(
                                RichText::new("Create Notebook")
                                    .strong()
                                    .color(Color32::WHITE),
                            )
                            .fill(Color32::from_rgb(0, 114, 206))
                            .corner_radius(16.0)
                            .min_size(Vec2::new(140.0, 34.0));
                            if ui.add(create_btn).clicked() {
                                let title = if self.new_notebook_title.trim().is_empty() {
                                    "Untitled Notebook"
                                } else {
                                    self.new_notebook_title.trim()
                                };
                                let mut nb = if self.new_notebook_use_custom_size {
                                    Notebook::new_with_size(
                                        title,
                                        self.new_notebook_template,
                                        self.new_notebook_cover,
                                        self.new_notebook_custom_width.clamp(200.0, 4000.0),
                                        self.new_notebook_custom_height.clamp(200.0, 4000.0),
                                    )
                                } else {
                                    Notebook::new_with_preset(
                                        title,
                                        self.new_notebook_template,
                                        self.new_notebook_cover,
                                        self.new_notebook_preset,
                                        self.new_notebook_orientation,
                                    )
                                };
                                nb.folder_id = self.current_folder_id.clone();
                                let new_id = nb.id.clone();
                                self.library.notebooks.push(nb);
                                // Fase 6: Simpan notebook baru langsung (force-save)
                                self.force_save_notebook(&new_id);
                                self.show_new_notebook_modal = false;

                                if !self.open_notebook_ids.contains(&new_id) {
                                    self.open_notebook_ids.push(new_id.clone());
                                }
                                self.active_notebook_id = Some(new_id);
                                self.current_page_index = 0;
                            }

                            if ui.button("Cancel").clicked() {
                                self.show_new_notebook_modal = false;
                            }
                        });
                    });
                });
        }

        // Fase 5: Create New Folder Modal Dialog
        if self.show_new_folder_modal {
            egui::Window::new("📁 New Folder")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.set_width(320.0);
                    ui.label(RichText::new("Folder Name").strong());
                    ui.text_edit_singleline(&mut self.new_folder_name);
                    ui.add_space(8.0);
                    ui.label(RichText::new("Folder Color").strong());
                    ui.horizontal_wrapped(|ui| {
                        for c in COVER_PALETTE {
                            let (rect, resp) =
                                ui.allocate_exact_size(Vec2::new(22.0, 22.0), Sense::click());
                            let stroke = if self.new_folder_color == c {
                                Stroke::new(2.5_f32, Color32::WHITE)
                            } else {
                                Stroke::NONE
                            };
                            ui.painter_at(rect)
                                .rect(rect, 4.0, c, stroke, StrokeKind::Outside);
                            if resp.clicked() {
                                self.new_folder_color = c;
                            }
                        }
                    });
                    ui.add_space(14.0);
                    ui.horizontal(|ui| {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.button("Create").clicked() {
                                let name = if self.new_folder_name.trim().is_empty() {
                                    "New Folder".to_string()
                                } else {
                                    self.new_folder_name.trim().to_string()
                                };
                                let mut f = crate::model::Folder::new(
                                    &name,
                                    self.current_folder_id.clone(),
                                );
                                f.color = self.new_folder_color.into();
                                self.library.folders.push(f);
                                self.persist_library();
                                self.show_new_folder_modal = false;
                                self.new_folder_name.clear();
                            }
                            if ui.button("Cancel").clicked() {
                                self.show_new_folder_modal = false;
                            }
                        });
                    });
                });
        }

        // Fase 5: Move Notebook to Folder Modal Dialog
        if let Some(nb_id) = self.moving_notebook_id.clone() {
            egui::Window::new("📁 Move Notebook to Folder")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.set_width(320.0);
                    ui.label(RichText::new("Select destination folder:").strong());
                    ui.add_space(8.0);
                    ScrollArea::vertical().max_height(240.0).show(ui, |ui| {
                        if ui.button("📁 Root (All Documents)").clicked() {
                            if let Some(nb) =
                                self.library.notebooks.iter_mut().find(|n| n.id == nb_id)
                            {
                                nb.folder_id = None;
                                self.mark_dirty(&nb_id);
                                self.sync_library_index();
                            }
                            self.moving_notebook_id = None;
                        }
                        ui.separator();
                        let folders = self.library.folders.clone();
                        for folder in folders {
                            if ui.button(format!("📁 {}", folder.name)).clicked() {
                                if let Some(nb) =
                                    self.library.notebooks.iter_mut().find(|n| n.id == nb_id)
                                {
                                    nb.folder_id = Some(folder.id);
                                    self.mark_dirty(&nb_id);
                                    self.sync_library_index();
                                }
                                self.moving_notebook_id = None;
                            }
                        }
                    });
                    ui.add_space(8.0);
                    if ui.button("Cancel").clicked() {
                        self.moving_notebook_id = None;
                    }
                });
        }
    }

    fn render_editor_view(&mut self, ctx: &egui::Context) {
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

            // Fase 5: Ctrl+F: In-document search
            if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::F)) {
                self.show_in_doc_search = !self.show_in_doc_search;
                self.doc_search_current_match = 0;
            }

            // Ctrl+S: Save notebook immediately
            if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::S)) {
                let nb_id = notebook_id.clone();
                self.force_save_notebook(&nb_id);
                self.toast_message = Some("💾 Dokumen berhasil disimpan.".to_string());
            }

            // Ctrl+0: Reset zoom to 100% & center pan
            if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Num0)) {
                self.apply_zoom_anchored(nb_idx, 1.0, None);
                self.canvas_pan_offset = Vec2::ZERO;
                self.toast_message = Some("🔍 Zoom di-reset ke 100%.".to_string());
            }

            // Ctrl+A: Select all objects on current page
            if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::A)) {
                let p = &self.library.notebooks[nb_idx].pages[self.current_page_index];
                self.canvas_state.selected_ids =
                    p.objects.iter().map(|o| o.id().to_string()).collect();
                self.canvas_state.current_tool = ActiveTool::Lasso;
            }

            // Shortcuts: Ctrl+C (Copy), Ctrl+X (Cut), Ctrl+D (Duplicate), Ctrl+V (Paste)
            let is_copy = ctx.input_mut(|i| {
                i.consume_key(egui::Modifiers::COMMAND, egui::Key::C)
                    || i.events.iter().any(|e| matches!(e, egui::Event::Copy))
                    || (i.modifiers.command && i.key_pressed(egui::Key::C))
            }) || ctx.input(|i| {
                i.raw.events.iter().any(|e| match e {
                    egui::Event::Copy => true,
                    egui::Event::Key {
                        key: egui::Key::C,
                        pressed: true,
                        modifiers,
                        ..
                    } => modifiers.command,
                    _ => false,
                })
            });

            let is_cut = ctx.input_mut(|i| {
                i.consume_key(egui::Modifiers::COMMAND, egui::Key::X)
                    || i.events.iter().any(|e| matches!(e, egui::Event::Cut))
                    || (i.modifiers.command && i.key_pressed(egui::Key::X))
            }) || ctx.input(|i| {
                i.raw.events.iter().any(|e| match e {
                    egui::Event::Cut => true,
                    egui::Event::Key {
                        key: egui::Key::X,
                        pressed: true,
                        modifiers,
                        ..
                    } => modifiers.command,
                    _ => false,
                })
            });

            let is_duplicate = ctx.input_mut(|i| {
                i.consume_key(egui::Modifiers::COMMAND, egui::Key::D)
                    || (i.modifiers.command && i.key_pressed(egui::Key::D))
            }) || ctx.input(|i| {
                i.raw.events.iter().any(|e| match e {
                    egui::Event::Key {
                        key: egui::Key::D,
                        pressed: true,
                        modifiers,
                        ..
                    } => modifiers.command,
                    _ => false,
                })
            });

            let is_paste = ctx.input_mut(|i| {
                i.consume_key(egui::Modifiers::COMMAND, egui::Key::V)
                    || i.events.iter().any(|e| matches!(e, egui::Event::Paste(_)))
                    || (i.modifiers.command && i.key_pressed(egui::Key::V))
            }) || ctx.input(|i| {
                i.raw.events.iter().any(|e| match e {
                    egui::Event::Paste(_) => true,
                    egui::Event::Key {
                        key: egui::Key::V,
                        pressed: true,
                        modifiers,
                        ..
                    } => modifiers.command,
                    _ => false,
                })
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
            if is_cut {
                let mut cut_any = false;
                self.push_doc_undo(nb_idx);
                let mut copied = Vec::new();
                for page in &mut self.library.notebooks[nb_idx].pages {
                    for obj in &page.objects {
                        if self
                            .canvas_state
                            .selected_ids
                            .contains(&obj.id().to_string())
                        {
                            copied.push(obj.clone());
                        }
                    }
                    if !self.canvas_state.selected_ids.is_empty() {
                        let prev_len = page.objects.len();
                        page.objects.retain(|o| {
                            !self.canvas_state.selected_ids.contains(&o.id().to_string())
                        });
                        if page.objects.len() != prev_len {
                            cut_any = true;
                        }
                    }
                }
                if cut_any {
                    self.canvas_state.object_clipboard = copied;
                    self.canvas_state.app_clipboard_timestamp = Some(Instant::now());
                    self.canvas_state.selected_ids.clear();
                    self.mark_dirty(&notebook_id);
                    self.toast_message = Some("✂ Objek dipotong.".to_string());
                }
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
                    && (i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace))
            }) {
                let p = &mut self.library.notebooks[nb_idx].pages[self.current_page_index];
                self.canvas_state.delete_selected(p);
                self.mark_dirty(&notebook_id);
                self.toast_message = Some("🗑 Objek dihapus.".to_string());
            }

            // Ctrl+Z: Undo
            if ctx
                .input(|i| i.modifiers.command && !i.modifiers.shift && i.key_pressed(egui::Key::Z))
            {
                self.perform_undo(nb_idx);
            }

            // Ctrl+Y or Ctrl+Shift+Z: Redo
            if ctx.input(|i| {
                (i.modifiers.command && i.key_pressed(egui::Key::Y))
                    || (i.modifiers.command && i.modifiers.shift && i.key_pressed(egui::Key::Z))
            }) {
                self.perform_redo(nb_idx);
            }

            // Single key tool shortcuts (only without modifiers!)
            let no_mods = ctx.input(|i| {
                !i.modifiers.command && !i.modifiers.ctrl && !i.modifiers.alt && !i.modifiers.shift
            });
            if no_mods {
                if ctx.input(|i| i.key_pressed(egui::Key::P)) {
                    self.canvas_state.current_tool = ActiveTool::Pen;
                } else if ctx.input(|i| i.key_pressed(egui::Key::H)) {
                    self.canvas_state.current_tool = ActiveTool::Highlighter;
                } else if ctx.input(|i| i.key_pressed(egui::Key::E)) {
                    self.canvas_state.current_tool = ActiveTool::Eraser;
                } else if ctx.input(|i| i.key_pressed(egui::Key::L)) {
                    self.canvas_state.current_tool = ActiveTool::Lasso;
                } else if ctx.input(|i| i.key_pressed(egui::Key::T)) {
                    self.canvas_state.current_tool = ActiveTool::Text;
                } else if ctx.input(|i| i.key_pressed(egui::Key::S)) {
                    self.canvas_state.current_tool = ActiveTool::Shape(ShapeKind::Rectangle);
                } else if ctx.input(|i| i.key_pressed(egui::Key::N)) {
                    self.canvas_state.current_tool = ActiveTool::StickyNote;
                }
            }

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
                i.key_pressed(egui::Key::F6) || (i.modifiers.command && i.key_pressed(egui::Key::B))
            }) {
                self.show_thumbnail_sidebar = !self.show_thumbnail_sidebar;
            }
            if ctx.input(|i| i.modifiers.alt && i.key_pressed(egui::Key::Z)) {
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

        let lang = self.settings.language;

        // Authentic GoodNotes Single Navy Top Navbar (#284872) + Pop-ups for Specific & Advanced Features
        let mut return_to_library = false;
        egui::TopBottomPanel::top("editor_navbar")
            .min_height(44.0)
            .show(ctx, |ui| {
                ui.painter().rect_filled(
                    ui.max_rect(),
                    0.0,
                    Color32::from_rgb(40, 72, 114), // #284872 GoodNotes Navy
                );

                ui.add_space(5.0);
                let total_w = ui.available_width();
                let is_wide = total_w >= 1020.0;

                // Helper closures/blocks via macro-free scoped methods on ui:
                // Baris Utama: Kiri (Navigasi & Judul), Tengah (Tools Center pada layar lebar), Paling Kanan (Settings & Page Actions)
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    ui.add_space(6.0);

                    // --- 1. LEFT GROUP: Back, Document Title ▾, Sidebar, Search, Zoom Win, Bookmark ---
                    if ui
                        .add(
                            Button::new(RichText::new("◀").size(14.0).color(Color32::WHITE))
                                .fill(Color32::from_rgb(54, 95, 145))
                                .corner_radius(6.0)
                                .min_size(Vec2::new(28.0, 28.0)),
                        )
                        .on_hover_text(lang.tr("Kembali ke Perpustakaan", "Back to Library"))
                        .clicked()
                    {
                        return_to_library = true;
                    }

                    // Document Title with Dropdown (`[Title] ▾`) like GoodNotes
                    let raw_title = &self.library.notebooks[nb_idx].title;
                    let max_chars = if total_w < 1180.0 { 14 } else { 22 };
                    let display_title = if raw_title.chars().count() > max_chars {
                        format!(
                            "{}… ▾",
                            raw_title.chars().take(max_chars - 1).collect::<String>()
                        )
                    } else {
                        format!("{raw_title} ▾")
                    };
                    let title_btn = Button::new(
                        RichText::new(display_title)
                            .size(13.5)
                            .strong()
                            .color(Color32::WHITE),
                    )
                    .fill(if self.show_doc_title_popup {
                        Color32::from_rgb(54, 95, 145)
                    } else {
                        Color32::TRANSPARENT
                    })
                    .corner_radius(6.0);
                    if ui
                        .add(title_btn)
                        .on_hover_text(lang.tr(
                            "Klik untuk ubah judul & info dokumen",
                            "Click to rename & view document info",
                        ))
                        .clicked()
                    {
                        self.show_doc_title_popup = !self.show_doc_title_popup;
                        self.show_tool_popup = false;
                        self.show_more_menu_popup = false;
                    }

                    // Page Thumbnail Sidebar Icon (`◫`)
                    let sb_btn = Button::new(
                        RichText::new("◫")
                            .size(14.0)
                            .color(if self.show_thumbnail_sidebar {
                                Color32::from_rgb(130, 195, 255)
                            } else {
                                Color32::WHITE
                            }),
                    )
                    .fill(if self.show_thumbnail_sidebar {
                        Color32::from_rgb(54, 95, 145)
                    } else {
                        Color32::TRANSPARENT
                    })
                    .corner_radius(6.0)
                    .min_size(Vec2::new(28.0, 28.0));
                    if ui
                        .add(sb_btn)
                        .on_hover_text(lang.tr(
                            "Panel Halaman, Bookmark & Outline (Ctrl+B)",
                            "Pages, Bookmarks & Outline Sidebar (Ctrl+B)",
                        ))
                        .clicked()
                    {
                        self.show_thumbnail_sidebar = !self.show_thumbnail_sidebar;
                    }

                    // Search Icon (`🔍`)
                    let find_btn =
                        Button::new(RichText::new("🔍").size(13.0).color(Color32::WHITE))
                            .fill(if self.show_in_doc_search {
                                Color32::from_rgb(54, 95, 145)
                            } else {
                                Color32::TRANSPARENT
                            })
                            .corner_radius(6.0)
                            .min_size(Vec2::new(28.0, 28.0));
                    if ui
                        .add(find_btn)
                        .on_hover_text(
                            lang.tr("Cari di Dokumen (Ctrl+F)", "Find in Document (Ctrl+F)"),
                        )
                        .clicked()
                    {
                        self.show_in_doc_search = !self.show_in_doc_search;
                        self.doc_search_current_match = 0;
                    }

                    // Zoom Window Icon (`🔍+`)
                    let zw_btn = Button::new(RichText::new("🔍+").size(12.0).color(Color32::WHITE))
                        .fill(if self.canvas_state.show_zoom_window {
                            Color32::from_rgb(0, 114, 206)
                        } else {
                            Color32::TRANSPARENT
                        })
                        .corner_radius(6.0)
                        .min_size(Vec2::new(30.0, 28.0));
                    if ui
                        .add(zw_btn)
                        .on_hover_text(lang.tr(
                            "Jendela Pembesar Tulis / Zoom Window (Alt+Z)",
                            "Zoom Window for Detailed Writing (Alt+Z)",
                        ))
                        .clicked()
                    {
                        self.canvas_state.show_zoom_window = !self.canvas_state.show_zoom_window;
                    }

                    // Bookmark Icon (`★`/`☆`)
                    let is_bm =
                        self.library.notebooks[nb_idx].pages[self.current_page_index].bookmarked;
                    let bm_icon = if is_bm { "★" } else { "☆" };
                    let bm_col = if is_bm {
                        Color32::from_rgb(255, 204, 0)
                    } else {
                        Color32::WHITE
                    };
                    if ui
                        .add(
                            Button::new(RichText::new(bm_icon).size(15.0).color(bm_col))
                                .fill(Color32::TRANSPARENT)
                                .min_size(Vec2::new(26.0, 28.0)),
                        )
                        .on_hover_text(
                            lang.tr("Tandai Halaman Ini (Bookmark)", "Bookmark Current Page"),
                        )
                        .clicked()
                    {
                        self.library.notebooks[nb_idx].pages[self.current_page_index].bookmarked =
                            !is_bm;
                        self.mark_dirty(&notebook_id);
                    }

                    let left_end_x = ui.cursor().min.x;

                    // --- 3. FAR RIGHT GROUP: Settings (`⋯`), Export (`📤`), Add Page (`📄+`), Paper Size (`📐`), Scroll Mode (`📜`) ---
                    let mut right_start_x = ui.max_rect().right();
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.add_space(8.0);
                        ui.spacing_mut().item_spacing.x = 6.0;

                        // GoodNotes Circular More Menu / Settings Button (`⋯`) — Paling Kanan
                        let more_btn = Button::new(
                            RichText::new("⋯")
                                .size(16.0)
                                .strong()
                                .color(if self.show_more_menu_popup {
                                    Color32::from_rgb(30, 58, 95)
                                } else {
                                    Color32::WHITE
                                }),
                        )
                        .fill(if self.show_more_menu_popup {
                            Color32::from_rgb(214, 232, 255)
                        } else {
                            Color32::from_rgb(54, 95, 145)
                        })
                        .corner_radius(14.0)
                        .min_size(Vec2::new(30.0, 28.0));
                        if ui
                            .add(more_btn)
                            .on_hover_text(lang.tr(
                                "Pengaturan & Fitur Lanjutan (GoodNotes More Menu)",
                                "Settings & Advanced Features Pop-up",
                            ))
                            .clicked()
                        {
                            self.show_more_menu_popup = !self.show_more_menu_popup;
                            self.show_tool_popup = false;
                            self.show_doc_title_popup = false;
                        }

                        // Export Menu (`📤`)
                        ui.menu_button(
                            RichText::new("📤").size(13.5).color(Color32::WHITE),
                            |ui| {
                                if ui.button("📄 Export Notebook to PDF").clicked() {
                                    ui.close_menu();
                                    let nb = &self.library.notebooks[nb_idx];
                                    let default_name = format!(
                                        "{}.pdf",
                                        nb.title.replace(
                                            ['/', '\\', ':', '*', '?', '"', '<', '>', '|'],
                                            "_"
                                        )
                                    );
                                    if let Some(path) = rfd::FileDialog::new()
                                        .set_file_name(&default_name)
                                        .add_filter("PDF Document", &["pdf"])
                                        .save_file()
                                    {
                                        let storage_dir = app_storage_dir();
                                        match export_notebook_to_pdf(&storage_dir, nb, &path) {
                                            Ok(()) => {
                                                self.toast_message = Some(
                                                    "✅ Dokumen PDF berhasil diekspor.".to_string(),
                                                );
                                            }
                                            Err(e) => {
                                                self.toast_message =
                                                    Some(format!("⚠ Gagal ekspor PDF: {e}"));
                                            }
                                        }
                                    }
                                }
                                if ui.button("🖼 Export Page to PNG").clicked() {
                                    ui.close_menu();
                                    let page = &self.library.notebooks[nb_idx].pages
                                        [self.current_page_index];
                                    let default_name =
                                        format!("page_{}.png", self.current_page_index + 1);
                                    if let Some(path) = rfd::FileDialog::new()
                                        .set_file_name(&default_name)
                                        .add_filter("PNG Image", &["png"])
                                        .save_file()
                                    {
                                        let storage_dir = app_storage_dir();
                                        match export_page_to_png(
                                            &storage_dir,
                                            &notebook_id,
                                            page,
                                            &path,
                                        ) {
                                            Ok(()) => {
                                                self.toast_message = Some(
                                                    "✅ Halaman berhasil diekspor ke PNG."
                                                        .to_string(),
                                                );
                                            }
                                            Err(e) => {
                                                self.toast_message =
                                                    Some(format!("⚠ Gagal ekspor PNG: {e}"));
                                            }
                                        }
                                    }
                                }
                            },
                        );

                        // Add Page (`📄+`)
                        let add_page_btn = Button::new(
                            RichText::new("📄+")
                                .size(13.0)
                                .strong()
                                .color(Color32::WHITE),
                        )
                        .fill(Color32::from_rgb(54, 95, 145))
                        .corner_radius(6.0)
                        .min_size(Vec2::new(34.0, 28.0));
                        if ui
                            .add(add_page_btn)
                            .on_hover_text(lang.tr(
                                "Atur & Tambah Halaman Baru...",
                                "Configure & Add New Page...",
                            ))
                            .clicked()
                        {
                            self.open_add_page_modal(nb_idx);
                        }

                        // Paper Size Quick Badge (`📐 595×842`) -> opens Per-Page / Custom Paper Size Modal
                        let (cur_pw, cur_ph) = {
                            let p = &self.library.notebooks[nb_idx].pages[self.current_page_index];
                            (p.width, p.height)
                        };
                        let size_btn = Button::new(
                            RichText::new(format!("📐 {:.0}×{:.0}", cur_pw, cur_ph))
                                .size(11.5)
                                .color(Color32::WHITE),
                        )
                        .fill(Color32::from_rgb(54, 95, 145))
                        .corner_radius(14.0)
                        .min_size(Vec2::new(72.0, 28.0));
                        if ui
                            .add(size_btn)
                            .on_hover_text(lang.tr(
                                "Atur Ukuran Kertas Spesifik / Per-Halaman & Template...",
                                "Configure Custom / Per-Page Paper Size & Template...",
                            ))
                            .clicked()
                        {
                            self.open_page_resize_modal(nb_idx, self.current_page_index);
                        }

                        // Scrollable vs Single Page Toggle (`📜 Scroll` vs `📄 1 Hal`)
                        let view_label = match self.view_mode {
                            ViewMode::ContinuousVertical => lang.tr("📜 Scroll", "📜 Scroll"),
                            ViewMode::SinglePage => lang.tr("📄 1 Hal", "📄 1 Page"),
                        };
                        let vm_btn = Button::new(
                            RichText::new(view_label)
                                .size(12.0)
                                .strong()
                                .color(Color32::WHITE),
                        )
                        .fill(match self.view_mode {
                            ViewMode::ContinuousVertical => Color32::from_rgb(0, 114, 206),
                            ViewMode::SinglePage => Color32::from_rgb(54, 95, 145),
                        })
                        .corner_radius(14.0)
                        .min_size(Vec2::new(68.0, 28.0));
                        if ui
                            .add(vm_btn)
                            .on_hover_text(lang.tr(
                                "Toggle Mode Halaman: Scroll Menyambung (Continuous) vs Satu Halaman Tunggal (Single Page, tidak scroll ke halaman lain)",
                                "Toggle Page Mode: Scrollable Connected Pages vs Single Isolated Page",
                            ))
                            .clicked()
                        {
                            let next_mode = match self.view_mode {
                                ViewMode::SinglePage => ViewMode::ContinuousVertical,
                                ViewMode::ContinuousVertical => ViewMode::SinglePage,
                            };
                            self.set_view_mode(nb_idx, next_mode);
                        }

                        right_start_x = ui.cursor().max.x;

                        // --- 2. CENTER GROUP (pada layar lebar): Tools ditaruh tepat di tengah (Center) navbar ---
                        if is_wide {
                            let tools_width = 456.0_f32;
                            let ideal_left = (total_w - tools_width) * 0.5;
                            let clamped_left = ideal_left
                                .max(left_end_x + 8.0)
                                .min((right_start_x - tools_width - 8.0).max(left_end_x + 8.0));
                            let center_rect = Rect::from_min_size(
                                Pos2::new(clamped_left, ui.max_rect().top()),
                                Vec2::new(tools_width, 30.0),
                            );
                            let mut center_ui = ui.new_child(
                                egui::UiBuilder::new()
                                    .max_rect(center_rect)
                                    .layout(Layout::left_to_right(Align::Center)),
                            );
                            center_ui.spacing_mut().item_spacing.x = 4.0;

                            let active_shape_kind = match self.canvas_state.current_tool {
                                ActiveTool::Shape(k) => k,
                                _ => ShapeKind::Rectangle,
                            };
                            let tools = [
                                (
                                    ActiveTool::Pen,
                                    "✒",
                                    "Pen (P) — Klik lagi untuk atur ketebalan & warna",
                                ),
                                (ActiveTool::Pencil, "✏", "Pencil — Goresan pensil"),
                                (
                                    ActiveTool::Highlighter,
                                    "🖍",
                                    "Highlighter (H) — Stabilo transparan",
                                ),
                                (
                                    ActiveTool::Eraser,
                                    "⌫",
                                    "Eraser (E) — Klik lagi untuk opsi Partial / Whole Stroke",
                                ),
                                (
                                    ActiveTool::Shape(active_shape_kind),
                                    "📐",
                                    "Shapes — Klik lagi untuk pilih bentuk",
                                ),
                                (ActiveTool::Lasso, "✂", "Lasso Selection (L)"),
                                (ActiveTool::Text, "T", "Text Box (T)"),
                                (ActiveTool::StickyNote, "🗒", "Sticky Note"),
                                (ActiveTool::Laser, "✨", "Laser Pointer"),
                                (ActiveTool::Hand, "✋", "Hand / Pan Mode (Space)"),
                            ];

                            for (tool_var, icon, tooltip) in tools {
                                let is_sel = match (self.canvas_state.current_tool, tool_var) {
                                    (ActiveTool::Shape(_), ActiveTool::Shape(_)) => true,
                                    (a, b) => a == b,
                                };
                                let btn = Button::new(
                                    RichText::new(icon)
                                        .size(14.0)
                                        .strong()
                                        .color(if is_sel {
                                            Color32::from_rgb(30, 58, 95)
                                        } else {
                                            Color32::WHITE
                                        }),
                                )
                                .fill(if is_sel {
                                    Color32::from_rgb(214, 232, 255)
                                } else {
                                    Color32::TRANSPARENT
                                })
                                .corner_radius(7.0)
                                .min_size(Vec2::new(28.0, 28.0));

                                if center_ui.add(btn).on_hover_text(tooltip).clicked() {
                                    if is_sel {
                                        self.show_tool_popup = !self.show_tool_popup;
                                        self.show_more_menu_popup = false;
                                        self.show_doc_title_popup = false;
                                    } else {
                                        self.canvas_state.current_tool = tool_var;
                                    }
                                }
                            }

                            // Insert Image Icon (`🖼`)
                            let img_btn = Button::new(
                                RichText::new("🖼").size(14.0).color(Color32::WHITE),
                            )
                            .fill(Color32::TRANSPARENT)
                            .corner_radius(7.0)
                            .min_size(Vec2::new(28.0, 28.0));
                            if center_ui
                                .add(img_btn)
                                .on_hover_text(
                                    lang.tr("Sisipkan Gambar ke Halaman", "Insert Image to Page"),
                                )
                                .clicked()
                                && let Some(path) = rfd::FileDialog::new()
                                    .add_filter("Images", &["png", "jpg", "jpeg", "webp"])
                                    .pick_file()
                                && let Ok(bytes) = std::fs::read(&path)
                            {
                                let ext =
                                    path.extension().and_then(|s| s.to_str()).unwrap_or("png");
                                if self.insert_image_to_page(nb_idx, &bytes, ext, None) {
                                    self.toast_message = Some(
                                        "✅ Gambar berhasil disisipkan ke halaman.".to_string(),
                                    );
                                }
                            }

                            // Active Stroke & Color Pop-up Pill (`● 3.0px ▾`)
                            let (sw_rect, sw_resp) = center_ui
                                .allocate_exact_size(Vec2::new(74.0, 28.0), Sense::click());
                            let pill_bg = if self.show_tool_popup || sw_resp.hovered() {
                                Color32::from_rgb(54, 95, 145)
                            } else {
                                Color32::from_rgb(31, 56, 90)
                            };
                            center_ui.painter().rect_filled(sw_rect, 14.0, pill_bg);
                            let dot_center = Pos2::new(sw_rect.left() + 14.0, sw_rect.center().y);
                            center_ui.painter().circle(
                                dot_center,
                                7.0,
                                self.canvas_state.stroke_color,
                                Stroke::new(1.5_f32, Color32::WHITE),
                            );
                            center_ui.painter().text(
                                Pos2::new(sw_rect.left() + 26.0, sw_rect.center().y),
                                egui::Align2::LEFT_CENTER,
                                format!("{:.1}px ▾", self.canvas_state.stroke_width),
                                FontId::proportional(11.5),
                                Color32::WHITE,
                            );
                            if sw_resp
                                .on_hover_text(lang.tr(
                                    "Pengaturan Ketebalan Pena, Warna & Opsi Alat",
                                    "Stroke Width, Color Palette & Tool Options Pop-up",
                                ))
                                .clicked()
                            {
                                self.show_tool_popup = !self.show_tool_popup;
                                self.show_more_menu_popup = false;
                                self.show_doc_title_popup = false;
                            }
                        }
                    });
                });

                // Pada jendela sempit (< 1020px), baris kedua merender Tools tepat di tengah (Center) agar tidak menabrak Kiri/Kanan
                if !is_wide {
                    ui.add_space(3.0);
                    ui.vertical_centered(|ui| {
                        let tools_width = 456.0_f32.min(total_w - 16.0);
                        let start_x = ((total_w - tools_width) * 0.5).max(8.0);
                        ui.horizontal_wrapped(|ui| {
                            ui.add_space(start_x);
                            ui.spacing_mut().item_spacing.x = 4.0;
                            let active_shape_kind = match self.canvas_state.current_tool {
                                ActiveTool::Shape(k) => k,
                                _ => ShapeKind::Rectangle,
                            };
                            let tools = [
                                (
                                    ActiveTool::Pen,
                                    "✒",
                                    "Pen (P) — Klik lagi untuk atur ketebalan & warna",
                                ),
                                (ActiveTool::Pencil, "✏", "Pencil — Goresan pensil"),
                                (
                                    ActiveTool::Highlighter,
                                    "🖍",
                                    "Highlighter (H) — Stabilo transparan",
                                ),
                                (
                                    ActiveTool::Eraser,
                                    "⌫",
                                    "Eraser (E) — Klik lagi untuk opsi Partial / Whole Stroke",
                                ),
                                (
                                    ActiveTool::Shape(active_shape_kind),
                                    "📐",
                                    "Shapes — Klik lagi untuk pilih bentuk",
                                ),
                                (ActiveTool::Lasso, "✂", "Lasso Selection (L)"),
                                (ActiveTool::Text, "T", "Text Box (T)"),
                                (ActiveTool::StickyNote, "🗒", "Sticky Note"),
                                (ActiveTool::Laser, "✨", "Laser Pointer"),
                                (ActiveTool::Hand, "✋", "Hand / Pan Mode (Space)"),
                            ];

                            for (tool_var, icon, tooltip) in tools {
                                let is_sel = match (self.canvas_state.current_tool, tool_var) {
                                    (ActiveTool::Shape(_), ActiveTool::Shape(_)) => true,
                                    (a, b) => a == b,
                                };
                                let btn = Button::new(
                                    RichText::new(icon)
                                        .size(14.0)
                                        .strong()
                                        .color(if is_sel {
                                            Color32::from_rgb(30, 58, 95)
                                        } else {
                                            Color32::WHITE
                                        }),
                                )
                                .fill(if is_sel {
                                    Color32::from_rgb(214, 232, 255)
                                } else {
                                    Color32::TRANSPARENT
                                })
                                .corner_radius(7.0)
                                .min_size(Vec2::new(28.0, 28.0));

                                if ui.add(btn).on_hover_text(tooltip).clicked() {
                                    if is_sel {
                                        self.show_tool_popup = !self.show_tool_popup;
                                        self.show_more_menu_popup = false;
                                        self.show_doc_title_popup = false;
                                    } else {
                                        self.canvas_state.current_tool = tool_var;
                                    }
                                }
                            }

                            let img_btn = Button::new(
                                RichText::new("🖼").size(14.0).color(Color32::WHITE),
                            )
                            .fill(Color32::TRANSPARENT)
                            .corner_radius(7.0)
                            .min_size(Vec2::new(28.0, 28.0));
                            if ui
                                .add(img_btn)
                                .on_hover_text(
                                    lang.tr("Sisipkan Gambar ke Halaman", "Insert Image to Page"),
                                )
                                .clicked()
                                && let Some(path) = rfd::FileDialog::new()
                                    .add_filter("Images", &["png", "jpg", "jpeg", "webp"])
                                    .pick_file()
                                && let Ok(bytes) = std::fs::read(&path)
                            {
                                let ext =
                                    path.extension().and_then(|s| s.to_str()).unwrap_or("png");
                                if self.insert_image_to_page(nb_idx, &bytes, ext, None) {
                                    self.toast_message = Some(
                                        "✅ Gambar berhasil disisipkan ke halaman.".to_string(),
                                    );
                                }
                            }

                            let (sw_rect, sw_resp) =
                                ui.allocate_exact_size(Vec2::new(74.0, 28.0), Sense::click());
                            let pill_bg = if self.show_tool_popup || sw_resp.hovered() {
                                Color32::from_rgb(54, 95, 145)
                            } else {
                                Color32::from_rgb(31, 56, 90)
                            };
                            ui.painter().rect_filled(sw_rect, 14.0, pill_bg);
                            let dot_center = Pos2::new(sw_rect.left() + 14.0, sw_rect.center().y);
                            ui.painter().circle(
                                dot_center,
                                7.0,
                                self.canvas_state.stroke_color,
                                Stroke::new(1.5_f32, Color32::WHITE),
                            );
                            ui.painter().text(
                                Pos2::new(sw_rect.left() + 26.0, sw_rect.center().y),
                                egui::Align2::LEFT_CENTER,
                                format!("{:.1}px ▾", self.canvas_state.stroke_width),
                                FontId::proportional(11.5),
                                Color32::WHITE,
                            );
                            if sw_resp
                                .on_hover_text(lang.tr(
                                    "Pengaturan Ketebalan Pena, Warna & Opsi Alat",
                                    "Stroke Width, Color Palette & Tool Options Pop-up",
                                ))
                                .clicked()
                            {
                                self.show_tool_popup = !self.show_tool_popup;
                                self.show_more_menu_popup = false;
                                self.show_doc_title_popup = false;
                            }
                        });
                    });
                }
                ui.add_space(4.0);
            });

        if return_to_library {
            let nb_id = notebook_id.clone();
            self.force_save_notebook(&nb_id);
            self.active_notebook_id = None;
            return;
        }

        // =====================================================================
        // GOODNOTES POP-UP 1: Document Title & Info Pop-up (`[Title] ▾`)
        // =====================================================================
        if self.show_doc_title_popup {
            egui::Window::new("📄 Info & Judul Dokumen")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::LEFT_TOP, [44.0, 50.0])
                .show(ctx, |ui| {
                    ui.set_width(290.0);
                    ui.label(
                        RichText::new(lang.tr("Judul Catatan:", "Notebook Title:"))
                            .size(12.0)
                            .color(Color32::GRAY),
                    );
                    if ui
                        .text_edit_singleline(&mut self.library.notebooks[nb_idx].title)
                        .changed()
                    {
                        self.mark_dirty(&notebook_id);
                    }
                    ui.add_space(6.0);
                    let page_cnt = self.library.notebooks[nb_idx].pages.len();
                    let cur_p = &self.library.notebooks[nb_idx].pages[self.current_page_index];
                    ui.label(
                        RichText::new(format!(
                            "Halaman Aktif: {} dari {} ({:.0} × {:.0} pt, {:?})",
                            self.current_page_index + 1,
                            page_cnt,
                            cur_p.width,
                            cur_p.height,
                            cur_p.template
                        ))
                        .size(11.5)
                        .color(Color32::LIGHT_GRAY),
                    );
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if ui.button("📐 Atur Ukuran Kertas...").clicked() {
                            self.show_doc_title_popup = false;
                            self.open_page_resize_modal(nb_idx, self.current_page_index);
                        }
                        if ui.button("Tutup").clicked() {
                            self.show_doc_title_popup = false;
                        }
                    });
                });
        }

        // =====================================================================
        // GOODNOTES POP-UP 2: Tool Settings Pop-up (Ketebalan, Warna, Bentuk, Eraser)
        // =====================================================================
        if self.show_tool_popup {
            egui::Window::new("🎨 Pengaturan Alat & Palet")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_TOP, [0.0, 50.0])
                .show(ctx, |ui| {
                    ui.set_width(360.0);

                    // Ketebalan Goresan (Stroke Width Presets + Slider)
                    ui.label(
                        RichText::new(
                            lang.tr("Ketebalan Goresan (Stroke Width):", "Stroke Width:"),
                        )
                        .strong()
                        .size(12.0),
                    );
                    ui.horizontal_wrapped(|ui| {
                        let mut remove_width_idx = None;
                        let widths = self.settings.pen_widths.clone();
                        for (idx, &w) in widths.iter().enumerate() {
                            let is_sel = (self.canvas_state.stroke_width - w).abs() < 0.1;
                            let btn = Button::new(format!("{w:.1}px"))
                                .fill(if is_sel {
                                    Color32::from_rgb(0, 114, 206)
                                } else {
                                    Color32::from_rgb(65, 63, 64)
                                })
                                .corner_radius(6.0);
                            let resp = ui.add(btn);
                            if resp.clicked() {
                                self.canvas_state.stroke_width = w;
                            }
                            if resp.secondary_clicked() && self.settings.pen_widths.len() > 1 {
                                remove_width_idx = Some(idx);
                            }
                            resp.on_hover_text(
                                "Klik kiri: pilih ketebalan | Klik kanan: hapus preset",
                            );
                        }
                        if let Some(rem) = remove_width_idx {
                            self.settings.pen_widths.remove(rem);
                            let _ = save_settings_to_dir(&app_storage_dir(), &self.settings);
                        }
                        if ui
                            .small_button("+")
                            .on_hover_text("Tambah preset ketebalan baru (+2.0px)")
                            .clicked()
                        {
                            let next_w = (self.settings.pen_widths.last().copied().unwrap_or(2.0)
                                + 2.0)
                                .min(24.0);
                            if !self
                                .settings
                                .pen_widths
                                .iter()
                                .any(|&x| (x - next_w).abs() < 0.1)
                            {
                                self.settings.pen_widths.push(next_w);
                                let _ = save_settings_to_dir(&app_storage_dir(), &self.settings);
                            }
                        }
                    });
                    ui.add(
                        egui::Slider::new(&mut self.canvas_state.stroke_width, 0.5..=28.0)
                            .text("px"),
                    );

                    ui.separator();

                    // Palet Warna (Color Swatches + Color Picker)
                    ui.label(
                        RichText::new(lang.tr("Palet Warna (Color Swatches):", "Color Swatches:"))
                            .strong()
                            .size(12.0),
                    );
                    ui.horizontal_wrapped(|ui| {
                        let mut remove_swatch_idx = None;
                        let swatches = self.settings.color_swatches.clone();
                        for (idx, c_data) in swatches.iter().enumerate() {
                            let c = c_data.to_color32();
                            let (rect, resp) =
                                ui.allocate_exact_size(Vec2::new(24.0, 24.0), Sense::click());
                            let stroke = if self.canvas_state.stroke_color == c {
                                Stroke::new(2.5_f32, Color32::from_rgb(76, 147, 236))
                            } else {
                                Stroke::new(1.0_f32, Color32::from_rgb(100, 100, 100))
                            };
                            ui.painter_at(rect)
                                .rect(rect, 12.0, c, stroke, StrokeKind::Outside);
                            if resp.clicked() {
                                self.canvas_state.stroke_color = c;
                            }
                            if resp.secondary_clicked() && self.settings.color_swatches.len() > 1 {
                                remove_swatch_idx = Some(idx);
                            }
                            resp.on_hover_text("Klik kiri: pilih warna | Klik kanan: hapus swatch");
                        }
                        if let Some(rem) = remove_swatch_idx {
                            self.settings.color_swatches.remove(rem);
                            let _ = save_settings_to_dir(&app_storage_dir(), &self.settings);
                        }
                        ui.color_edit_button_srgba(&mut self.canvas_state.stroke_color);
                        if ui
                            .small_button("+ Simpan Warna")
                            .on_hover_text("Simpan warna aktif ke palet swatch")
                            .clicked()
                        {
                            let new_sw: crate::model::ColorData =
                                self.canvas_state.stroke_color.into();
                            if !self.settings.color_swatches.contains(&new_sw) {
                                self.settings.color_swatches.push(new_sw);
                                let _ = save_settings_to_dir(&app_storage_dir(), &self.settings);
                            }
                        }
                    });

                    // Opsi Spesifik Bentuk Geometri (Shapes)
                    if matches!(self.canvas_state.current_tool, ActiveTool::Shape(_)) {
                        ui.separator();
                        ui.label(RichText::new("Jenis Bentuk (Shape):").strong().size(12.0));
                        ui.horizontal_wrapped(|ui| {
                            let scalene_preset = ShapeKind::ScaleneTriangle {
                                u0: 0.22,
                                v0: 0.0,
                                u1: 1.0,
                                v1: 0.72,
                                u2: 0.0,
                                v2: 1.0,
                            };
                            let shapes = [
                                (ShapeKind::Rectangle, "▭ Rectangle"),
                                (ShapeKind::Parallelogram, "▱ Jajar Genjang"),
                                (ShapeKind::Ellipse, "◯ Circle / Ellipse"),
                                (ShapeKind::Line, "╱ Line"),
                                (ShapeKind::Triangle, "△ Segitiga Sama Kaki"),
                                (ShapeKind::RightTriangle, "◣ Segitiga Siku-Siku"),
                                (scalene_preset, "◿ Segitiga Sembarang"),
                            ];
                            for (kind, name) in shapes {
                                let is_sel = match (self.canvas_state.current_tool, kind) {
                                    (
                                        ActiveTool::Shape(ShapeKind::ScaleneTriangle { .. }),
                                        ShapeKind::ScaleneTriangle { .. },
                                    ) => true,
                                    (ActiveTool::Shape(cur), k) => cur == k,
                                    _ => false,
                                };
                                if ui.selectable_label(is_sel, name).clicked() {
                                    self.canvas_state.current_tool = ActiveTool::Shape(kind);
                                }
                            }
                        });
                    }

                    // Opsi Spesifik Penghapus (Eraser Mode)
                    if self.canvas_state.current_tool == ActiveTool::Eraser {
                        ui.separator();
                        ui.label(
                            RichText::new("Mode Penghapus (Eraser Mode):")
                                .strong()
                                .size(12.0),
                        );
                        ui.horizontal(|ui| {
                            let modes = [
                                (EraserMode::Partial, "✂ Partial (Potong Segmen)"),
                                (EraserMode::WholeStroke, "⌫ Whole Stroke (Hapus Utuh)"),
                            ];
                            for (mode, label) in modes {
                                let is_sel = self.canvas_state.eraser_mode == mode;
                                if ui.selectable_label(is_sel, label).clicked() {
                                    self.canvas_state.eraser_mode = mode;
                                }
                            }
                        });
                    }

                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.button("Selesai").clicked() {
                                self.show_tool_popup = false;
                            }
                        });
                    });
                });
        }

        // =====================================================================
        // GOODNOTES POP-UP 3: Authentic GoodNotes Right Pop-up Menu (`⋯`)
        // =====================================================================
        if self.show_more_menu_popup {
            let page_count = self.library.notebooks[nb_idx].pages.len();
            egui::Window::new(format!("Page {}", self.current_page_index + 1))
                .id(egui::Id::new("goodnotes_more_menu_popup"))
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::RIGHT_TOP, [-12.0, 50.0])
                .frame(
                    egui::Frame::window(&ctx.style())
                        .fill(Color32::from_rgb(26, 26, 28))
                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(62, 62, 66)))
                        .corner_radius(12.0)
                        .inner_margin(12.0),
                )
                .show(ctx, |ui| {
                    ui.set_width(275.0);
                    ui.spacing_mut().item_spacing.y = 5.0;

                    // --- Section 1: Page Actions (seperti GoodNotes) ---
                    let is_bm =
                        self.library.notebooks[nb_idx].pages[self.current_page_index].bookmarked;
                    if ui
                        .add(
                            Button::new(
                                RichText::new(if is_bm {
                                    "★ Remove Bookmark"
                                } else {
                                    "🔖 Bookmark Page"
                                })
                                .color(Color32::WHITE),
                            )
                            .fill(Color32::TRANSPARENT)
                            .min_size(Vec2::new(260.0, 26.0)),
                        )
                        .clicked()
                    {
                        self.library.notebooks[nb_idx].pages[self.current_page_index].bookmarked =
                            !is_bm;
                        self.mark_dirty(&notebook_id);
                        self.show_more_menu_popup = false;
                    }

                    if ui
                        .add(
                            Button::new(RichText::new("📄 Duplicate Page").color(Color32::WHITE))
                                .fill(Color32::TRANSPARENT)
                                .min_size(Vec2::new(260.0, 26.0)),
                        )
                        .clicked()
                    {
                        self.push_doc_undo(nb_idx);
                        if let Some(new_idx) =
                            self.library.notebooks[nb_idx].duplicate_page(self.current_page_index)
                        {
                            self.navigate_to_page(nb_idx, new_idx);
                            self.mark_dirty(&notebook_id);
                        }
                        self.show_more_menu_popup = false;
                    }

                    if ui
                        .add(
                            Button::new(
                                RichText::new("↻ Rotate Page 90° CW (Searah Jarum Jam)")
                                    .color(Color32::WHITE),
                            )
                            .fill(Color32::TRANSPARENT)
                            .min_size(Vec2::new(260.0, 26.0)),
                        )
                        .clicked()
                    {
                        self.push_doc_undo(nb_idx);
                        self.library.notebooks[nb_idx].rotate_page(self.current_page_index, 90);
                        self.mark_dirty(&notebook_id);
                        self.toast_message =
                            Some("↻ Halaman diputar 90° searah jarum jam.".to_string());
                        self.show_more_menu_popup = false;
                    }

                    if ui
                        .add(
                            Button::new(
                                RichText::new("↺ Rotate Page 90° CCW (Berlawanan Jarum Jam)")
                                    .color(Color32::WHITE),
                            )
                            .fill(Color32::TRANSPARENT)
                            .min_size(Vec2::new(260.0, 26.0)),
                        )
                        .clicked()
                    {
                        self.push_doc_undo(nb_idx);
                        self.library.notebooks[nb_idx].rotate_page(self.current_page_index, -90);
                        self.mark_dirty(&notebook_id);
                        self.toast_message =
                            Some("↺ Halaman diputar 90° berlawanan arah jarum jam.".to_string());
                        self.show_more_menu_popup = false;
                    }

                    if ui
                        .add(
                            Button::new(RichText::new("⟲ Rotate Page 180°").color(Color32::WHITE))
                                .fill(Color32::TRANSPARENT)
                                .min_size(Vec2::new(260.0, 26.0)),
                        )
                        .clicked()
                    {
                        self.push_doc_undo(nb_idx);
                        self.library.notebooks[nb_idx].rotate_page(self.current_page_index, 180);
                        self.mark_dirty(&notebook_id);
                        self.toast_message = Some("⟲ Halaman diputar 180°.".to_string());
                        self.show_more_menu_popup = false;
                    }

                    if ui
                        .add(
                            Button::new(
                                RichText::new("📑 Add Page to Outline...").color(Color32::WHITE),
                            )
                            .fill(Color32::TRANSPARENT)
                            .min_size(Vec2::new(260.0, 26.0)),
                        )
                        .clicked()
                    {
                        self.new_outline_title = format!("Halaman {}", self.current_page_index + 1);
                        self.show_add_outline_modal = true;
                        self.show_more_menu_popup = false;
                    }

                    if ui
                        .add(
                            Button::new(
                                RichText::new("📐 Change Template & Paper Size...")
                                    .color(Color32::WHITE),
                            )
                            .fill(Color32::TRANSPARENT)
                            .min_size(Vec2::new(260.0, 26.0)),
                        )
                        .clicked()
                    {
                        self.show_more_menu_popup = false;
                        self.open_page_resize_modal(nb_idx, self.current_page_index);
                    }

                    if ui
                        .add(
                            Button::new(
                                RichText::new("💾 Save Page as Custom Template...")
                                    .color(Color32::WHITE),
                            )
                            .fill(Color32::TRANSPARENT)
                            .min_size(Vec2::new(260.0, 26.0)),
                        )
                        .clicked()
                    {
                        self.show_more_menu_popup = false;
                        self.show_save_template_modal = true;
                        self.new_template_name =
                            format!("Template {}", self.current_page_index + 1);
                    }

                    // Go to Page row (`Go to Page   ◀ 1 of N ▶`)
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("➡ Go to Page").color(Color32::WHITE));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if self.current_page_index + 1 < page_count
                                && ui.small_button("▶").clicked()
                            {
                                let next_idx = self.current_page_index + 1;
                                self.navigate_to_page(nb_idx, next_idx);
                            }
                            ui.label(
                                RichText::new(format!(
                                    "{} of {}",
                                    self.current_page_index + 1,
                                    page_count
                                ))
                                .size(11.5)
                                .color(Color32::GRAY),
                            );
                            if self.current_page_index > 0 && ui.small_button("◀").clicked() {
                                let prev_idx = self.current_page_index - 1;
                                self.navigate_to_page(nb_idx, prev_idx);
                            }
                        });
                    });

                    ui.separator();

                    // --- Section 2: Clear or Trash Page (GoodNotes Red Actions) ---
                    ui.label(
                        RichText::new("Clear or Trash Page")
                            .size(11.0)
                            .strong()
                            .color(Color32::GRAY),
                    );
                    if ui
                        .add(
                            Button::new(
                                RichText::new("🧹 Clear Page")
                                    .color(Color32::from_rgb(255, 99, 92)),
                            )
                            .fill(Color32::TRANSPARENT)
                            .min_size(Vec2::new(260.0, 24.0)),
                        )
                        .clicked()
                    {
                        if !self.library.notebooks[nb_idx].pages[self.current_page_index]
                            .objects
                            .is_empty()
                        {
                            self.push_doc_undo(nb_idx);
                            self.library.notebooks[nb_idx].pages[self.current_page_index]
                                .objects
                                .clear();
                            self.mark_dirty(&notebook_id);
                        }
                        self.show_more_menu_popup = false;
                    }

                    if page_count > 1
                        && ui
                            .add(
                                Button::new(
                                    RichText::new("🗑 Move Page to Trash")
                                        .color(Color32::from_rgb(255, 99, 92)),
                                )
                                .fill(Color32::TRANSPARENT)
                                .min_size(Vec2::new(260.0, 24.0)),
                            )
                            .clicked()
                    {
                        self.push_doc_undo(nb_idx);
                        if let Ok(new_idx) =
                            self.library.notebooks[nb_idx].delete_page(self.current_page_index)
                        {
                            self.navigate_to_page(nb_idx, new_idx);
                            self.mark_dirty(&notebook_id);
                        }
                        self.show_more_menu_popup = false;
                    }

                    ui.separator();

                    // --- Section 3: Fitur Lanjutan (Advanced Features) ---
                    ui.label(
                        RichText::new("Advanced Features")
                            .size(11.0)
                            .strong()
                            .color(Color32::GRAY),
                    );
                    if ui
                        .add(
                            Button::new(
                                RichText::new("🔍 Handwriting OCR (Ekstrak Teks)...")
                                    .color(Color32::WHITE),
                            )
                            .fill(Color32::TRANSPARENT)
                            .min_size(Vec2::new(260.0, 24.0)),
                        )
                        .clicked()
                    {
                        let p = &mut self.library.notebooks[nb_idx].pages[self.current_page_index];
                        let extracted =
                            crate::geometry::recognize_handwriting_from_strokes(&p.objects);
                        if !extracted.is_empty() {
                            p.ocr_text = extracted;
                            self.mark_dirty(&notebook_id);
                        }
                        self.show_ocr_modal = true;
                        self.show_more_menu_popup = false;
                    }

                    ui.horizontal(|ui| {
                        ui.label(RichText::new("✋ Palm Rejection").color(Color32::WHITE));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.checkbox(&mut self.settings.palm_rejection, "").changed() {
                                self.canvas_state.palm_rejection_enabled =
                                    self.settings.palm_rejection;
                                let _ = save_settings_to_dir(&app_storage_dir(), &self.settings);
                            }
                        });
                    });

                    ui.horizontal(|ui| {
                        ui.label(RichText::new("🔍 Zoom Window").color(Color32::WHITE));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.checkbox(&mut self.canvas_state.show_zoom_window, "");
                        });
                    });

                    ui.separator();

                    // --- Section 4: Settings (Scrolling Direction, Zoom, Theme, Language, Shortcuts) ---
                    ui.label(
                        RichText::new("Settings")
                            .size(11.0)
                            .strong()
                            .color(Color32::GRAY),
                    );

                    ui.horizontal(|ui| {
                        ui.label(RichText::new("📜 Page Mode").color(Color32::WHITE));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui
                                .selectable_label(
                                    self.view_mode == ViewMode::SinglePage,
                                    "Single Page",
                                )
                                .clicked()
                            {
                                self.set_view_mode(nb_idx, ViewMode::SinglePage);
                            }
                            if ui
                                .selectable_label(
                                    self.view_mode == ViewMode::ContinuousVertical,
                                    "Scrollable",
                                )
                                .clicked()
                            {
                                self.set_view_mode(nb_idx, ViewMode::ContinuousVertical);
                            }
                        });
                    });

                    ui.horizontal(|ui| {
                        ui.label(RichText::new("🔎 Canvas Zoom").color(Color32::WHITE));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.small_button("+").clicked() {
                                let z = (self.canvas_state.zoom + 0.1).min(2.5);
                                self.apply_zoom_anchored(nb_idx, z, None);
                            }
                            if ui
                                .small_button(format!(
                                    "{}%",
                                    (self.canvas_state.zoom * 100.0) as i32
                                ))
                                .on_hover_text("Klik untuk reset Zoom ke 100%")
                                .clicked()
                            {
                                self.apply_zoom_anchored(nb_idx, 1.0, None);
                                self.canvas_pan_offset = Vec2::ZERO;
                            }
                            if ui.small_button("-").clicked() {
                                let z = (self.canvas_state.zoom - 0.1).max(0.4);
                                self.apply_zoom_anchored(nb_idx, z, None);
                            }
                        });
                    });

                    ui.horizontal(|ui| {
                        ui.label(RichText::new("🎨 Theme").color(Color32::WHITE));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let theme_icon = match self.settings.theme_mode {
                                ThemeMode::Dark => "🌙 Dark",
                                ThemeMode::Light => "☀ Light",
                                ThemeMode::System => "💻 System",
                            };
                            if ui.small_button(theme_icon).clicked() {
                                self.settings.theme_mode = self.settings.theme_mode.next();
                                let _ = save_settings_to_dir(&app_storage_dir(), &self.settings);
                            }
                        });
                    });

                    ui.horizontal(|ui| {
                        ui.label(RichText::new("🌐 Language").color(Color32::WHITE));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.small_button(lang.badge()).clicked() {
                                self.settings.language = self.settings.language.toggle();
                                let _ = save_settings_to_dir(&app_storage_dir(), &self.settings);
                            }
                        });
                    });

                    if ui
                        .add(
                            Button::new(
                                RichText::new("⌨ Keyboard Shortcuts (F1)").color(Color32::WHITE),
                            )
                            .fill(Color32::TRANSPARENT)
                            .min_size(Vec2::new(260.0, 24.0)),
                        )
                        .clicked()
                    {
                        self.show_more_menu_popup = false;
                        self.show_shortcut_modal = true;
                    }
                });
        }

        // Left Sidebar: Tabbed Sidebar Panel (Pages / Bookmarks / Outline)
        if self.show_thumbnail_sidebar {
            egui::SidePanel::left("editor_thumbnail_sidebar")
                .exact_width(185.0)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.add_space(8.0);

                    // Sidebar Tab Selector (Pages, Bookmarks, Outline)
                    ui.horizontal(|ui| {
                        let tabs = [
                            (SidebarTab::Thumbnails, "📄 Pages"),
                            (SidebarTab::Bookmarks, "🔖 Marks"),
                            (SidebarTab::Outline, "📑 Outline"),
                        ];
                        for (tab, label) in tabs {
                            let is_sel = self.sidebar_tab == tab;
                            if ui
                                .selectable_label(is_sel, RichText::new(label).size(11.0))
                                .clicked()
                            {
                                self.sidebar_tab = tab;
                            }
                        }
                    });
                    ui.separator();

                    match self.sidebar_tab {
                        SidebarTab::Thumbnails => {
                            let page_count = self.library.notebooks[nb_idx].pages.len();
                            ui.horizontal(|ui| {
                                if self.thumb_select_mode {
                                    ui.label(
                                        RichText::new(format!(
                                            "{} dipilih",
                                            self.thumb_selected_pages.len()
                                        ))
                                        .strong()
                                        .color(Color32::WHITE),
                                    );
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        if ui.button("✕").on_hover_text("Batal mode seleksi").clicked() {
                                            self.thumb_select_mode = false;
                                            self.thumb_selected_pages.clear();
                                        }
                                        if ui.button("Semua").on_hover_text("Pilih semua halaman").clicked() {
                                            for i in 0..page_count {
                                                self.thumb_selected_pages.insert(i);
                                            }
                                        }
                                    });
                                } else {
                                    ui.label(RichText::new("Thumbnails").strong().color(Color32::WHITE));
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        if ui.button("➕ Add").on_hover_text("Atur & Tambah Halaman Baru").clicked() {
                                            self.open_add_page_modal(nb_idx);
                                        }
                                        if ui.button("☑ Pilih").on_hover_text("Pilih multi halaman untuk aksi massal").clicked() {
                                            self.thumb_select_mode = true;
                                            self.thumb_selected_pages.clear();
                                            self.thumb_selected_pages.insert(self.current_page_index);
                                        }
                                    });
                                }
                            });

                            // Bulk Actions Toolbar jika dalam mode multi-select
                            if self.thumb_select_mode && !self.thumb_selected_pages.is_empty() {
                                ui.add_space(4.0);
                                ui.horizontal_wrapped(|ui| {
                                    ui.spacing_mut().item_spacing.x = 4.0;
                                    // 1. Bulk Delete (hanya jika sisa minimal 1 halaman)
                                    if self.thumb_selected_pages.len() < page_count
                                        && ui.button("🗑 Hapus").on_hover_text("Hapus halaman terpilih").clicked()
                                    {
                                        self.push_doc_undo(nb_idx);
                                        let to_delete: Vec<usize> =
                                            self.thumb_selected_pages.iter().copied().rev().collect();
                                        for idx in to_delete {
                                            self.library.notebooks[nb_idx].pages.remove(idx);
                                        }
                                        self.thumb_selected_pages.clear();
                                        self.thumb_select_mode = false;
                                        self.current_page_index = self
                                            .current_page_index
                                            .min(self.library.notebooks[nb_idx].pages.len().saturating_sub(1));
                                        self.mark_dirty(&notebook_id);
                                        self.toast_message =
                                            Some("🗑 Halaman terpilih berhasil dihapus.".to_string());
                                    }

                                    // 2. Bulk Duplicate
                                    if ui.button("📄 Salin").on_hover_text("Gandakan halaman terpilih").clicked() {
                                        self.push_doc_undo(nb_idx);
                                        let to_dup: Vec<usize> =
                                            self.thumb_selected_pages.iter().copied().collect();
                                        for (offset, idx) in to_dup.into_iter().enumerate() {
                                            let page_to_copy = self.library.notebooks[nb_idx].pages[idx + offset].clone();
                                            self.library.notebooks[nb_idx]
                                                .pages
                                                .insert(idx + offset + 1, page_to_copy);
                                        }
                                        self.thumb_selected_pages.clear();
                                        self.thumb_select_mode = false;
                                        self.mark_dirty(&notebook_id);
                                        self.toast_message =
                                            Some("📄 Halaman terpilih berhasil digandakan.".to_string());
                                    }

                                    // 3. Bulk Rotate 90 CW
                                    if ui.button("↻ Putar").on_hover_text("Putar halaman terpilih 90° CW").clicked() {
                                        self.push_doc_undo(nb_idx);
                                        for &idx in &self.thumb_selected_pages {
                                            self.library.notebooks[nb_idx].rotate_page(idx, 90);
                                        }
                                        self.mark_dirty(&notebook_id);
                                        self.toast_message =
                                            Some("↻ Halaman terpilih berhasil diputar 90°.".to_string());
                                    }

                                    // 4. Bulk Export to PDF
                                    if ui.button("📤 Ekspor").on_hover_text("Ekspor halaman terpilih ke PDF").clicked() {
                                        let default_name = format!(
                                            "{}_selected.pdf",
                                            self.library.notebooks[nb_idx].title.replace(
                                                ['/', '\\', ':', '*', '?', '"', '<', '>', '|'],
                                                "_"
                                            )
                                        );
                                        if let Some(path) = rfd::FileDialog::new()
                                            .set_file_name(&default_name)
                                            .add_filter("PDF Document", &["pdf"])
                                            .save_file()
                                        {
                                            let storage_dir = app_storage_dir();
                                            let mut temp_nb = self.library.notebooks[nb_idx].clone();
                                            temp_nb.pages = self
                                                .thumb_selected_pages
                                                .iter()
                                                .filter_map(|&i| {
                                                    self.library.notebooks[nb_idx].pages.get(i).cloned()
                                                })
                                                .collect();
                                            match export_notebook_to_pdf(&storage_dir, &temp_nb, &path) {
                                                Ok(()) => {
                                                    self.toast_message = Some(
                                                        "✅ Halaman terpilih berhasil diekspor ke PDF."
                                                            .to_string(),
                                                    );
                                                }
                                                Err(e) => {
                                                    self.toast_message =
                                                        Some(format!("⚠ Gagal ekspor PDF: {e}"));
                                                }
                                            }
                                        }
                                    }
                                });
                            }
                            ui.separator();

                            ScrollArea::vertical()
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    let mut action_select = None;
                                    let mut action_duplicate = None;
                                    let mut action_delete = None;
                                    let mut action_move_up = None;
                                    let mut action_move_down = None;
                                    let mut action_insert_before = None;
                                    let mut action_insert_after = None;
                                    let mut action_change_size = None;
                                    let mut action_rotate_cw = None;
                                    let mut action_rotate_ccw = None;
                                    let mut action_rotate_180 = None;
                                    let mut action_move_drag = None;
                                    let page_count = self.library.notebooks[nb_idx].pages.len();
                                    self.canvas_state.active_notebook_id = Some(notebook_id.clone());

                                    let pointer_released = ui.input(|i| i.pointer.any_released());

                                    for p_i in 0..page_count {
                                        // Pre-cache PDF/Image background & Image object textures for this thumbnail
                                        let mut needed_assets: Vec<String> = Vec::new();
                                        {
                                            let p_ref = &self.library.notebooks[nb_idx].pages[p_i];
                                            if let crate::model::PageBackground::Image { asset_id }
                                            | crate::model::PageBackground::Pdf {
                                                asset_id, ..
                                            } = p_ref.effective_background()
                                            {
                                                needed_assets.push(asset_id);
                                            }
                                            for obj in &p_ref.objects {
                                                if let crate::model::DrawObject::Image {
                                                    asset_id,
                                                    ..
                                                } = obj
                                                {
                                                    needed_assets.push(asset_id.clone());
                                                }
                                            }
                                        }
                                        for asset_id in needed_assets {
                                            self.canvas_state.get_or_load_texture(ui.ctx(), &asset_id);
                                        }

                                        let p = &self.library.notebooks[nb_idx].pages[p_i];
                                        let is_active = p_i == self.current_page_index;
                                        ui.vertical_centered(|ui| {
                                            let thumb_w = 110.0;
                                            let ratio = p.height / p.width.max(1.0);
                                            let thumb_h = (thumb_w * ratio).clamp(60.0, 160.0);

                                            let (rect, resp) = ui.allocate_exact_size(
                                                Vec2::new(thumb_w, thumb_h),
                                                Sense::click_and_drag(),
                                            );
                                            let painter = ui.painter_at(rect);

                                            // Deteksi Drag-and-Drop Reorder Halaman
                                            if resp.drag_started() && !self.thumb_select_mode {
                                                self.dragged_thumb_page = Some(p_i);
                                            }

                                            // Visual Drop Indicator Line saat halaman sedang di-drag
                                            if let Some(from_i) = self.dragged_thumb_page {
                                                if from_i != p_i && resp.hovered() {
                                                    let hover_y = ui
                                                        .input(|i| i.pointer.hover_pos())
                                                        .map_or(rect.center().y, |p| p.y);
                                                    let is_top = hover_y < rect.center().y;
                                                    let line_y = if is_top { rect.top() } else { rect.bottom() };
                                                    let indicator_stroke =
                                                        Stroke::new(3.0_f32, Color32::from_rgb(0, 114, 206));
                                                    painter.line_segment(
                                                        [
                                                            Pos2::new(rect.left(), line_y),
                                                            Pos2::new(rect.right(), line_y),
                                                        ],
                                                        indicator_stroke,
                                                    );
                                                    painter.circle_filled(
                                                        Pos2::new(rect.left() + 2.0, line_y),
                                                        4.0,
                                                        Color32::from_rgb(0, 114, 206),
                                                    );
                                                    painter.circle_filled(
                                                        Pos2::new(rect.right() - 2.0, line_y),
                                                        4.0,
                                                        Color32::from_rgb(0, 114, 206),
                                                    );
                                                }

                                                if pointer_released && resp.hovered() && from_i != p_i {
                                                    let hover_y = ui
                                                        .input(|i| i.pointer.hover_pos())
                                                        .map_or(rect.center().y, |p| p.y);
                                                    let target_pos = if hover_y < rect.center().y {
                                                        p_i
                                                    } else {
                                                        (p_i + 1).min(page_count.saturating_sub(1))
                                                    };
                                                    action_move_drag = Some((from_i, target_pos));
                                                }
                                            }

                                            // 1. Draw miniature paper base color
                                            painter.rect_filled(rect, 3.0, p.paper_color.to_color32());

                                            // 2. Draw PDF / Image background or miniature paper template
                                            let scale = thumb_w / p.width.max(1.0);
                                            let origin = rect.min;
                                            match p.effective_background() {
                                                crate::model::PageBackground::Image { asset_id }
                                                | crate::model::PageBackground::Pdf {
                                                    asset_id,
                                                    ..
                                                } => {
                                                    if let Some(tex) =
                                                        self.canvas_state.texture_cache.get(&asset_id)
                                                    {
                                                        crate::canvas::draw_rotated_background_image(
                                                            &painter,
                                                            tex.id(),
                                                            rect,
                                                            p.rotation,
                                                        );
                                                    }
                                                }
                                                crate::model::PageBackground::Template(tmpl) => {
                                                    let spacing = (30.0 * scale).max(4.0);
                                                    let line_col = Color32::from_rgb(220, 226, 231);
                                                    match tmpl {
                                                        PageTemplate::Blank => {}
                                                        PageTemplate::Ruled | PageTemplate::Cornell => {
                                                            let mut y = rect.top() + spacing;
                                                            while y < rect.bottom() {
                                                                painter.line_segment(
                                                                    [
                                                                        Pos2::new(rect.left(), y),
                                                                        Pos2::new(rect.right(), y),
                                                                    ],
                                                                    Stroke::new(0.5_f32, line_col),
                                                                );
                                                                y += spacing;
                                                            }
                                                        }
                                                        PageTemplate::Grid => {
                                                            let mut y = rect.top() + spacing;
                                                            while y < rect.bottom() {
                                                                painter.line_segment(
                                                                    [
                                                                        Pos2::new(rect.left(), y),
                                                                        Pos2::new(rect.right(), y),
                                                                    ],
                                                                    Stroke::new(0.5_f32, line_col),
                                                                );
                                                                y += spacing;
                                                            }
                                                            let mut x = rect.left() + spacing;
                                                            while x < rect.right() {
                                                                painter.line_segment(
                                                                    [
                                                                        Pos2::new(x, rect.top()),
                                                                        Pos2::new(x, rect.bottom()),
                                                                    ],
                                                                    Stroke::new(0.5_f32, line_col),
                                                                );
                                                                x += spacing;
                                                            }
                                                        }
                                                        PageTemplate::Dotted => {
                                                            let mut y = rect.top() + spacing;
                                                            while y < rect.bottom() {
                                                                let mut x = rect.left() + spacing;
                                                                while x < rect.right() {
                                                                    painter.circle_filled(
                                                                        Pos2::new(x, y),
                                                                        0.6,
                                                                        line_col,
                                                                    );
                                                                    x += spacing;
                                                                }
                                                                y += spacing;
                                                            }
                                                        }
                                                    }
                                                }
                                            }

                                            // 3. Miniature objects preview (strokes, shapes, images, sticky notes, text)
                                            for obj in &p.objects {
                                                match obj {
                                                    crate::model::DrawObject::Stroke {
                                                        points,
                                                        color,
                                                        width,
                                                        is_highlighter,
                                                        ..
                                                    } => {
                                                        if points.len() > 1 {
                                                            let c = if *is_highlighter {
                                                                Color32::from_rgba_unmultiplied(
                                                                    color.r, color.g, color.b, 95,
                                                                )
                                                            } else {
                                                                color.to_color32()
                                                            };
                                                            let stroke = Stroke::new(
                                                                (width * scale).max(0.8),
                                                                c,
                                                            );
                                                            for w in points.windows(2) {
                                                                let p1 = origin
                                                                    + Vec2::new(
                                                                        w[0].x * scale,
                                                                        w[0].y * scale,
                                                                    );
                                                                let p2 = origin
                                                                    + Vec2::new(
                                                                        w[1].x * scale,
                                                                        w[1].y * scale,
                                                                    );
                                                                painter.line_segment([p1, p2], stroke);
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
                                                        ..
                                                    } => {
                                                        let stroke = Stroke::new(
                                                            (stroke_width * scale).max(0.9),
                                                            color.to_color32(),
                                                        );
                                                        match shape_type {
                                                            ShapeKind::Ellipse => {
                                                                let p1 = origin
                                                                    + Vec2::new(x * scale, y * scale);
                                                                let p2 = origin
                                                                    + Vec2::new(
                                                                        (x + width) * scale,
                                                                        (y + height) * scale,
                                                                    );
                                                                let r = Rect::from_two_pos(p1, p2);
                                                                let rad = (r.width().min(r.height())
                                                                    * 0.5)
                                                                    .max(1.0);
                                                                painter.circle_stroke(
                                                                    r.center(),
                                                                    rad,
                                                                    stroke,
                                                                );
                                                            }
                                                            ShapeKind::Line => {
                                                                let p1 = origin
                                                                    + Vec2::new(x * scale, y * scale);
                                                                let p2 = origin
                                                                    + Vec2::new(
                                                                        (x + width) * scale,
                                                                        (y + height) * scale,
                                                                    );
                                                                painter.line_segment([p1, p2], stroke);
                                                            }
                                                            _ => {
                                                                let verts =
                                                                    crate::geometry::shape_vertices(
                                                                        *shape_type,
                                                                        *x,
                                                                        *y,
                                                                        *width,
                                                                        *height,
                                                                    );
                                                                for i in 0..verts.len() {
                                                                    let a = origin
                                                                        + Vec2::new(
                                                                            verts[i].x * scale,
                                                                            verts[i].y * scale,
                                                                        );
                                                                    let b = origin
                                                                        + Vec2::new(
                                                                            verts[(i + 1) % verts.len()]
                                                                                .x
                                                                                * scale,
                                                                            verts[(i + 1) % verts.len()]
                                                                                .y
                                                                                * scale,
                                                                        );
                                                                    painter.line_segment([a, b], stroke);
                                                                }
                                                            }
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
                                                        let center = origin + Vec2::new((*x + *width / 2.0) * scale, (*y + *height / 2.0) * scale);
                                                        let half_w = (*width * scale) / 2.0;
                                                        let half_h = (*height * scale) / 2.0;
                                                        let r = Rect::from_center_size(center, Vec2::new(half_w * 2.0, half_h * 2.0));
                                                        if let Some(tex) =
                                                            self.canvas_state.texture_cache.get(asset_id)
                                                        {
                                                            let rot_deg = (rotation.to_degrees().round() as i32).rem_euclid(360);
                                                            crate::canvas::draw_rotated_background_image(
                                                                &painter,
                                                                tex.id(),
                                                                r,
                                                                rot_deg,
                                                            );
                                                        }
                                                    }
                                                    crate::model::DrawObject::StickyNote {
                                                        x,
                                                        y,
                                                        width,
                                                        height,
                                                        color,
                                                        ..
                                                    } => {
                                                        let p1 =
                                                            origin + Vec2::new(x * scale, y * scale);
                                                        let p2 = origin
                                                            + Vec2::new(
                                                                (x + width) * scale,
                                                                (y + height) * scale,
                                                            );
                                                        let r = Rect::from_two_pos(p1, p2);
                                                        painter.rect_filled(r, 1.5, color.to_color32());
                                                    }
                                                    crate::model::DrawObject::Text {
                                                        x,
                                                        y,
                                                        text,
                                                        color,
                                                        ..
                                                    } => {
                                                        let pos =
                                                            origin + Vec2::new(x * scale, y * scale);
                                                        let preview_text = if text.chars().count() > 8 {
                                                            format!(
                                                                "{}…",
                                                                text.chars().take(7).collect::<String>()
                                                            )
                                                        } else {
                                                            text.clone()
                                                        };
                                                        painter.text(
                                                            pos,
                                                            egui::Align2::LEFT_TOP,
                                                            preview_text,
                                                            FontId::proportional(8.0),
                                                            color.to_color32(),
                                                        );
                                                    }
                                                }
                                            }

                                            // Bookmark badge if page is bookmarked
                                            if p.bookmarked {
                                                let bm_pos = rect.right_top() + Vec2::new(-14.0, 12.0);
                                                painter.text(
                                                    bm_pos,
                                                    egui::Align2::CENTER_CENTER,
                                                    "🔖",
                                                    FontId::proportional(11.0),
                                                    Color32::from_rgb(255, 204, 0),
                                                );
                                            }

                                            // Checkbox badge jika dalam mode multi-select
                                            if self.thumb_select_mode {
                                                let is_selected = self.thumb_selected_pages.contains(&p_i);
                                                let badge_rect = Rect::from_min_size(
                                                    rect.left_top() + Vec2::new(4.0, 4.0),
                                                    Vec2::new(16.0, 16.0),
                                                );
                                                let badge_bg = if is_selected {
                                                    Color32::from_rgb(0, 114, 206)
                                                } else {
                                                    Color32::from_rgba_unmultiplied(30, 30, 30, 190)
                                                };
                                                painter.rect_filled(badge_rect, 3.0, badge_bg);
                                                painter.rect_stroke(
                                                    badge_rect,
                                                    3.0,
                                                    Stroke::new(1.0_f32, Color32::WHITE),
                                                    StrokeKind::Outside,
                                                );
                                                if is_selected {
                                                    painter.text(
                                                        badge_rect.center(),
                                                        egui::Align2::CENTER_CENTER,
                                                        "✓",
                                                        FontId::proportional(11.0),
                                                        Color32::WHITE,
                                                    );
                                                }
                                            }

                                            // Border highlight for active page or selected pages
                                            let is_thumb_selected = self.thumb_selected_pages.contains(&p_i);
                                            let border_color = if is_thumb_selected {
                                                Color32::from_rgb(0, 180, 255)
                                            } else if is_active {
                                                Color32::from_rgb(0, 114, 206)
                                            } else if resp.hovered() {
                                                Color32::from_rgb(90, 90, 90)
                                            } else {
                                                Color32::from_rgb(50, 50, 50)
                                            };
                                            let border_w = if is_active || is_thumb_selected { 2.5_f32 } else { 1.0_f32 };
                                            painter.rect_stroke(
                                                rect,
                                                3.0,
                                                Stroke::new(border_w, border_color),
                                                StrokeKind::Outside,
                                            );

                                            if resp.clicked() {
                                                if self.thumb_select_mode {
                                                    if self.thumb_selected_pages.contains(&p_i) {
                                                        self.thumb_selected_pages.remove(&p_i);
                                                    } else {
                                                        self.thumb_selected_pages.insert(p_i);
                                                    }
                                                } else if ctx.input(|i| {
                                                    i.modifiers.command || i.modifiers.ctrl || i.modifiers.shift
                                                }) {
                                                    self.thumb_select_mode = true;
                                                    self.thumb_selected_pages.clear();
                                                    self.thumb_selected_pages.insert(self.current_page_index);
                                                    self.thumb_selected_pages.insert(p_i);
                                                } else {
                                                    action_select = Some(p_i);
                                                }
                                            }

                                            resp.context_menu(|ui| {
                                                if ui.button("📄 Duplicate Page").clicked() {
                                                    action_duplicate = Some(p_i);
                                                    ui.close_menu();
                                                }
                                                if page_count > 1 && ui.button("🗑 Delete Page").clicked()
                                                {
                                                    action_delete = Some(p_i);
                                                    ui.close_menu();
                                                }
                                                ui.separator();
                                                if ui.button("↻ Rotate 90° CW").clicked() {
                                                    action_rotate_cw = Some(p_i);
                                                    ui.close_menu();
                                                }
                                                if ui.button("↺ Rotate 90° CCW").clicked() {
                                                    action_rotate_ccw = Some(p_i);
                                                    ui.close_menu();
                                                }
                                                if ui.button("⟲ Rotate 180°").clicked() {
                                                    action_rotate_180 = Some(p_i);
                                                    ui.close_menu();
                                                }
                                                ui.separator();
                                                if p_i > 0 && ui.button("⬆ Move Up").clicked() {
                                                    action_move_up = Some(p_i);
                                                    ui.close_menu();
                                                }
                                                if p_i + 1 < page_count
                                                    && ui.button("⬇ Move Down").clicked()
                                                {
                                                    action_move_down = Some(p_i);
                                                    ui.close_menu();
                                                }
                                                ui.separator();
                                                if ui.button("➕ Insert Page Before").clicked() {
                                                    action_insert_before = Some(p_i);
                                                    ui.close_menu();
                                                }
                                                if ui.button("➕ Insert Page After").clicked() {
                                                    action_insert_after = Some(p_i);
                                                    ui.close_menu();
                                                }
                                                ui.separator();
                                                if ui.button("📐 Change Size...").clicked() {
                                                    action_change_size = Some(p_i);
                                                    ui.close_menu();
                                                }
                                            });

                                            ui.label(
                                                RichText::new(format!("{}", p_i + 1)).size(11.0).color(
                                                    if is_active {
                                                        Color32::from_rgb(76, 147, 236)
                                                    } else {
                                                        Color32::GRAY
                                                    },
                                                ),
                                            );
                                            ui.add_space(8.0);
                                        });
                                    }

                                    // Execute deferred context menu actions
                                    if let Some((from_i, to_i)) = action_move_drag
                                        && from_i != to_i
                                    {
                                        self.push_doc_undo(nb_idx);
                                        self.library.notebooks[nb_idx].move_page(from_i, to_i);
                                        self.navigate_to_page(nb_idx, to_i);
                                        self.mark_dirty(&notebook_id);
                                        self.toast_message = Some(format!(
                                            "📑 Halaman dipindahkan ke posisi {}.",
                                            to_i + 1
                                        ));
                                    }
                                    if pointer_released {
                                        self.dragged_thumb_page = None;
                                    }
                                    if let Some(idx) = action_rotate_cw {
                                        self.push_doc_undo(nb_idx);
                                        self.library.notebooks[nb_idx].rotate_page(idx, 90);
                                        self.mark_dirty(&notebook_id);
                                        self.toast_message =
                                            Some(format!("↻ Halaman {} diputar 90° CW.", idx + 1));
                                    }
                                    if let Some(idx) = action_rotate_ccw {
                                        self.push_doc_undo(nb_idx);
                                        self.library.notebooks[nb_idx].rotate_page(idx, -90);
                                        self.mark_dirty(&notebook_id);
                                        self.toast_message =
                                            Some(format!("↺ Halaman {} diputar 90° CCW.", idx + 1));
                                    }
                                    if let Some(idx) = action_rotate_180 {
                                        self.push_doc_undo(nb_idx);
                                        self.library.notebooks[nb_idx].rotate_page(idx, 180);
                                        self.mark_dirty(&notebook_id);
                                        self.toast_message =
                                            Some(format!("⟲ Halaman {} diputar 180°.", idx + 1));
                                    }
                                    if let Some(idx) = action_select {
                                        self.navigate_to_page(nb_idx, idx);
                                    }
                                    if let Some(idx) = action_duplicate {
                                        self.push_doc_undo(nb_idx);
                                        if let Some(new_idx) =
                                            self.library.notebooks[nb_idx].duplicate_page(idx)
                                        {
                                            self.navigate_to_page(nb_idx, new_idx);
                                            self.mark_dirty(&notebook_id);
                                        }
                                    }
                                    if let Some(idx) = action_delete {
                                        self.push_doc_undo(nb_idx);
                                        if let Ok(new_idx) = self.library.notebooks[nb_idx].delete_page(idx)
                                        {
                                            self.navigate_to_page(nb_idx, new_idx);
                                            self.mark_dirty(&notebook_id);
                                        }
                                    }
                                    if let Some(idx) = action_move_up {
                                        self.push_doc_undo(nb_idx);
                                        if self.library.notebooks[nb_idx].move_page(idx, idx - 1) {
                                            self.navigate_to_page(nb_idx, idx - 1);
                                            self.mark_dirty(&notebook_id);
                                        }
                                    }
                                    if let Some(idx) = action_move_down {
                                        self.push_doc_undo(nb_idx);
                                        if self.library.notebooks[nb_idx].move_page(idx, idx + 1) {
                                            self.navigate_to_page(nb_idx, idx + 1);
                                            self.mark_dirty(&notebook_id);
                                        }
                                    }
                                    if let Some(idx) = action_insert_before {
                                        let (w, h, tmpl) = {
                                            let p = &self.library.notebooks[nb_idx].pages[idx];
                                            (p.width, p.height, p.template)
                                        };
                                        self.push_doc_undo(nb_idx);
                                        self.library.notebooks[nb_idx].insert_page(idx, tmpl, w, h);
                                        self.navigate_to_page(nb_idx, idx);
                                        self.mark_dirty(&notebook_id);
                                    }
                                    if let Some(idx) = action_insert_after {
                                        let (w, h, tmpl) = {
                                            let p = &self.library.notebooks[nb_idx].pages[idx];
                                            (p.width, p.height, p.template)
                                        };
                                        self.push_doc_undo(nb_idx);
                                        self.library.notebooks[nb_idx].insert_page(idx + 1, tmpl, w, h);
                                        self.navigate_to_page(nb_idx, idx + 1);
                                        self.mark_dirty(&notebook_id);
                                    }
                                    if let Some(idx) = action_change_size {
                                        self.navigate_to_page(nb_idx, idx);
                                        self.open_page_resize_modal(nb_idx, idx);
                                    }
                                });
                        }
                        SidebarTab::Bookmarks => {
                            ui.label(RichText::new("Bookmarked Pages").strong().color(Color32::WHITE));
                            ui.separator();
                            ScrollArea::vertical()
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    let mut unbookmark_idx = None;
                                    let mut jump_idx = None;
                                    let pages = &self.library.notebooks[nb_idx].pages;
                                    let bookmarked_list: Vec<(usize, &Page)> = pages
                                        .iter()
                                        .enumerate()
                                        .filter(|(_, p)| p.bookmarked)
                                        .collect();

                                    if bookmarked_list.is_empty() {
                                        ui.add_space(20.0);
                                        ui.label(
                                            RichText::new("Tidak ada bookmark.\nKlik ikon 🔖 di bilah atas untuk menandai halaman.")
                                                .size(11.0)
                                                .color(Color32::GRAY),
                                        );
                                    } else {
                                        for (p_i, p) in bookmarked_list {
                                            let is_cur = p_i == self.current_page_index;
                                            ui.horizontal(|ui| {
                                                let btn_text = format!("🔖 Hal. {}", p_i + 1);
                                                let btn = Button::new(
                                                    RichText::new(btn_text)
                                                        .size(12.0)
                                                        .color(if is_cur {
                                                            Color32::from_rgb(76, 147, 236)
                                                        } else {
                                                            Color32::WHITE
                                                        }),
                                                )
                                                .fill(if is_cur {
                                                    Color32::from_rgb(39, 54, 91)
                                                } else {
                                                    Color32::TRANSPARENT
                                                });
                                                if ui.add(btn).clicked() {
                                                    jump_idx = Some(p_i);
                                                }
                                                if ui.small_button("✕").on_hover_text("Hapus bookmark").clicked() {
                                                    unbookmark_idx = Some(p_i);
                                                }
                                            });
                                            // Optional mini summary if page has text
                                            let first_text = p.objects.iter().find_map(|o| {
                                                if let crate::model::DrawObject::Text { text, .. } = o {
                                                    Some(text.as_str())
                                                } else if let crate::model::DrawObject::StickyNote { text, .. } = o {
                                                    Some(text.as_str())
                                                } else {
                                                    None
                                                }
                                            });
                                            if let Some(txt) = first_text {
                                                let preview: String = txt.chars().take(22).collect();
                                                ui.label(RichText::new(format!("  \"{preview}…\"")).size(10.0).color(Color32::DARK_GRAY));
                                            }
                                            ui.add_space(4.0);
                                        }
                                    }

                                    if let Some(j) = jump_idx {
                                        self.navigate_to_page(nb_idx, j);
                                    }
                                    if let Some(u) = unbookmark_idx {
                                        self.library.notebooks[nb_idx].pages[u].bookmarked = false;
                                        self.mark_dirty(&notebook_id);
                                    }
                                });
                        }
                        SidebarTab::Outline => {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("Daftar Isi").strong().color(Color32::WHITE));
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if ui.button("➕ Tambah").clicked() {
                                        self.new_outline_title = format!("Halaman {}", self.current_page_index + 1);
                                        self.show_add_outline_modal = true;
                                    }
                                });
                            });
                            ui.separator();
                            ScrollArea::vertical()
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    let mut delete_outline_idx = None;
                                    let mut jump_page = None;
                                    let outline = &self.library.notebooks[nb_idx].outline;

                                    if outline.is_empty() {
                                        ui.add_space(20.0);
                                        ui.label(
                                            RichText::new("Belum ada daftar isi.\nKlik '➕ Tambah' untuk menandai bab/topik di halaman ini.")
                                                .size(11.0)
                                                .color(Color32::GRAY),
                                        );
                                    } else {
                                        for (idx, item) in outline.iter().enumerate() {
                                            let is_cur = item.page_index == self.current_page_index;
                                            ui.horizontal(|ui| {
                                                let label_text = format!("{} (Hal. {})", item.title, item.page_index + 1);
                                                let btn = Button::new(
                                                    RichText::new(label_text)
                                                        .size(12.0)
                                                        .color(if is_cur {
                                                            Color32::from_rgb(76, 147, 236)
                                                        } else {
                                                            Color32::WHITE
                                                        }),
                                                )
                                                .fill(if is_cur {
                                                    Color32::from_rgb(39, 54, 91)
                                                } else {
                                                    Color32::TRANSPARENT
                                                });
                                                if ui.add(btn).clicked() {
                                                    jump_page = Some(item.page_index);
                                                }
                                                if ui.small_button("✕").on_hover_text("Hapus dari outline").clicked() {
                                                    delete_outline_idx = Some(idx);
                                                }
                                            });
                                            ui.add_space(4.0);
                                        }
                                    }

                                    if let Some(p) = jump_page
                                        && p < self.library.notebooks[nb_idx].pages.len()
                                    {
                                        self.navigate_to_page(nb_idx, p);
                                    }
                                    if let Some(d) = delete_outline_idx {
                                        self.library.notebooks[nb_idx].outline.remove(d);
                                        self.mark_dirty(&notebook_id);
                                    }
                                });
                        }
                    }
                });
        }

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

        // Modal Pengaturan Ukuran Kertas Spesifik & Per-Halaman + Template
        if self.show_page_resize_modal {
            let page_count = self.library.notebooks[nb_idx].pages.len();
            if self.resize_target_page_index >= page_count {
                self.resize_target_page_index = self.current_page_index.min(page_count - 1);
            }
            egui::Window::new("📐 Pengaturan Ukuran Kertas & Template Halaman")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.set_width(430.0);
                    ui.label(
                        RichText::new(
                            "Atur ukuran kertas spesifik (preset maupun angka bebas) serta template per halaman atau untuk seluruh dokumen.",
                        )
                        .size(11.5)
                        .color(Color32::GRAY),
                    );
                    ui.add_space(8.0);

                    // 1. Scope: Per-Page vs All Pages
                    ui.label(RichText::new("Cakupan Penerapan (Scope):").strong());
                    ui.horizontal(|ui| {
                        if ui
                            .selectable_label(
                                !self.resize_apply_all_pages,
                                format!(
                                    "📄 Halaman Spesifik (Hal. {} dari {})",
                                    self.resize_target_page_index + 1,
                                    page_count
                                ),
                            )
                            .clicked()
                        {
                            self.resize_apply_all_pages = false;
                        }
                        if ui
                            .selectable_label(
                                self.resize_apply_all_pages,
                                format!("📚 Semua Halaman ({page_count} Hal.)"),
                            )
                            .clicked()
                        {
                            self.resize_apply_all_pages = true;
                        }
                    });

                    // Jika mode per-halaman dan ada > 1 halaman, izinkan pilih halaman mana yang ingin diatur
                    if !self.resize_apply_all_pages && page_count > 1 {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("Pilih Halaman Target:")
                                    .size(11.5)
                                    .color(Color32::LIGHT_GRAY),
                            );
                            if self.resize_target_page_index > 0
                                && ui.small_button("◀").clicked()
                            {
                                let next_idx = self.resize_target_page_index - 1;
                                self.open_page_resize_modal(nb_idx, next_idx);
                            }
                            ui.label(
                                RichText::new(format!(
                                    "Halaman {}",
                                    self.resize_target_page_index + 1
                                ))
                                .strong(),
                            );
                            if self.resize_target_page_index + 1 < page_count
                                && ui.small_button("▶").clicked()
                            {
                                let next_idx = self.resize_target_page_index + 1;
                                self.open_page_resize_modal(nb_idx, next_idx);
                            }
                        });
                    }

                    ui.separator();

                    // 2. Preset & Custom Paper Size
                    ui.label(RichText::new("Pilih Preset atau Ukuran Spesifik:").strong());
                    ui.horizontal_wrapped(|ui| {
                        for preset in PaperSizePreset::ALL {
                            let is_sel = !self.resize_use_custom_size
                                && self.resize_target_preset == preset;
                            if ui.selectable_label(is_sel, preset.name()).clicked() {
                                self.resize_use_custom_size = false;
                                self.resize_target_preset = preset;
                                let (pw, ph) =
                                    preset.dimensions(self.resize_target_orientation);
                                self.resize_custom_width = pw;
                                self.resize_custom_height = ph;
                            }
                        }
                        let is_slide = self.resize_use_custom_size
                            && (self.resize_custom_width - 1280.0).abs() < 1.0
                            && (self.resize_custom_height - 720.0).abs() < 1.0;
                        if ui
                            .selectable_label(is_slide, "16:9 Slide (1280×720)")
                            .clicked()
                        {
                            self.resize_use_custom_size = true;
                            self.resize_custom_width = 1280.0;
                            self.resize_custom_height = 720.0;
                        }
                        let is_square = self.resize_use_custom_size
                            && (self.resize_custom_width - 800.0).abs() < 1.0
                            && (self.resize_custom_height - 800.0).abs() < 1.0;
                        if ui
                            .selectable_label(is_square, "Square (800×800)")
                            .clicked()
                        {
                            self.resize_use_custom_size = true;
                            self.resize_custom_width = 800.0;
                            self.resize_custom_height = 800.0;
                        }
                        if ui
                            .selectable_label(
                                self.resize_use_custom_size && !is_slide && !is_square,
                                "✏ Custom Spesifik",
                            )
                            .clicked()
                        {
                            self.resize_use_custom_size = true;
                        }
                    });

                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.label("Orientasi:");
                        if ui
                            .selectable_label(
                                !self.resize_use_custom_size
                                    && self.resize_target_orientation == PageOrientation::Portrait,
                                "Portrait",
                            )
                            .clicked()
                        {
                            self.resize_use_custom_size = false;
                            self.resize_target_orientation = PageOrientation::Portrait;
                            let (pw, ph) = self
                                .resize_target_preset
                                .dimensions(self.resize_target_orientation);
                            self.resize_custom_width = pw;
                            self.resize_custom_height = ph;
                        }
                        if ui
                            .selectable_label(
                                !self.resize_use_custom_size
                                    && self.resize_target_orientation == PageOrientation::Landscape,
                                "Landscape",
                            )
                            .clicked()
                        {
                            self.resize_use_custom_size = false;
                            self.resize_target_orientation = PageOrientation::Landscape;
                            let (pw, ph) = self
                                .resize_target_preset
                                .dimensions(self.resize_target_orientation);
                            self.resize_custom_width = pw;
                            self.resize_custom_height = ph;
                        }
                        if ui
                            .small_button("⇄ Tukar Lebar × Tinggi")
                            .clicked()
                        {
                            self.resize_use_custom_size = true;
                            std::mem::swap(
                                &mut self.resize_custom_width,
                                &mut self.resize_custom_height,
                            );
                        }
                    });

                    ui.add_space(6.0);
                    ui.label(RichText::new("Dimensi Kertas Spesifik (Width × Height):").strong());
                    ui.horizontal(|ui| {
                        ui.label("Lebar (Width):");
                        if ui
                            .add(
                                egui::DragValue::new(&mut self.resize_custom_width)
                                    .range(200.0..=4000.0)
                                    .speed(2.0)
                                    .suffix(" pt"),
                            )
                            .changed()
                        {
                            self.resize_use_custom_size = true;
                        }
                        ui.add_space(8.0);
                        ui.label("Tinggi (Height):");
                        if ui
                            .add(
                                egui::DragValue::new(&mut self.resize_custom_height)
                                    .range(200.0..=4000.0)
                                    .speed(2.0)
                                    .suffix(" pt"),
                            )
                            .changed()
                        {
                            self.resize_use_custom_size = true;
                        }
                    });

                    ui.separator();

                    // 3. Page Template Selector
                    ui.label(RichText::new("Template Pola Kertas:").strong());
                    ui.horizontal_wrapped(|ui| {
                        let tmpls = [
                            (PageTemplate::Ruled, "Ruled"),
                            (PageTemplate::Grid, "Grid"),
                            (PageTemplate::Dotted, "Dotted"),
                            (PageTemplate::Blank, "Blank"),
                            (PageTemplate::Cornell, "Cornell"),
                        ];
                        for (t, label) in tmpls {
                            if ui
                                .selectable_label(self.resize_target_template == t, label)
                                .clicked()
                            {
                                self.resize_target_template = t;
                            }
                        }
                    });

                    let (w, h) = if self.resize_use_custom_size {
                        (
                            self.resize_custom_width.clamp(200.0, 4000.0),
                            self.resize_custom_height.clamp(200.0, 4000.0),
                        )
                    } else {
                        self.resize_target_preset
                            .dimensions(self.resize_target_orientation)
                    };
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new(format!(
                            "Hasil Akhir: {:.0} × {:.0} pt ({:?})",
                            w, h, self.resize_target_template
                        ))
                        .size(12.0)
                        .color(Color32::from_rgb(130, 195, 255)),
                    );

                    ui.add_space(10.0);
                    ui.separator();
                    ui.add_space(6.0);

                    ui.horizontal(|ui| {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let apply_btn = Button::new(
                                RichText::new("Terapkan Perubahan")
                                    .strong()
                                    .color(Color32::WHITE),
                            )
                            .fill(Color32::from_rgb(0, 114, 206));
                            if ui.add(apply_btn).clicked() {
                                self.push_doc_undo(nb_idx);
                                if self.resize_apply_all_pages {
                                    let total = self.library.notebooks[nb_idx].pages.len();
                                    for idx in 0..total {
                                        self.library.notebooks[nb_idx].resize_page(idx, w, h);
                                        let p = &mut self.library.notebooks[nb_idx].pages[idx];
                                        p.template = self.resize_target_template;
                                        if matches!(
                                            p.background,
                                            Some(crate::model::PageBackground::Template(_))
                                        ) {
                                            p.background =
                                                Some(crate::model::PageBackground::Template(
                                                    self.resize_target_template,
                                                ));
                                        }
                                    }
                                } else {
                                    let target_idx = self.resize_target_page_index;
                                    self.library.notebooks[nb_idx].resize_page(target_idx, w, h);
                                    let p = &mut self.library.notebooks[nb_idx].pages[target_idx];
                                    p.template = self.resize_target_template;
                                    if matches!(
                                        p.background,
                                        Some(crate::model::PageBackground::Template(_))
                                    ) {
                                        p.background = Some(crate::model::PageBackground::Template(
                                            self.resize_target_template,
                                        ));
                                    }
                                }
                                self.mark_dirty(&notebook_id);
                                self.show_page_resize_modal = false;
                            }
                            if ui.button("Batal").clicked() {
                                self.show_page_resize_modal = false;
                            }
                        });
                    });
                });
        }

        // Add Page Modal
        self.render_add_page_modal(ctx, nb_idx);

        // Fase 5: In-Document Search (Ctrl+F)
        if self.show_in_doc_search {
            egui::Window::new("🔍 Cari di Dokumen")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::RIGHT_TOP, [-24.0, 96.0])
                .show(ctx, |ui| {
                    ui.set_width(280.0);
                    ui.horizontal(|ui| {
                        let query_resp = ui.text_edit_singleline(&mut self.doc_search_query);
                        if query_resp.lost_focus()
                            && ctx.input(|i| i.key_pressed(egui::Key::Escape))
                        {
                            self.show_in_doc_search = false;
                        }
                        if ui.button("✕").clicked() {
                            self.show_in_doc_search = false;
                        }
                    });

                    let q = self.doc_search_query.trim().to_lowercase();
                    if !q.is_empty() {
                        // Find all matches across all pages in active notebook (including Fase 8.6 OCR text)
                        let mut matches: Vec<(usize, String)> = Vec::new();
                        for (p_i, page) in self.library.notebooks[nb_idx].pages.iter().enumerate() {
                            if !page.ocr_text.is_empty()
                                && page.ocr_text.to_lowercase().contains(&q)
                            {
                                matches.push((p_i, format!("✍ OCR: {}", page.ocr_text)));
                            }
                            for obj in &page.objects {
                                match obj {
                                    crate::model::DrawObject::Text { text, .. }
                                    | crate::model::DrawObject::StickyNote { text, .. }
                                        if text.to_lowercase().contains(&q) =>
                                    {
                                        matches.push((p_i, text.clone()));
                                    }
                                    _ => {}
                                }
                            }
                        }

                        if matches.is_empty() {
                            ui.label(
                                RichText::new("Tidak ditemukan kecocokan.")
                                    .size(11.0)
                                    .color(Color32::GRAY),
                            );
                        } else {
                            if self.doc_search_current_match >= matches.len() {
                                self.doc_search_current_match = 0;
                            }
                            let cur_match = self.doc_search_current_match;

                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(format!(
                                        "{}/{} hasil",
                                        cur_match + 1,
                                        matches.len()
                                    ))
                                    .size(11.0)
                                    .color(Color32::LIGHT_GRAY),
                                );
                                if ui.small_button("◀").clicked() {
                                    if cur_match == 0 {
                                        self.doc_search_current_match = matches.len() - 1;
                                    } else {
                                        self.doc_search_current_match -= 1;
                                    }
                                    let target_p = matches[self.doc_search_current_match].0;
                                    self.navigate_to_page(nb_idx, target_p);
                                }
                                if ui.small_button("▶").clicked() {
                                    self.doc_search_current_match = (cur_match + 1) % matches.len();
                                    let target_p = matches[self.doc_search_current_match].0;
                                    self.navigate_to_page(nb_idx, target_p);
                                }
                            });

                            let target_page = matches[cur_match].0;
                            let snippet = &matches[cur_match].1;
                            let snippet_preview: String = snippet.chars().take(28).collect();
                            ui.label(
                                RichText::new(format!(
                                    "Hal. {}: \"{}…\"",
                                    target_page + 1,
                                    snippet_preview
                                ))
                                .size(10.0)
                                .color(Color32::from_rgb(76, 147, 236)),
                            );
                        }
                    } else {
                        ui.label(
                            RichText::new("Ketik kata untuk mencari teks...")
                                .size(10.0)
                                .color(Color32::GRAY),
                        );
                    }
                });
        }

        // Fase 5: Add Outline Item Modal Dialog
        if self.show_add_outline_modal {
            egui::Window::new("📑 Tambah ke Daftar Isi (Outline)")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.set_width(320.0);
                    ui.label(RichText::new("Tambahkan Bab / Judul pada Halaman Aktif").strong());
                    ui.label(
                        RichText::new(format!("Halaman saat ini: {}", self.current_page_index + 1))
                            .size(11.0)
                            .color(Color32::GRAY),
                    );
                    ui.add_space(8.0);
                    ui.label("Judul / Topik:");
                    ui.text_edit_singleline(&mut self.new_outline_title);
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.button("Simpan").clicked()
                                && !self.new_outline_title.trim().is_empty()
                            {
                                let item = OutlineItem {
                                    id: uuid::Uuid::new_v4().to_string(),
                                    title: self.new_outline_title.trim().to_string(),
                                    page_index: self.current_page_index,
                                };
                                self.library.notebooks[nb_idx].outline.push(item);
                                self.mark_dirty(&notebook_id);
                                self.show_add_outline_modal = false;
                                self.new_outline_title.clear();
                            }
                            if ui.button("Batal").clicked() {
                                self.show_add_outline_modal = false;
                            }
                        });
                    });
                });
        }

        // Fase 5: Save Page as Custom Template Modal Dialog
        if self.show_save_template_modal {
            egui::Window::new("💾 Simpan Halaman sebagai Template Kustom")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.set_width(340.0);
                    ui.label(RichText::new("Simpan Format & Latar Halaman Ini").strong());
                    ui.label(
                        RichText::new("Template yang disimpan dapat digunakan kembali saat membuat buku catatan baru.")
                            .size(11.0)
                            .color(Color32::GRAY),
                    );
                    ui.add_space(8.0);
                    ui.label("Nama Template:");
                    ui.text_edit_singleline(&mut self.new_template_name);
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.button("Simpan Template").clicked() && !self.new_template_name.trim().is_empty() {
                                let cur_p = &self.library.notebooks[nb_idx].pages[self.current_page_index];
                                let tmpl = CustomTemplate {
                                    id: uuid::Uuid::new_v4().to_string(),
                                    name: self.new_template_name.trim().to_string(),
                                    template: cur_p.template,
                                    background: cur_p.background.clone(),
                                    paper_color: cur_p.paper_color.clone(),
                                    width: cur_p.width,
                                    height: cur_p.height,
                                };
                                let storage_dir = app_storage_dir();
                                if let Ok(()) = save_custom_template(&storage_dir, &tmpl) {
                                    self.custom_templates.push(tmpl);
                                    self.toast_message = Some("✅ Template kustom berhasil disimpan!".to_string());
                                }
                                self.show_save_template_modal = false;
                                self.new_template_name.clear();
                            }
                            if ui.button("Batal").clicked() {
                                self.show_save_template_modal = false;
                            }
                        });
                    });
                });
        }
        if self.show_shortcut_modal {
            egui::Window::new("⌨ Pintasan Keyboard & Gestur (Shortcuts)")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.set_width(440.0);
                    ui.label(
                        RichText::new("Pintasan Keyboard, Alat & Gestur Kanvas")
                            .strong()
                            .size(15.0),
                    );
                    ui.add_space(8.0);

                    egui::Grid::new("shortcut_table")
                        .striped(true)
                        .spacing([24.0, 6.0])
                        .show(ui, |ui| {
                            ui.strong("Pintasan");
                            ui.strong("Fungsi");
                            ui.end_row();

                            ui.label("P");
                            ui.label("Alat Pena (Pen)");
                            ui.end_row();

                            ui.label("H");
                            ui.label("Alat Penstabilo (Highlighter)");
                            ui.end_row();

                            ui.label("E");
                            ui.label("Alat Penghapus (Eraser)");
                            ui.end_row();

                            ui.label("L");
                            ui.label("Alat Lasso / Seleksi");
                            ui.end_row();

                            ui.label("T");
                            ui.label("Alat Teks");
                            ui.end_row();

                            ui.label("S / N");
                            ui.label("Alat Bentuk (Shape) / Catatan Tempel (Sticky Note)");
                            ui.end_row();

                            ui.label("Ctrl + S");
                            ui.label("Simpan Dokumen Segera (Force Save)");
                            ui.end_row();

                            ui.label("Ctrl + 0");
                            ui.label("Reset Zoom ke 100%");
                            ui.end_row();

                            ui.label("Ctrl + Scroll / Pinch");
                            ui.label("Zoom In / Out Sesuai Posisi Kursor Mouse");
                            ui.end_row();

                            ui.label("Arrow Keys (↑ ↓ ← →)");
                            ui.label("Geser / Pan Kanvas ke 4 Arah (Shift + Arrow: Cepat)");
                            ui.end_row();

                            ui.label("Ctrl + A");
                            ui.label("Pilih Semua Objek Halaman");
                            ui.end_row();

                            ui.label("Ctrl + C / X / V / D");
                            ui.label("Salin / Potong / Tempel / Gandakan Objek");
                            ui.end_row();

                            ui.label("Delete / Backspace");
                            ui.label("Hapus Objek Terpilih");
                            ui.end_row();

                            ui.label("Ctrl + Z / Ctrl + Y");
                            ui.label("Undo / Redo");
                            ui.end_row();

                            ui.label("Shift + Drag Handle");
                            ui.label("Skala Proporsional (Kunci Rasio Aspek)");
                            ui.end_row();

                            ui.label("Drag Handle Atas ⟳");
                            ui.label("Rotasi Bebas Objek Terpilih");
                            ui.end_row();

                            ui.label("Klik Kanan Kanvas");
                            ui.label("Menu Konteks (Balik H/V, Pindah Depan/Belakang)");
                            ui.end_row();

                            ui.label("PageUp / PageDown");
                            ui.label("Navigasi Halaman Sebelumnya / Berikutnya");
                            ui.end_row();

                            ui.label("Ctrl + Home / End");
                            ui.label("Lompat ke Halaman Pertama / Terakhir");
                            ui.end_row();

                            ui.label("F6 / Ctrl + B");
                            ui.label("Tampilkan / Sembunyikan Panel Halaman");
                            ui.end_row();

                            ui.label("Alt + Z");
                            ui.label("Buka / Tutup Zoom Window");
                            ui.end_row();
                        });

                    ui.add_space(14.0);
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button("Tutup (Esc)").clicked() {
                            self.show_shortcut_modal = false;
                        }
                    });
                });
        }

        // Fase 8.6: Handwriting OCR Inspector & Search Index Modal
        if self.show_ocr_modal {
            egui::Window::new(lang.tr(
                "🔤 Ekstraksi OCR Tulisan Tangan",
                "🔤 Handwriting OCR Extraction",
            ))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.set_width(380.0);
                ui.label(
                    RichText::new(lang.tr(
                        "Teks Tulisan Tangan Terindeks (Halaman Aktif)",
                        "Indexed Handwriting Text (Current Page)",
                    ))
                    .strong(),
                );
                ui.label(
                    RichText::new(lang.tr(
                        "Teks ini diekstrak secara otomatis dari goresan pena dan diindeks untuk pencarian global & Ctrl+F. Anda juga dapat menyuntingnya secara langsung.",
                        "This text is automatically extracted from pen strokes and indexed for global & Ctrl+F search. You can also edit it directly.",
                    ))
                    .size(11.0)
                    .color(Color32::GRAY),
                );
                ui.add_space(8.0);

                let cur_idx = self.current_page_index;
                let mut ocr_changed = false;
                {
                    let p = &mut self.library.notebooks[nb_idx].pages[cur_idx];
                    let resp = ui.add(
                        egui::TextEdit::multiline(&mut p.ocr_text)
                            .desired_rows(4)
                            .desired_width(f32::INFINITY)
                            .hint_text(lang.tr(
                                "Belum ada goresan tulisan tangan yang dikenali...",
                                "No handwriting strokes recognized yet...",
                            )),
                    );
                    if resp.changed() {
                        ocr_changed = true;
                    }
                }
                if ocr_changed {
                    self.mark_dirty(&notebook_id);
                }

                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui
                        .button(lang.tr("🔄 Pindai Ulang Goresan", "🔄 Rescan Strokes"))
                        .clicked()
                    {
                        let p = &mut self.library.notebooks[nb_idx].pages[cur_idx];
                        p.ocr_text = crate::geometry::recognize_handwriting_from_strokes(&p.objects);
                        self.mark_dirty(&notebook_id);
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button(lang.tr("Tutup", "Close")).clicked() {
                            self.show_ocr_modal = false;
                        }
                    });
                });
            });
        }

        // Fase 7: About Dialog Modal
        if self.show_about_modal {
            egui::Window::new("ℹ️ Tentang RustNotes")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.set_width(380.0);
                    ui.vertical_centered(|ui| {
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new("RustNotes")
                                .size(22.0)
                                .strong()
                                .color(Color32::from_rgb(0, 114, 206)),
                        );
                        ui.label(
                            RichText::new(format!("Versi {}", env!("CARGO_PKG_VERSION")))
                                .size(12.0)
                                .color(Color32::GRAY),
                        );
                        ui.add_space(6.0);
                        ui.label(
                            RichText::new(
                                "Aplikasi Buku Catatan Digital 100% Pure Rust\nGPU Hardware-Accelerated (eframe/egui)",
                            )
                            .size(12.0)
                            .color(Color32::WHITE),
                        );
                    });
                    ui.add_space(12.0);
                    ui.separator();
                    ui.add_space(8.0);

                    egui::Grid::new("about_grid")
                        .spacing([16.0, 6.0])
                        .show(ui, |ui| {
                            ui.label(RichText::new("Format Berkas:").strong());
                            ui.label(".rnote (Zip Archive + JSON + Assets)");
                            ui.end_row();

                            ui.label(RichText::new("Mesin Inking:").strong());
                            ui.label("Bézier Ribbon Mesh + Spatial Grid Index");
                            ui.end_row();

                            ui.label(RichText::new("Penyimpanan:").strong());
                            ui.label("Atomic JSON (%APPDATA%\\RustNotes)");
                            ui.end_row();

                            ui.label(RichText::new("Target OS:").strong());
                            ui.label("Windows 10 / 11 64-bit (Native)");
                            ui.end_row();

                            ui.label(RichText::new("Lisensi:").strong());
                            ui.label("MIT / Apache-2.0 (Zero Electron/Node)");
                            ui.end_row();
                        });

                    ui.add_space(14.0);
                    ui.separator();
                    ui.add_space(8.0);
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button("Tutup").clicked() {
                            self.show_about_modal = false;
                        }
                    });
                });
        }
    }
}

// Function to draw realistic Goodnotes book covers
fn draw_goodnotes_cover(
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
    let icon_bytes = include_bytes!("../assets/icon.png");
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

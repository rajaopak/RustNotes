use super::types::{
    AddPagePosition, COVER_PALETTE, NavView, ShortcutModalTab, SidebarTab, ViewMode,
    get_windows_clipboard_sequence,
};
use crate::canvas::CanvasState;
use crate::model::{
    AppSettings, CustomTemplate, Library, Page, PageOrientation, PageTemplate, PaperSizePreset,
};
use crate::storage::{
    LoadStatus, app_storage_dir, load_custom_templates, load_library_from_dir,
    load_library_index_from_dir, load_notebook_from_dir, load_settings_from_dir,
    save_library_index_to_dir,
};
use eframe::egui::{Color32, Rect, Vec2};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::time::Instant;

pub struct RustNotesApp {
    pub(crate) library: Library,
    pub(crate) open_notebook_ids: Vec<String>,
    pub(crate) active_notebook_id: Option<String>,
    pub(crate) current_page_index: usize,
    pub(crate) canvas_state: CanvasState,
    pub(crate) load_status: LoadStatus,
    pub(crate) is_read_only: bool,
    pub(crate) toast_message: Option<String>,
    pub(crate) unloaded_notebook_metas: Vec<crate::model::NotebookMeta>,

    // Library UI state
    pub(crate) current_nav_view: NavView,
    pub(crate) search_query: String,
    pub(crate) show_new_notebook_modal: bool,
    pub(crate) new_notebook_title: String,
    pub(crate) new_notebook_template: PageTemplate,
    pub(crate) new_notebook_cover: Color32,
    #[allow(dead_code)]
    pub(crate) new_notebook_paper_color: Color32,
    pub(crate) new_notebook_preset: PaperSizePreset,
    pub(crate) new_notebook_orientation: PageOrientation,
    pub(crate) new_notebook_use_custom_size: bool,
    pub(crate) new_notebook_custom_width: f32,
    pub(crate) new_notebook_custom_height: f32,

    // Fase 2: Page Management & View Mode
    pub(crate) view_mode: ViewMode,
    pub(crate) show_thumbnail_sidebar: bool,
    pub(crate) show_page_resize_modal: bool,
    pub(crate) resize_target_page_index: usize,
    pub(crate) resize_target_preset: PaperSizePreset,
    pub(crate) resize_target_orientation: PageOrientation,
    pub(crate) resize_use_custom_size: bool,
    pub(crate) resize_custom_width: f32,
    pub(crate) resize_custom_height: f32,
    pub(crate) resize_target_template: PageTemplate,
    pub(crate) resize_apply_all_pages: bool,
    pub(crate) doc_undo_stacks: HashMap<String, Vec<(Vec<Page>, usize)>>,
    pub(crate) doc_redo_stacks: HashMap<String, Vec<(Vec<Page>, usize)>>,

    // GoodNotes-style Pop-ups & Viewport Camera State
    pub(crate) show_more_menu_popup: bool,
    pub(crate) show_tool_popup: bool,
    pub(crate) show_doc_title_popup: bool,
    pub(crate) show_zoom_slider_expanded: bool,
    pub(crate) canvas_scroll_offset: Vec2,
    pub(crate) pending_scroll_offset: Option<Vec2>,
    pub(crate) canvas_pan_offset: Vec2,
    pub(crate) last_canvas_viewport_rect: Rect,

    // Fase 6: Dirty flag + debounced per-notebook save
    pub(crate) dirty_notebooks: HashSet<String>,
    pub(crate) last_modified: HashMap<String, Instant>,

    // Fase 4: Shortcut cheat sheet modal & custom shortcut settings
    pub(crate) show_shortcut_modal: bool,
    pub(crate) shortcut_modal_tab: ShortcutModalTab,
    pub(crate) rebinding_shortcut: Option<crate::model::ShortcutAction>,
    pub(crate) shortcut_search_query: String,

    // Fase 5: Nested Folders, Outline/Bookmarks, Search, Custom Templates
    pub(crate) current_folder_id: Option<String>,
    pub(crate) show_new_folder_modal: bool,
    pub(crate) new_folder_name: String,
    pub(crate) new_folder_color: Color32,
    pub(crate) moving_notebook_id: Option<String>,

    pub(crate) sidebar_tab: SidebarTab,
    pub(crate) show_add_outline_modal: bool,
    pub(crate) new_outline_title: String,

    pub(crate) show_in_doc_search: bool,
    pub(crate) doc_search_query: String,
    pub(crate) doc_search_current_match: usize,

    pub(crate) custom_templates: Vec<CustomTemplate>,
    pub(crate) show_save_template_modal: bool,
    pub(crate) new_template_name: String,

    // Fase 7: About dialog modal
    pub(crate) show_about_modal: bool,
    pub(crate) confirm_delete_notebook: Option<(String, String)>,
    pub(crate) confirm_empty_trash: bool,

    // Fase 8.3, 8.4, 8.5, 8.7, 8.8: User settings
    pub(crate) settings: AppSettings,

    // Fase 8.6: Handwriting OCR inspector modal
    pub(crate) show_ocr_modal: bool,

    // Add Page Modal
    pub(crate) show_add_page_modal: bool,
    pub(crate) add_page_template: PageTemplate,
    pub(crate) add_page_preset: PaperSizePreset,
    pub(crate) add_page_orientation: PageOrientation,
    pub(crate) add_page_target_pos: AddPagePosition,
    pub(crate) add_page_use_custom_size: bool,
    pub(crate) add_page_custom_width: f32,
    pub(crate) add_page_custom_height: f32,
    pub(crate) add_page_paper_color: Color32,

    // Memori template terakhir kali digunakan / dibuat
    pub(crate) last_created_template: PageTemplate,
    pub(crate) last_created_preset: PaperSizePreset,
    pub(crate) last_created_orientation: PageOrientation,
    pub(crate) last_created_use_custom: bool,
    pub(crate) last_created_width: f32,
    pub(crate) last_created_height: f32,
    pub(crate) last_created_paper_color: Color32,

    // Thumbnail Multi-Select & Drag-Drop Reorder
    pub(crate) thumb_selected_pages: BTreeSet<usize>,
    pub(crate) thumb_select_mode: bool,
    pub(crate) dragged_thumb_page: Option<usize>,

    // Windows Clipboard Sequence & Timestamp Tracking
    pub(crate) win_clipboard_seq: u32,
    pub(crate) win_clipboard_timestamp: Instant,
    pub(crate) last_v_key_down: bool,
    pub(crate) app_clipboard_win_seq: u32,
}

impl Default for RustNotesApp {
    fn default() -> Self {
        Self::load_from_dir(&app_storage_dir())
    }
}

impl RustNotesApp {
    pub(crate) fn load_from_dir(storage_dir: &std::path::Path) -> Self {
        let index_res = load_library_index_from_dir(storage_dir);

        let mut library = Library::default();
        let mut load_errors: Vec<String> = Vec::new();
        let mut unloaded_notebook_metas: Vec<crate::model::NotebookMeta> = Vec::new();
        for meta in &index_res.index.notebooks {
            match load_notebook_from_dir(storage_dir, &meta.id) {
                Ok(nb) => library.notebooks.push(nb),
                Err(e) => {
                    load_errors.push(e);
                    unloaded_notebook_metas.push(meta.clone());
                }
            }
        }
        library.folders = index_res.index.folders;

        let mut actual_load_status = match index_res.status {
            crate::storage::LoadIndexStatus::RecoveredFromPrevious => {
                LoadStatus::RecoveredFromPrevious
            }
            _ => LoadStatus::LoadedDirect,
        };

        let (is_read_only, toast) = if !load_errors.is_empty() {
            (
                false,
                Some(format!(
                    "Peringatan: {} notebook gagal dimuat (metadata dipertahankan).",
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
                    let legacy_res = {
                        if library.notebooks.is_empty()
                            && index_res.status == crate::storage::LoadIndexStatus::FreshIndex
                        {
                            Some(load_library_from_dir(storage_dir))
                        } else {
                            None
                        }
                    };
                    if let Some(lr) = legacy_res {
                        actual_load_status = lr.status.clone();
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

        // Jangan menimpa library_index.json saat startup jika ada error muat atau mode baca-saja,
        // atau jika indeks sudah konsisten tanpa perubahan.
        if !is_read_only && load_errors.is_empty() && !library.notebooks.is_empty() {
            let mut init_index = crate::model::LibraryIndex::default();
            for nb in &library.notebooks {
                init_index
                    .notebooks
                    .push(crate::model::NotebookMeta::from_notebook(nb));
            }
            init_index.folders = library.folders.clone();
            let _ = save_library_index_to_dir(storage_dir, &init_index);
        }

        Self {
            library,
            open_notebook_ids: Vec::new(),
            active_notebook_id: None,
            current_page_index: 0,
            canvas_state: CanvasState::default(),
            load_status: actual_load_status,
            is_read_only,
            toast_message: toast,
            unloaded_notebook_metas,
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
            shortcut_modal_tab: ShortcutModalTab::default(),
            rebinding_shortcut: None,
            shortcut_search_query: String::new(),
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
            custom_templates: load_custom_templates(storage_dir),
            show_save_template_modal: false,
            new_template_name: String::new(),
            show_about_modal: false,
            confirm_delete_notebook: None,
            confirm_empty_trash: false,
            settings: load_settings_from_dir(storage_dir),
            show_ocr_modal: false,
            show_add_page_modal: false,
            add_page_template: PageTemplate::Ruled,
            add_page_preset: PaperSizePreset::A4,
            add_page_orientation: PageOrientation::Portrait,
            add_page_target_pos: AddPagePosition::AfterCurrent,
            add_page_use_custom_size: false,
            add_page_custom_width: 595.0,
            add_page_custom_height: 842.0,
            add_page_paper_color: Color32::from_rgb(255, 254, 250),
            last_created_template: PageTemplate::Ruled,
            last_created_preset: PaperSizePreset::A4,
            last_created_orientation: PageOrientation::Portrait,
            last_created_use_custom: false,
            last_created_width: 595.0,
            last_created_height: 842.0,
            last_created_paper_color: Color32::from_rgb(255, 254, 250),
            thumb_selected_pages: BTreeSet::new(),
            thumb_select_mode: false,
            dragged_thumb_page: None,
            win_clipboard_seq: get_windows_clipboard_sequence(),
            win_clipboard_timestamp: Instant::now(),
            last_v_key_down: false,
            app_clipboard_win_seq: 0,
        }
    }
}

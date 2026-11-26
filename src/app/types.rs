use eframe::egui::Color32;

#[cfg(target_os = "windows")]
unsafe extern "system" {
    fn GetClipboardSequenceNumber() -> u32;
    fn GetAsyncKeyState(vKey: i32) -> i16;
    fn GetForegroundWindow() -> *mut std::ffi::c_void;
    fn GetCurrentProcessId() -> u32;
    fn GetWindowThreadProcessId(hWnd: *mut std::ffi::c_void, lpdwProcessId: *mut u32) -> u32;
}

pub fn get_windows_clipboard_sequence() -> u32 {
    #[cfg(target_os = "windows")]
    unsafe {
        GetClipboardSequenceNumber()
    }
    #[cfg(not(target_os = "windows"))]
    0
}

pub fn is_v_key_physically_pressed() -> bool {
    #[cfg(target_os = "windows")]
    unsafe {
        let fg = GetForegroundWindow();
        if fg.is_null() {
            return false;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(fg, &mut pid);
        if pid != GetCurrentProcessId() {
            return false;
        }
        const VK_V: i32 = 0x56;
        (GetAsyncKeyState(VK_V) as u16 & 0x8000) != 0
    }
    #[cfg(not(target_os = "windows"))]
    false
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AddPagePosition {
    AfterCurrent,
    BeforeCurrent,
    AtEnd,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavView {
    Recent,
    Documents,
    Favorites,
    Trash,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum LibrarySortBy {
    #[default]
    LastModified,
    DateCreated,
    NameAsc,
    NameDesc,
    PageCount,
}

impl LibrarySortBy {
    pub const ALL: [Self; 5] = [
        Self::LastModified,
        Self::DateCreated,
        Self::NameAsc,
        Self::NameDesc,
        Self::PageCount,
    ];

    pub fn label(self, lang: crate::model::AppLanguage) -> &'static str {
        match (self, lang) {
            (Self::LastModified, crate::model::AppLanguage::Indonesian) => "Terakhir Diubah",
            (Self::LastModified, crate::model::AppLanguage::English) => "Last Modified",
            (Self::DateCreated, crate::model::AppLanguage::Indonesian) => "Tanggal Dibuat",
            (Self::DateCreated, crate::model::AppLanguage::English) => "Date Created",
            (Self::NameAsc, _) => "Nama (A–Z)",
            (Self::NameDesc, _) => "Nama (Z–A)",
            (Self::PageCount, crate::model::AppLanguage::Indonesian) => "Jumlah Halaman",
            (Self::PageCount, crate::model::AppLanguage::English) => "Page Count",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum LibraryViewStyle {
    #[default]
    Grid,
    List,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum LibraryFilter {
    #[default]
    All,
    PdfOnly,
    FavoritesOnly,
}

impl LibraryFilter {
    pub const ALL: [Self; 3] = [Self::All, Self::PdfOnly, Self::FavoritesOnly];

    pub fn label(self, lang: crate::model::AppLanguage) -> &'static str {
        match (self, lang) {
            (Self::All, crate::model::AppLanguage::Indonesian) => "Semua",
            (Self::All, crate::model::AppLanguage::English) => "All",
            (Self::PdfOnly, crate::model::AppLanguage::Indonesian) => "PDF",
            (Self::PdfOnly, crate::model::AppLanguage::English) => "PDF",
            (Self::FavoritesOnly, crate::model::AppLanguage::Indonesian) => "Favorit",
            (Self::FavoritesOnly, crate::model::AppLanguage::English) => "Favorites",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LibraryDragPayload {
    Notebooks(Vec<String>),
    Folder(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ShortcutModalTab {
    #[default]
    Customize,
    CheatSheet,
    Inking,
}

pub const COVER_PALETTE: [Color32; 8] = [
    Color32::from_rgb(189, 173, 150), // Sand/Tan
    Color32::from_rgb(49, 85, 119),   // Classic Navy
    Color32::from_rgb(112, 153, 141), // Sage Green
    Color32::from_rgb(197, 131, 120), // Dusty Rose
    Color32::from_rgb(198, 167, 95),  // Mustard Yellow
    Color32::from_rgb(112, 102, 140), // Purple Slate
    Color32::from_rgb(52, 57, 71),    // Charcoal Gray
    Color32::from_rgb(185, 200, 214), // Sky Blue
];

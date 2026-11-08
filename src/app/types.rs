use eframe::egui::Color32;

#[cfg(target_os = "windows")]
unsafe extern "system" {
    fn GetClipboardSequenceNumber() -> u32;
}

pub fn get_windows_clipboard_sequence() -> u32 {
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AddPagePosition {
    AfterCurrent,
    BeforeCurrent,
    AtEnd,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavView {
    Documents,
    Favorites,
    Trash,
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

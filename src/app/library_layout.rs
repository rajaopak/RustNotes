use super::types::LibrarySortBy;
use crate::model::{AppLanguage, Folder, Notebook, PageBackground};
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResponsiveGridMetrics {
    pub cols: usize,
    pub card_width: f32,
    pub card_height: f32,
}

/// Menghitung jumlah kolom dan ukuran kartu grid secara matematis berdasarkan lebar area tersedia
/// sehingga deretan kartu tidak pernah bertabrakan atau meluber keluar batas jendela pada ukuran mana pun.
pub fn compute_responsive_grid(
    available_width: f32,
    pad_x: f32,
    gap_x: f32,
    min_card_w: f32,
    target_card_w: f32,
    max_card_w: f32,
    aspect_h_over_w: f32,
) -> ResponsiveGridMetrics {
    let inner_w = (available_width - pad_x * 2.0).max(84.0);
    let effective_min = min_card_w.min(inner_w);
    let mut cols = ((inner_w + gap_x) / (target_card_w + gap_x))
        .floor()
        .max(1.0) as usize;

    while cols > 1 {
        let row_w = (inner_w - gap_x * (cols.saturating_sub(1) as f32)) / (cols as f32);
        if row_w >= effective_min {
            break;
        }
        cols -= 1;
    }

    let raw_card_w = (inner_w - gap_x * (cols.saturating_sub(1) as f32)) / (cols as f32);
    let card_width = if cols == 1 {
        raw_card_w.min(max_card_w).max(effective_min)
    } else {
        raw_card_w.clamp(effective_min, max_card_w)
    };
    let card_height = (card_width * aspect_h_over_w).round();

    ResponsiveGridMetrics {
        cols,
        card_width,
        card_height,
    }
}

/// Menghitung jumlah kolom dan lebar kartu folder responsif sesuai lebar panel utama.
pub fn compute_folder_grid(available_width: f32, pad_x: f32, gap_x: f32) -> (usize, f32) {
    let metrics = compute_responsive_grid(available_width, pad_x, gap_x, 140.0, 176.0, 240.0, 0.42);
    (metrics.cols, metrics.card_width)
}

/// Memeriksa apakah folder `source_id` aman dipindahkan ke dalam `dest_parent`
/// (mencegah folder dipindah ke dalam dirinya sendiri atau turunannya / siklus).
pub fn can_move_folder_into(
    folders: &[Folder],
    source_id: &str,
    dest_parent: Option<&str>,
) -> bool {
    let Some(mut cur_id) = dest_parent else {
        return true;
    };
    let mut visited = HashSet::new();
    for _ in 0..=folders.len() {
        if cur_id == source_id {
            return false;
        }
        if !visited.insert(cur_id) {
            return false;
        }
        if let Some(folder) = folders.iter().find(|f| f.id == cur_id) {
            match folder.parent_id.as_deref() {
                Some(parent) => cur_id = parent,
                None => return true,
            }
        } else {
            return true;
        }
    }
    false
}

/// Mengembalikan rantai breadcrumb terurut dari folder teratas hingga `folder_id` saat ini.
pub fn folder_breadcrumb_chain(
    folders: &[Folder],
    folder_id: Option<&str>,
) -> Vec<(String, String)> {
    let mut chain = Vec::new();
    let mut curr = folder_id.map(str::to_string);
    let mut visited = HashSet::new();
    while let Some(c_id) = curr {
        if !visited.insert(c_id.clone()) {
            break;
        }
        if let Some(f) = folders.iter().find(|f| f.id == c_id) {
            chain.push((f.id.clone(), f.name.clone()));
            curr = f.parent_id.clone();
        } else {
            break;
        }
    }
    chain.reverse();
    chain
}

/// Mengembalikan jalur direktori lengkap dalam bentuk string (mis. `"All Documents / Kuliah / Kalkulus"`).
pub fn folder_path_label(folders: &[Folder], folder_id: Option<&str>, root_label: &str) -> String {
    let chain = folder_breadcrumb_chain(folders, folder_id);
    if chain.is_empty() {
        return root_label.to_string();
    }
    let mut out = String::from(root_label);
    for (_, name) in chain {
        out.push_str(" / ");
        out.push_str(&name);
    }
    out
}

/// Mengembalikan `true` apabila notebook mengandung minimal satu halaman berlatar PDF.
pub fn notebook_has_pdf_pages(nb: &Notebook) -> bool {
    nb.pages
        .iter()
        .any(|p| matches!(p.effective_background(), PageBackground::Pdf { .. }))
}

/// Mengurutkan daftar item notebook `(indeks_asli, snippet_opsional)` sesuai mode `LibrarySortBy`.
pub fn sort_notebook_items(
    notebooks: &[Notebook],
    items: &mut [(usize, Option<String>)],
    sort: LibrarySortBy,
) {
    items.sort_by(|(idx_a, _), (idx_b, _)| {
        let a = &notebooks[*idx_a];
        let b = &notebooks[*idx_b];
        let ord = match sort {
            LibrarySortBy::LastModified => b.updated_at.cmp(&a.updated_at),
            LibrarySortBy::DateCreated => b.created_at.cmp(&a.created_at),
            LibrarySortBy::NameAsc => a.title.to_lowercase().cmp(&b.title.to_lowercase()),
            LibrarySortBy::NameDesc => b.title.to_lowercase().cmp(&a.title.to_lowercase()),
            LibrarySortBy::PageCount => b.pages.len().cmp(&a.pages.len()),
        };
        ord.then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
            .then_with(|| a.id.cmp(&b.id))
    });
}

/// Memformat selisih waktu ke label relatif ringkas sesuai bahasa aktif (`ID` / `EN`).
pub fn format_relative_time(timestamp_ms: i64, now_ms: i64, lang: AppLanguage) -> String {
    if timestamp_ms <= 0 {
        return lang.tr("Baru saja", "Just now").to_string();
    }
    let diff_secs = ((now_ms - timestamp_ms) / 1000).max(0);
    if diff_secs < 60 {
        return lang.tr("Baru saja", "Just now").to_string();
    }
    let mins = diff_secs / 60;
    if mins < 60 {
        return match lang {
            AppLanguage::Indonesian => format!("{mins}m lalu"),
            AppLanguage::English => format!("{mins}m ago"),
        };
    }
    let hours = mins / 60;
    if hours < 24 {
        return match lang {
            AppLanguage::Indonesian => format!("{hours}j lalu"),
            AppLanguage::English => format!("{hours}h ago"),
        };
    }
    let days = hours / 24;
    if days == 1 {
        return lang.tr("Kemarin", "Yesterday").to_string();
    }
    if days < 7 {
        return match lang {
            AppLanguage::Indonesian => format!("{days}h lalu"),
            AppLanguage::English => format!("{days}d ago"),
        };
    }
    if let Some(dt) = chrono::DateTime::from_timestamp_millis(timestamp_ms) {
        return dt.format("%d/%m/%Y").to_string();
    }
    lang.tr("Baru saja", "Just now").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Page, PageTemplate};
    use eframe::egui::Color32;

    #[test]
    fn test_responsive_grid_never_overflows_across_window_sizes() {
        let pad = 20.0;
        let gap = 16.0;
        for avail in [320.0_f32, 420.0, 560.0, 800.0, 1024.0, 1280.0, 1920.0] {
            let m = compute_responsive_grid(avail, pad, gap, 118.0, 134.0, 160.0, 1.29);
            assert!(m.cols >= 1, "avail={avail} must yield >= 1 col");
            let used_width = m.cols as f32 * m.card_width + m.cols.saturating_sub(1) as f32 * gap;
            let inner = (avail - pad * 2.0).max(84.0);
            assert!(
                used_width <= inner + 0.5,
                "avail={avail}: used_width ({used_width}) exceeded inner ({inner})"
            );
            assert!(m.card_width >= 118.0 || inner < 118.0);
            assert!(m.card_height > m.card_width);
        }
    }

    #[test]
    fn test_can_move_folder_into_blocks_self_and_descendants() {
        let mut root = Folder::new("Kuliah", None);
        root.id = "f-root".to_string();
        let mut child = Folder::new("Kalkulus", Some("f-root".to_string()));
        child.id = "f-child".to_string();
        let mut grand = Folder::new("Latihan", Some("f-child".to_string()));
        grand.id = "f-grand".to_string();
        let mut sibling = Folder::new("Kerja", None);
        sibling.id = "f-sib".to_string();

        let folders = vec![root, child, grand, sibling];

        assert!(!can_move_folder_into(&folders, "f-root", Some("f-root")));
        assert!(!can_move_folder_into(&folders, "f-root", Some("f-child")));
        assert!(!can_move_folder_into(&folders, "f-root", Some("f-grand")));
        assert!(can_move_folder_into(&folders, "f-root", Some("f-sib")));
        assert!(can_move_folder_into(&folders, "f-child", None));
        assert!(can_move_folder_into(&folders, "f-grand", Some("f-root")));
    }

    #[test]
    fn test_folder_breadcrumb_chain_and_path_label() {
        let mut f1 = Folder::new("Kuliah", None);
        f1.id = "f1".to_string();
        let mut f2 = Folder::new("Fisika", Some("f1".to_string()));
        f2.id = "f2".to_string();
        let folders = vec![f1, f2];

        assert_eq!(
            folder_breadcrumb_chain(&folders, Some("f2")),
            vec![
                ("f1".to_string(), "Kuliah".to_string()),
                ("f2".to_string(), "Fisika".to_string())
            ]
        );
        assert_eq!(
            folder_path_label(&folders, Some("f2"), "All Documents"),
            "All Documents / Kuliah / Fisika"
        );
        assert_eq!(
            folder_path_label(&folders, None, "Semua Dokumen"),
            "Semua Dokumen"
        );
    }

    #[test]
    fn test_sort_notebook_items_and_pdf_detection() {
        let mut nb_a = Notebook::new("Beta", PageTemplate::Blank, Color32::WHITE);
        nb_a.id = "a".to_string();
        nb_a.updated_at = 200;
        nb_a.created_at = 50;

        let mut nb_b = Notebook::new("Alpha", PageTemplate::Grid, Color32::WHITE);
        nb_b.id = "b".to_string();
        nb_b.updated_at = 100;
        nb_b.created_at = 300;
        let mut pdf_page = Page::new(PageTemplate::Blank);
        pdf_page.background = Some(PageBackground::Pdf {
            asset_id: "doc.pdf".to_string(),
            page_index: 0,
        });
        nb_b.pages.push(pdf_page);

        let list = vec![nb_a, nb_b];
        assert!(!notebook_has_pdf_pages(&list[0]));
        assert!(notebook_has_pdf_pages(&list[1]));

        let mut items = vec![(0, None), (1, None)];
        sort_notebook_items(&list, &mut items, LibrarySortBy::NameAsc);
        assert_eq!(items[0].0, 1); // Alpha

        sort_notebook_items(&list, &mut items, LibrarySortBy::LastModified);
        assert_eq!(items[0].0, 0); // Beta (200 > 100)

        sort_notebook_items(&list, &mut items, LibrarySortBy::PageCount);
        assert_eq!(items[0].0, 1); // Alpha (2 pages > 1 page)
    }

    #[test]
    fn test_format_relative_time_bilingual() {
        let now = 1_700_000_000_000_i64;
        assert_eq!(
            format_relative_time(now - 20_000, now, AppLanguage::Indonesian),
            "Baru saja"
        );
        assert_eq!(
            format_relative_time(now - 20_000, now, AppLanguage::English),
            "Just now"
        );
        assert_eq!(
            format_relative_time(now - 10 * 60_000, now, AppLanguage::English),
            "10m ago"
        );
        assert_eq!(
            format_relative_time(now - 3 * 3_600_000, now, AppLanguage::Indonesian),
            "3j lalu"
        );
        assert_eq!(
            format_relative_time(now - 26 * 3_600_000, now, AppLanguage::English),
            "Yesterday"
        );
    }
}

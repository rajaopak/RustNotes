use super::export_png::render_page_to_rgba_image;
use super::notebook_store::save_asset_to_notebook;
use crate::model::{Notebook, Page, PageBackground, PageTemplate};
use pdfium_render::prelude::*;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub fn get_pdfium() -> Result<Pdfium, String> {
    let mut searched_locations = Vec::new();
    let mut tested_paths = HashSet::new();

    let mut try_candidate = |candidate: PathBuf, desc: &str| -> Option<Pdfium> {
        let canonical_key = candidate
            .canonicalize()
            .unwrap_or_else(|_| candidate.clone());
        if !tested_paths.insert(canonical_key) {
            return None;
        }

        if !candidate.exists() {
            searched_locations.push(format!(
                "{desc}: '{}' (tidak ditemukan)",
                candidate.display()
            ));
            return None;
        }

        let lib_path_str = candidate.to_str().unwrap_or("pdfium.dll");
        match Pdfium::bind_to_library(lib_path_str) {
            Ok(bindings) => Some(Pdfium::new(bindings)),
            Err(e) => {
                searched_locations.push(format!(
                    "{desc}: '{}' (berkas ada, tetapi gagal dimuat: {e})",
                    candidate.display()
                ));
                None
            }
        }
    };

    // 1. Cek environment variable PDFIUM_PATH / PDFIUM_DYNAMIC_LIB_PATH
    for env_var in &["PDFIUM_PATH", "PDFIUM_DYNAMIC_LIB_PATH"] {
        if let Ok(env_val) = std::env::var(env_var) {
            let p = PathBuf::from(&env_val);
            let candidate = if p.is_dir() { p.join("pdfium.dll") } else { p };
            if let Some(pdfium) = try_candidate(candidate, &format!("ENV {env_var}")) {
                return Ok(pdfium);
            }
        }
    }

    // 2. Cek di samping executable aplikasi & folder deps
    if let Ok(exe_path) = std::env::current_exe()
        && let Some(parent) = exe_path.parent()
    {
        // Samping biner langsung
        if let Some(pdfium) = try_candidate(parent.join("pdfium.dll"), "Direktori Executable") {
            return Ok(pdfium);
        }

        // Folder deps (untuk integration tests)
        if let Some(pdfium) = try_candidate(
            parent.join("deps").join("pdfium.dll"),
            "Folder Executable Deps",
        ) {
            return Ok(pdfium);
        }

        // Telusuri direktori leluhur (ancestors) hingga 5 tingkat ke atas
        // (mis. target/release/../../pdfium.dll saat dijalankan via cargo run)
        let mut ancestor = parent.parent();
        let mut depth = 1;
        while let Some(anc) = ancestor {
            if depth > 5 {
                break;
            }
            let cand = anc.join("pdfium.dll");
            if let Some(pdfium) = try_candidate(
                cand,
                &format!("Leluhur Executable ({depth} tingkat di atas)"),
            ) {
                return Ok(pdfium);
            }
            ancestor = anc.parent();
            depth += 1;
        }
    }

    // 3. Cek direktori data aplikasi (%LOCALAPPDATA%\RustNotes dan %APPDATA%\RustNotes)
    if let Some(local_data) = dirs::data_local_dir() {
        let app_dir = local_data.join("RustNotes");
        if let Some(pdfium) = try_candidate(app_dir.join("pdfium.dll"), "Data Lokal Aplikasi") {
            return Ok(pdfium);
        }
        if let Some(pdfium) = try_candidate(
            app_dir.join("bin").join("pdfium.dll"),
            "Data Lokal Aplikasi /bin",
        ) {
            return Ok(pdfium);
        }
    }
    if let Some(roaming_data) = dirs::data_dir() {
        let app_dir = roaming_data.join("RustNotes");
        if let Some(pdfium) = try_candidate(app_dir.join("pdfium.dll"), "Data Roaming Aplikasi") {
            return Ok(pdfium);
        }
        if let Some(pdfium) = try_candidate(
            app_dir.join("bin").join("pdfium.dll"),
            "Data Roaming Aplikasi /bin",
        ) {
            return Ok(pdfium);
        }
    }

    // 4. Cek CARGO_MANIFEST_DIR (saat pengujian/pengembangan cargo)
    if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
        let p = PathBuf::from(manifest_dir);
        if let Some(pdfium) = try_candidate(p.join("pdfium.dll"), "Cargo Manifest Dir") {
            return Ok(pdfium);
        }
    }

    // 5. Cek direktori kerja saat ini (Current Working Directory)
    if let Some(pdfium) = try_candidate(PathBuf::from("pdfium.dll"), "Current Working Directory") {
        return Ok(pdfium);
    }
    if let Some(pdfium) = try_candidate(
        PathBuf::from("goodnotes-rust").join("pdfium.dll"),
        "CWD subfolder goodnotes-rust",
    ) {
        return Ok(pdfium);
    }

    // 6. Cek fallback system library / PATH Windows
    match Pdfium::bind_to_system_library() {
        Ok(bindings) => Ok(Pdfium::new(bindings)),
        Err(e) => {
            searched_locations.push(format!("System PATH / System32 (gagal: {e})"));

            let mut msg = String::from(
                "Pustaka PDFium ('pdfium.dll') tidak ditemukan atau gagal dimuat.\n\n",
            );
            msg.push_str("Lokasi yang telah diperiksa:\n");
            for loc in &searched_locations {
                msg.push_str(&format!("  - {loc}\n"));
            }
            msg.push_str("\nLangkah Penyelesaian:\n");
            msg.push_str("1. Pastikan berkas 'pdfium.dll' (arsitektur 64-bit / x86_64) diletakkan di direktori yang sama dengan berkas 'rustnotes.exe'.\n");
            msg.push_str("2. Atau letakkan berkas 'pdfium.dll' di direktori data aplikasi (%LOCALAPPDATA%\\RustNotes\\pdfium.dll).\n");
            msg.push_str("3. Atau atur variabel lingkungan PDFIUM_PATH mengarah ke berkas atau folder 'pdfium.dll'.\n");
            Err(msg)
        }
    }
}

pub fn import_pdf_to_pages(
    dir: &Path,
    notebook_id: &str,
    pdf_data: &[u8],
) -> Result<Vec<Page>, String> {
    let pdfium = get_pdfium()?;
    let document = pdfium
        .load_pdf_from_byte_slice(pdf_data, None)
        .map_err(|e| format!("Gagal memproses dokumen PDF: {e}"))?;

    let pdf_pages = document.pages();
    let count = pdf_pages.len();
    if count == 0 {
        return Err("Dokumen PDF tidak memiliki halaman.".to_string());
    }

    // Simpan aset PDF hanya setelah dokumen PDF berhasil divalidasi
    let pdf_asset_id = save_asset_to_notebook(dir, notebook_id, pdf_data, "pdf")?;

    let mut pages = Vec::with_capacity(count as usize);

    for idx in 0..count {
        let page = pdf_pages
            .get(idx)
            .map_err(|e| format!("Gagal membaca halaman {idx} PDF: {e}"))?;

        let width = page.width().value;
        let height = page.height().value;

        let render_config = PdfRenderConfig::new().set_target_width((width * 2.0).round() as i32);

        let mut bg = PageBackground::Pdf {
            asset_id: pdf_asset_id.clone(),
            page_index: idx as usize,
        };

        if let Ok(rendered) = page.render_with_config(&render_config) {
            let dyn_img = rendered.as_image();
            let mut buf = std::io::Cursor::new(Vec::new());
            if dyn_img.write_to(&mut buf, image::ImageFormat::Jpeg).is_ok()
                && let Ok(img_asset) =
                    save_asset_to_notebook(dir, notebook_id, &buf.into_inner(), "jpg")
            {
                bg = PageBackground::Image {
                    asset_id: img_asset,
                };
            }
        }

        let mut new_page = Page::with_size(PageTemplate::Blank, width, height);
        new_page.background = Some(bg);
        pages.push(new_page);
    }

    Ok(pages)
}

pub fn export_notebook_to_pdf(
    dir: &Path,
    notebook: &Notebook,
    output_path: &Path,
) -> Result<(), String> {
    if notebook.pages.is_empty() {
        return Err("Buku catatan tidak memiliki halaman untuk diekspor ke PDF.".to_string());
    }

    if let Some(parent) = output_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let pdfium = get_pdfium()?;
    let mut new_doc = pdfium
        .create_new_pdf()
        .map_err(|e| format!("Gagal membuat dokumen PDF baru: {e}"))?;

    for (idx, page) in notebook.pages.iter().enumerate() {
        let rgba_img = render_page_to_rgba_image(dir, &notebook.id, page)?;
        let dyn_img = image::DynamicImage::ImageRgba8(rgba_img);

        let w_pts = PdfPoints::new(page.width);
        let h_pts = PdfPoints::new(page.height);
        let mut pdf_page = new_doc
            .pages_mut()
            .create_page_at_end(PdfPagePaperSize::Custom(w_pts, h_pts))
            .map_err(|e| format!("Gagal membuat halaman {idx} PDF: {e}"))?;

        pdf_page
            .objects_mut()
            .create_image_object(
                PdfPoints::new(0.0),
                PdfPoints::new(0.0),
                &dyn_img,
                Some(w_pts),
                Some(h_pts),
            )
            .map_err(|e| format!("Gagal menambahkan objek gambar ke halaman {idx} PDF: {e}"))?;
    }

    // Simpan melalui berkas sementara unik (UUID) untuk menjamin atomic export
    let unique_id = uuid::Uuid::new_v4();
    let temp_path = output_path.with_extension(format!("tmp.{}", unique_id));

    if let Err(e) = new_doc.save_to_file(&temp_path) {
        let _ = std::fs::remove_file(&temp_path);
        return Err(format!("Gagal menyimpan berkas PDF sementara: {e}"));
    }

    // Rename dengan retry (Windows file locking defense)
    let mut rename_success = false;
    for _ in 0..5 {
        if std::fs::rename(&temp_path, output_path).is_ok() {
            rename_success = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    if !rename_success && std::fs::copy(&temp_path, output_path).is_ok() {
        let _ = std::fs::remove_file(&temp_path);
        rename_success = true;
    }

    if !rename_success {
        let _ = std::fs::remove_file(&temp_path);
        return Err(format!(
            "Gagal memindahkan berkas PDF ke tujuan akhir '{}'",
            output_path.display()
        ));
    }

    Ok(())
}

use super::export_png::render_page_to_rgba_image;
use super::notebook_store::save_asset_to_notebook;
use crate::model::{Notebook, Page, PageBackground, PageTemplate};
use pdfium_render::prelude::*;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

fn inspect_pe_architecture(path: &Path) -> Result<u16, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("gagal membaca berkas ({e})"))?;
    if bytes.len() < 0x40 || &bytes[0..2] != b"MZ" {
        return Err(format!(
            "bukan biner Windows PE/DLL yang valid ({} byte)",
            bytes.len()
        ));
    }
    let pe_offset =
        u32::from_le_bytes([bytes[0x3C], bytes[0x3D], bytes[0x3E], bytes[0x3F]]) as usize;
    if pe_offset + 6 > bytes.len() || &bytes[pe_offset..pe_offset + 4] != b"PE\0\0" {
        return Err("signature PE tidak valid".to_string());
    }
    Ok(u16::from_le_bytes([
        bytes[pe_offset + 4],
        bytes[pe_offset + 5],
    ]))
}

fn diagnose_pdfium_load_failure(path: &Path, err: &dyn std::fmt::Display) -> String {
    match inspect_pe_architecture(path) {
        Err(pe_err) => format!("berkas ada, tetapi tidak valid: {pe_err}"),
        Ok(machine) => {
            #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
            if machine != 0x8664 {
                let arch_label = match machine {
                    0x014C => "x86 (32-bit)",
                    0xAA64 => "ARM64",
                    _ => "tidak dikenal",
                };
                return format!(
                    "arsitektur DLL tidak cocok: ditemukan {arch_label} (0x{machine:04X}), dibutuhkan x64 (0x8664)"
                );
            }
            format!(
                "arsitektur PE valid (0x{machine:04X}), tetapi gagal dimuat/inisialisasi (kemungkinan DLL dependensi hilang atau inisialisasi PDFium gagal): {err}"
            )
        }
    }
}

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
                let detail = diagnose_pdfium_load_failure(&candidate, &e);
                searched_locations.push(format!("{desc}: '{}' ({detail})", candidate.display()));
                None
            }
        }
    };

    // 1. Cek environment variable RUSTNOTES_PDFIUM_PATH / PDFIUM_PATH / PDFIUM_DYNAMIC_LIB_PATH
    for env_var in &[
        "RUSTNOTES_PDFIUM_PATH",
        "PDFIUM_PATH",
        "PDFIUM_DYNAMIC_LIB_PATH",
    ] {
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
            msg.push_str("3. Atau atur variabel lingkungan RUSTNOTES_PDFIUM_PATH (atau PDFIUM_PATH) mengarah ke berkas atau folder 'pdfium.dll'.\n");
            Err(msg)
        }
    }
}

/// Memvalidasi dependensi runtime dan direktori penyimpanan untuk rilis Windows:
/// 1. Direktori penyimpanan (`app_storage_dir()`) dapat dibuat dan ditulisi.
/// 2. Pustaka dinamis `pdfium.dll` ditemukan, berarsitektur cocok, dan berhasil diinisialisasi.
pub fn validate_runtime_environment() -> Result<String, String> {
    let storage_dir = super::app_storage_dir();
    std::fs::create_dir_all(&storage_dir).map_err(|e| {
        format!(
            "Gagal membuat direktori penyimpanan '{}': {e}",
            storage_dir.display()
        )
    })?;
    let probe_path = storage_dir.join(".runtime_write_probe");
    std::fs::write(&probe_path, b"ok").map_err(|e| {
        format!(
            "Direktori penyimpanan '{}' tidak dapat ditulisi: {e}",
            storage_dir.display()
        )
    })?;
    let _ = std::fs::remove_file(&probe_path);

    let pdfium = get_pdfium()?;
    // Pastikan pustaka PDFium benar-benar dapat diinisialisasi di memori (bukan sekadar DLL ada)
    let _doc = pdfium
        .create_new_pdf()
        .map_err(|e| format!("PDFium DLL dimuat, tetapi gagal diinisialisasi: {e}"))?;

    let mode = if super::is_portable_mode() {
        "Portable"
    } else {
        "Installed"
    };
    Ok(format!(
        "RustNotes v{} runtime OK (mode={mode}, storage='{}', pdfium=initialized)",
        env!("CARGO_PKG_VERSION"),
        storage_dir.display()
    ))
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

    // Simpan aset PDF secara malas (lazy) hanya jika ada halaman yang gagal di-render ke gambar
    let mut pdf_asset_id: Option<String> = None;

    let mut pages = Vec::with_capacity(count as usize);

    for idx in 0..count {
        let page = pdf_pages
            .get(idx)
            .map_err(|e| format!("Gagal membaca halaman {idx} PDF: {e}"))?;

        let width = page.width().value;
        let height = page.height().value;

        let render_config = PdfRenderConfig::new().set_target_width((width * 2.0).round() as i32);

        let mut bg = None;

        if let Ok(rendered) = page.render_with_config(&render_config) {
            let dyn_img = rendered.as_image();
            let mut buf = std::io::Cursor::new(Vec::new());
            if dyn_img.write_to(&mut buf, image::ImageFormat::Jpeg).is_ok()
                && let Ok(img_asset) =
                    save_asset_to_notebook(dir, notebook_id, &buf.into_inner(), "jpg")
            {
                bg = Some(PageBackground::Image {
                    asset_id: img_asset,
                });
            }
        }

        let bg = match bg {
            Some(img_bg) => img_bg,
            None => {
                let asset_id = match &pdf_asset_id {
                    Some(id) => id.clone(),
                    None => {
                        let id = save_asset_to_notebook(dir, notebook_id, pdf_data, "pdf")?;
                        pdf_asset_id = Some(id.clone());
                        id
                    }
                };
                PageBackground::Pdf {
                    asset_id,
                    page_index: idx as usize,
                }
            }
        };

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_runtime_environment_initializes_pdfium() {
        let summary = validate_runtime_environment().expect("PDFium runtime should initialize");
        assert!(summary.contains("pdfium=initialized"));
    }

    #[test]
    fn test_diagnose_pdfium_load_failure_distinguishes_pe_and_arch() {
        let temp = tempfile::tempdir().expect("tempdir");
        let bad_pe = temp.path().join("bad.dll");
        std::fs::write(&bad_pe, b"not a dll").expect("write bad file");
        let dummy_err = "LoadLibrary failed";
        let msg_bad = diagnose_pdfium_load_failure(&bad_pe, &dummy_err);
        assert!(msg_bad.contains("bukan biner Windows PE/DLL yang valid"));

        // Minimal synthetic x86 (0x014C) PE header
        let mut x86_pe = vec![0u8; 0x90];
        x86_pe[0..2].copy_from_slice(b"MZ");
        x86_pe[0x3C..0x40].copy_from_slice(&0x80u32.to_le_bytes());
        x86_pe[0x80..0x84].copy_from_slice(b"PE\0\0");
        x86_pe[0x84..0x86].copy_from_slice(&0x014Cu16.to_le_bytes());
        let x86_path = temp.path().join("x86.dll");
        std::fs::write(&x86_path, &x86_pe).expect("write x86 pe");
        let msg_arch = diagnose_pdfium_load_failure(&x86_path, &dummy_err);
        assert!(msg_arch.contains("0x014C"));
        assert!(msg_arch.contains("0x8664"));
    }
}

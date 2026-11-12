use super::export_png::render_page_to_rgba_image;
use super::notebook_store::save_asset_to_notebook;
use crate::model::{Notebook, Page, PageBackground, PageTemplate};
use pdfium_render::prelude::*;
use std::path::Path;

pub fn get_pdfium() -> Result<Pdfium, String> {
    let mut searched_locations = Vec::new();

    // 0. Cek environment variable PDFIUM_PATH / PDFIUM_DYNAMIC_LIB_PATH
    if let Ok(env_path) =
        std::env::var("PDFIUM_PATH").or_else(|_| std::env::var("PDFIUM_DYNAMIC_LIB_PATH"))
    {
        let p = Path::new(&env_path);
        let candidate = if p.is_dir() {
            p.join("pdfium.dll")
        } else {
            p.to_path_buf()
        };
        searched_locations.push(format!("ENV: {}", candidate.display()));
        if candidate.exists()
            && let Ok(bindings) =
                Pdfium::bind_to_library(candidate.to_str().unwrap_or("pdfium.dll"))
        {
            return Ok(Pdfium::new(bindings));
        }
    }

    // 1. Cek di samping executable
    if let Ok(exe_path) = std::env::current_exe()
        && let Some(parent) = exe_path.parent()
    {
        let candidate = parent.join("pdfium.dll");
        searched_locations.push(candidate.display().to_string());
        if candidate.exists()
            && let Ok(bindings) =
                Pdfium::bind_to_library(candidate.to_str().unwrap_or("pdfium.dll"))
        {
            return Ok(Pdfium::new(bindings));
        }
    }

    // 2. Cek di direktori kerja
    let candidate = Path::new("pdfium.dll");
    searched_locations.push(candidate.display().to_string());
    if candidate.exists()
        && let Ok(bindings) = Pdfium::bind_to_library("pdfium.dll")
    {
        return Ok(Pdfium::new(bindings));
    }

    // 3. Fallback sistem / PATH
    Pdfium::bind_to_system_library()
        .map(Pdfium::new)
        .map_err(|e| {
            format!(
                "PDFium library tidak ditemukan ({e}). Lokasi yang diperiksa: {}. Pastikan 'pdfium.dll' berada di direktori aplikasi, path PDFIUM_PATH, atau terdaftar dalam PATH sistem.",
                searched_locations.join(", ")
            )
        })
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

    new_doc.save_to_file(output_path).map_err(|e| {
        format!(
            "Gagal menyimpan berkas PDF ke '{}': {e}",
            output_path.display()
        )
    })?;

    Ok(())
}

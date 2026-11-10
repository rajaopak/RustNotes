use super::export_png::export_page_to_png;
use super::notebook_store::save_asset_to_notebook;
use crate::model::{Notebook, Page, PageBackground, PageTemplate};
use pdfium_render::prelude::*;
use std::fs;
use std::path::Path;

pub fn get_pdfium() -> Result<Pdfium, String> {
    // 1. Cek di samping executable
    if let Ok(exe_path) = std::env::current_exe()
        && let Some(parent) = exe_path.parent()
    {
        let candidate = parent.join("pdfium.dll");
        if candidate.exists()
            && let Ok(bindings) =
                Pdfium::bind_to_library(candidate.to_str().unwrap_or("pdfium.dll"))
        {
            return Ok(Pdfium::new(bindings));
        }
    }

    // 2. Cek di direktori kerja
    let candidate = Path::new("pdfium.dll");
    if candidate.exists()
        && let Ok(bindings) = Pdfium::bind_to_library("pdfium.dll")
    {
        return Ok(Pdfium::new(bindings));
    }

    // 3. Fallback sistem
    Pdfium::bind_to_system_library()
        .map(Pdfium::new)
        .map_err(|e| {
            format!(
                "PDFium library tidak ditemukan: {e}. Pastikan 'pdfium.dll' berada di direktori aplikasi."
            )
        })
}

pub fn import_pdf_to_pages(
    dir: &Path,
    notebook_id: &str,
    pdf_data: &[u8],
) -> Result<Vec<Page>, String> {
    let pdf_asset_id = save_asset_to_notebook(dir, notebook_id, pdf_data, "pdf")?;

    let pdfium = get_pdfium()?;
    let document = pdfium
        .load_pdf_from_byte_slice(pdf_data, None)
        .map_err(|e| format!("Gagal memproses dokumen PDF: {e}"))?;

    let pdf_pages = document.pages();
    let count = pdf_pages.len();
    if count == 0 {
        return Err("Dokumen PDF tidak memiliki halaman.".to_string());
    }

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
    let pdfium = get_pdfium()?;
    let mut new_doc = pdfium
        .create_new_pdf()
        .map_err(|e| format!("Gagal membuat dokumen PDF: {e}"))?;

    for (idx, page) in notebook.pages.iter().enumerate() {
        let temp_png = dir.join(format!("temp_export_{idx}.png"));
        export_page_to_png(dir, &notebook.id, page, &temp_png)?;

        let w_pts = PdfPoints::new(page.width);
        let h_pts = PdfPoints::new(page.height);
        let mut pdf_page = new_doc
            .pages_mut()
            .create_page_at_end(PdfPagePaperSize::Custom(w_pts, h_pts))
            .map_err(|e| format!("Gagal membuat halaman PDF: {e}"))?;

        if let Ok(data) = fs::read(&temp_png) {
            let _ = fs::remove_file(&temp_png);
            if let Ok(dyn_img) = image::load_from_memory(&data) {
                let _ = pdf_page.objects_mut().create_image_object(
                    PdfPoints::new(0.0),
                    PdfPoints::new(0.0),
                    &dyn_img,
                    Some(w_pts),
                    Some(h_pts),
                );
            }
        }
    }

    new_doc
        .save_to_file(output_path)
        .map_err(|e| format!("Gagal menyimpan PDF: {e}"))?;

    Ok(())
}

use crate::model::{
    CURRENT_SCHEMA_VERSION, Library, LibraryIndex, Notebook, NotebookMeta, Page, PageBackground,
    PageTemplate,
};
use pdfium_render::prelude::*;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq)]
pub enum LoadStatus {
    LoadedDirect,
    RecoveredFromPrevious,
    CorruptedQuarantined { path: PathBuf },
    NewerVersionReadOnly { version: u32 },
}

#[derive(Clone, Debug)]
pub struct LoadResult {
    pub library: Library,
    pub status: LoadStatus,
}

pub fn app_storage_dir() -> PathBuf {
    let mut dir = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
    dir.push("RustNotes");
    let _ = fs::create_dir_all(&dir);
    dir
}

/// Applies sequential migrations (v1 -> v2 -> ...) to a raw JSON value.
pub fn migrate(mut value: serde_json::Value) -> Result<Library, String> {
    let version = value
        .get("schema_version")
        .or_else(|| value.get("version"))
        .and_then(|v| v.as_u64())
        .map(|v| v as u32)
        .unwrap_or(1);

    if version > CURRENT_SCHEMA_VERSION {
        return Err(format!(
            "Data file has schema version {}, which is newer than supported version {}.",
            version, CURRENT_SCHEMA_VERSION
        ));
    }

    // Step-by-step sequential migration:
    // v1 -> v2: Remove legacy "version" alias if present and set "schema_version" to 2
    if version == 1
        && let serde_json::Value::Object(ref mut map) = value
    {
        map.remove("version");
        map.insert(
            "schema_version".to_string(),
            serde_json::json!(CURRENT_SCHEMA_VERSION),
        );
    }

    serde_json::from_value(value).map_err(|e| format!("Failed to parse migrated library: {e}"))
}

pub fn load_library() -> LoadResult {
    load_library_from_dir(&app_storage_dir())
}

pub fn load_library_from_dir(dir: &Path) -> LoadResult {
    let current_path = dir.join("library.json");
    let previous_path = dir.join("library.previous.json");

    // 1. Try reading and migrating current library.json
    if current_path.exists()
        && let Ok(content) = fs::read_to_string(&current_path)
        && let Ok(value) = serde_json::from_str::<serde_json::Value>(&content)
    {
        // Check if file is from a newer version
        let ver = value
            .get("schema_version")
            .or_else(|| value.get("version"))
            .and_then(|v| v.as_u64())
            .map(|v| v as u32)
            .unwrap_or(1);

        if ver > CURRENT_SCHEMA_VERSION {
            if let Ok(lib) = serde_json::from_value::<Library>(value) {
                return LoadResult {
                    library: lib,
                    status: LoadStatus::NewerVersionReadOnly { version: ver },
                };
            }
        } else if let Ok(lib) = migrate(value) {
            return LoadResult {
                library: lib,
                status: LoadStatus::LoadedDirect,
            };
        }
    }

    // 2. Current file failed or corrupted. Try recovering from library.previous.json
    if previous_path.exists()
        && let Ok(content) = fs::read_to_string(&previous_path)
        && let Ok(value) = serde_json::from_str::<serde_json::Value>(&content)
        && let Ok(lib) = migrate(value)
    {
        // Restore current_path with recovered data
        let _ = save_library_to_dir(dir, &lib);
        return LoadResult {
            library: lib,
            status: LoadStatus::RecoveredFromPrevious,
        };
    }

    // 3. Both failed or no valid backup. Quarantine current corrupt file if it exists
    if current_path.exists() {
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string();
        let corrupt_path = dir.join(format!("library.corrupt.{timestamp}.json"));
        let _ = fs::rename(&current_path, &corrupt_path);
        return LoadResult {
            library: Library::default(),
            status: LoadStatus::CorruptedQuarantined { path: corrupt_path },
        };
    }

    // 4. Default fresh library for first run
    LoadResult {
        library: Library::default(),
        status: LoadStatus::LoadedDirect,
    }
}

pub fn atomic_write_file(
    target_path: &Path,
    content: &str,
    backup_path: Option<&Path>,
) -> Result<(), String> {
    if let Some(parent) = target_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let unique_id = uuid::Uuid::new_v4();
    let temp_path = target_path.with_extension(format!("tmp.{}", unique_id));

    let mut file = fs::File::create(&temp_path).map_err(|e| {
        format!(
            "Failed to create temporary file {}: {e}",
            temp_path.display()
        )
    })?;
    file.write_all(content.as_bytes())
        .map_err(|e| format!("Failed to write temporary file: {e}"))?;
    file.sync_all()
        .map_err(|e| format!("Failed to sync temporary file: {e}"))?;
    drop(file);

    if let Some(prev) = backup_path
        && target_path.exists()
    {
        let _ = fs::copy(target_path, prev);
    }

    let mut rename_success = false;
    for _ in 0..5 {
        if fs::rename(&temp_path, target_path).is_ok() {
            rename_success = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    if !rename_success && fs::copy(&temp_path, target_path).is_ok() {
        let _ = fs::remove_file(&temp_path);
        rename_success = true;
    }

    if !rename_success {
        let _ = fs::remove_file(&temp_path);
        return Err(format!(
            "Failed to atomically write to {}",
            target_path.display()
        ));
    }

    Ok(())
}

pub fn save_notebook_to_dir(dir: &Path, notebook: &Notebook) -> Result<(), String> {
    let nb_dir = dir.join("notebooks").join(&notebook.id);
    let _ = fs::create_dir_all(&nb_dir);
    let _ = fs::create_dir_all(nb_dir.join("assets"));

    let target_path = nb_dir.join("notebook.json");
    let previous_path = nb_dir.join("notebook.previous.json");

    let content = serde_json::to_string_pretty(notebook)
        .map_err(|e| format!("Failed to serialize notebook {}: {e}", notebook.id))?;

    atomic_write_file(&target_path, &content, Some(&previous_path))
}

pub fn load_notebook_from_dir(dir: &Path, notebook_id: &str) -> Result<Notebook, String> {
    let nb_dir = dir.join("notebooks").join(notebook_id);
    let target_path = nb_dir.join("notebook.json");
    let previous_path = nb_dir.join("notebook.previous.json");

    // 1. Try primary notebook.json
    if target_path.exists()
        && let Ok(content) = fs::read_to_string(&target_path)
        && let Ok(nb) = serde_json::from_str::<Notebook>(&content)
    {
        return Ok(nb);
    }

    // 2. Try previous backup
    if previous_path.exists()
        && let Ok(content) = fs::read_to_string(&previous_path)
        && let Ok(nb) = serde_json::from_str::<Notebook>(&content)
    {
        let _ = save_notebook_to_dir(dir, &nb);
        return Ok(nb);
    }

    // 3. Quarantine corrupt file if present
    if target_path.exists() {
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string();
        let corrupt_path = nb_dir.join(format!("notebook.corrupt.{timestamp}.json"));
        let _ = fs::rename(&target_path, &corrupt_path);
        return Err(format!(
            "Notebook corrupted; quarantined to {}",
            corrupt_path.display()
        ));
    }

    Err(format!("Notebook {} not found", notebook_id))
}

pub fn save_library_index_to_dir(dir: &Path, index: &LibraryIndex) -> Result<(), String> {
    let target = dir.join("library_index.json");
    let previous = dir.join("library_index.previous.json");
    let content = serde_json::to_string_pretty(index)
        .map_err(|e| format!("Failed to serialize library index: {e}"))?;
    atomic_write_file(&target, &content, Some(&previous))
}

#[derive(Clone, Debug, PartialEq)]
pub enum LoadIndexStatus {
    LoadedDirect,
    RecoveredFromPrevious,
    MigratedFromLegacy,
    FreshIndex,
}

#[derive(Clone, Debug)]
pub struct LoadIndexResult {
    pub index: LibraryIndex,
    pub status: LoadIndexStatus,
}

pub fn load_library_index_from_dir(dir: &Path) -> LoadIndexResult {
    let index_path = dir.join("library_index.json");
    let prev_path = dir.join("library_index.previous.json");

    // 1. Direct load
    if index_path.exists()
        && let Ok(content) = fs::read_to_string(&index_path)
        && let Ok(idx) = serde_json::from_str::<LibraryIndex>(&content)
    {
        return LoadIndexResult {
            index: idx,
            status: LoadIndexStatus::LoadedDirect,
        };
    }

    // 2. Fallback to previous
    if prev_path.exists()
        && let Ok(content) = fs::read_to_string(&prev_path)
        && let Ok(idx) = serde_json::from_str::<LibraryIndex>(&content)
    {
        let _ = save_library_index_to_dir(dir, &idx);
        return LoadIndexResult {
            index: idx,
            status: LoadIndexStatus::RecoveredFromPrevious,
        };
    }

    // 3. Migrate from legacy library.json if present
    let legacy_path = dir.join("library.json");
    if legacy_path.exists() {
        let legacy_res = load_library_from_dir(dir);
        let backup_dir = dir.join("backup");
        let _ = fs::create_dir_all(&backup_dir);
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string();
        let backup_file = backup_dir.join(format!("library.migrated.{timestamp}.json"));
        let _ = fs::copy(&legacy_path, &backup_file);

        let mut index = LibraryIndex {
            folders: legacy_res.library.folders,
            ..Default::default()
        };
        for nb in &legacy_res.library.notebooks {
            let _ = save_notebook_to_dir(dir, nb);
            index.notebooks.push(NotebookMeta::from_notebook(nb));
        }
        let _ = save_library_index_to_dir(dir, &index);
        let _ = fs::remove_file(&legacy_path);

        return LoadIndexResult {
            index,
            status: LoadIndexStatus::MigratedFromLegacy,
        };
    }

    // 4. Fresh empty index
    let index = LibraryIndex::default();
    let _ = save_library_index_to_dir(dir, &index);
    LoadIndexResult {
        index,
        status: LoadIndexStatus::FreshIndex,
    }
}

pub fn export_notebook_to_zip(
    dir: &Path,
    notebook_id: &str,
    zip_dest: &Path,
) -> Result<(), String> {
    use zip::write::SimpleFileOptions;

    let nb = load_notebook_from_dir(dir, notebook_id)?;
    let nb_dir = dir.join("notebooks").join(notebook_id);

    let file =
        fs::File::create(zip_dest).map_err(|e| format!("Failed to create export zip file: {e}"))?;
    let mut zip = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    // 1. Manifest
    let manifest = serde_json::json!({
        "format": "rustnotes-archive",
        "version": CURRENT_SCHEMA_VERSION,
        "notebook_id": nb.id,
        "title": nb.title,
        "page_count": nb.pages.len(),
        "exported_at": chrono::Utc::now().timestamp_millis(),
    });
    zip.start_file("manifest.json", options)
        .map_err(|e| format!("Zip error: {e}"))?;
    zip.write_all(manifest.to_string().as_bytes())
        .map_err(|e| format!("Zip write error: {e}"))?;

    // 2. Notebook JSON
    let nb_json = serde_json::to_string_pretty(&nb)
        .map_err(|e| format!("Failed to serialize notebook: {e}"))?;
    zip.start_file("notebook.json", options)
        .map_err(|e| format!("Zip error: {e}"))?;
    zip.write_all(nb_json.as_bytes())
        .map_err(|e| format!("Zip write error: {e}"))?;

    // 3. Assets if any
    let assets_dir = nb_dir.join("assets");
    if assets_dir.exists()
        && let Ok(entries) = fs::read_dir(assets_dir)
    {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                let file_name = path.file_name().unwrap_or_default().to_string_lossy();
                let zip_path = format!("assets/{file_name}");
                zip.start_file(zip_path, options)
                    .map_err(|e| format!("Zip error: {e}"))?;
                if let Ok(bytes) = fs::read(&path) {
                    zip.write_all(&bytes)
                        .map_err(|e| format!("Zip write error: {e}"))?;
                }
            }
        }
    }

    zip.finish()
        .map_err(|e| format!("Failed to finalize zip: {e}"))?;
    Ok(())
}

pub fn import_notebook_from_zip(dir: &Path, zip_source: &Path) -> Result<Notebook, String> {
    use std::io::Read;

    let file =
        fs::File::open(zip_source).map_err(|e| format!("Failed to open zip archive: {e}"))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("Failed to read zip archive: {e}"))?;

    // Read notebook.json from zip
    let mut nb_file = archive
        .by_name("notebook.json")
        .map_err(|e| format!("Archive missing notebook.json: {e}"))?;
    let mut content = String::new();
    nb_file
        .read_to_string(&mut content)
        .map_err(|e| format!("Failed to read notebook.json from archive: {e}"))?;
    drop(nb_file);

    let nb: Notebook =
        serde_json::from_str(&content).map_err(|e| format!("Invalid notebook.json in zip: {e}"))?;

    // Ensure directory exists
    let nb_dir = dir.join("notebooks").join(&nb.id);
    let assets_dir = nb_dir.join("assets");
    let _ = fs::create_dir_all(&assets_dir);

    // Extract assets
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("Zip entry error: {e}"))?;
        let entry_name = entry.name().to_string();
        if entry_name.starts_with("assets/") && !entry.is_dir() {
            let file_name = Path::new(&entry_name).file_name().unwrap_or_default();
            let dest_path = assets_dir.join(file_name);
            let mut buf = Vec::new();
            entry
                .read_to_end(&mut buf)
                .map_err(|e| format!("Read asset error: {e}"))?;
            let mut out =
                fs::File::create(dest_path).map_err(|e| format!("Write asset error: {e}"))?;
            out.write_all(&buf)
                .map_err(|e| format!("Asset write error: {e}"))?;
        }
    }

    // Save notebook to disk
    save_notebook_to_dir(dir, &nb)?;

    // Update library index
    let mut index_res = load_library_index_from_dir(dir);
    index_res.index.notebooks.retain(|n| n.id != nb.id);
    index_res
        .index
        .notebooks
        .push(NotebookMeta::from_notebook(&nb));
    save_library_index_to_dir(dir, &index_res.index)?;

    Ok(nb)
}

/// Ekspor buku catatan ke format berkas resmi `.rnote` (zip package).
pub fn export_notebook_to_rnote(
    dir: &Path,
    notebook_id: &str,
    rnote_dest: &Path,
) -> Result<(), String> {
    export_notebook_to_zip(dir, notebook_id, rnote_dest)
}

/// Impor buku catatan dari format berkas resmi `.rnote` (atau `.zip`).
pub fn import_notebook_from_rnote(dir: &Path, rnote_source: &Path) -> Result<Notebook, String> {
    import_notebook_from_zip(dir, rnote_source)
}

pub fn save_asset_to_notebook(
    dir: &Path,
    notebook_id: &str,
    data: &[u8],
    extension: &str,
) -> Result<String, String> {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let hash_hex = format!("{:x}", hasher.finalize());
    let ext = extension.trim_start_matches('.').to_lowercase();
    let asset_filename = format!("{hash_hex}.{ext}");

    let assets_dir = dir.join("notebooks").join(notebook_id).join("assets");
    let _ = fs::create_dir_all(&assets_dir);
    let target_path = assets_dir.join(&asset_filename);
    if !target_path.exists() {
        fs::write(&target_path, data)
            .map_err(|e| format!("Failed to save asset {asset_filename}: {e}"))?;
    }
    Ok(asset_filename)
}

pub fn load_asset_from_notebook(
    dir: &Path,
    notebook_id: &str,
    asset_id: &str,
) -> Result<Vec<u8>, String> {
    let asset_path = dir
        .join("notebooks")
        .join(notebook_id)
        .join("assets")
        .join(asset_id);
    fs::read(&asset_path).map_err(|e| format!("Failed to read asset {asset_id}: {e}"))
}

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
    // 1. Simpan berkas PDF asli sebagai aset notebook
    let pdf_asset_id = save_asset_to_notebook(dir, notebook_id, pdf_data, "pdf")?;

    // 2. Muat PDF via PDFium
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

        // Render bitmap pada 2x resolusi untuk ketajaman tampilan
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

fn blend_pixel(img: &mut image::RgbaImage, x: i32, y: i32, color: [u8; 4]) {
    if x < 0 || y < 0 || x >= img.width() as i32 || y >= img.height() as i32 {
        return;
    }
    let alpha = color[3] as f32 / 255.0;
    if alpha <= 0.0 {
        return;
    }
    let dst = img.get_pixel_mut(x as u32, y as u32);
    if alpha >= 0.999 {
        *dst = image::Rgba([color[0], color[1], color[2], 255]);
    } else {
        let inv = 1.0 - alpha;
        let r = (color[0] as f32 * alpha + dst[0] as f32 * inv)
            .round()
            .clamp(0.0, 255.0) as u8;
        let g = (color[1] as f32 * alpha + dst[1] as f32 * inv)
            .round()
            .clamp(0.0, 255.0) as u8;
        let b = (color[2] as f32 * alpha + dst[2] as f32 * inv)
            .round()
            .clamp(0.0, 255.0) as u8;
        *dst = image::Rgba([r, g, b, 255]);
    }
}

fn draw_capsule_segment(
    img: &mut image::RgbaImage,
    a: eframe::egui::Pos2,
    b: eframe::egui::Pos2,
    radius: f32,
    color: [u8; 4],
) {
    let r = radius.max(0.6);
    let min_x = (a.x.min(b.x) - r - 1.0).floor().max(0.0) as i32;
    let max_x = (a.x.max(b.x) + r + 1.0)
        .ceil()
        .min(img.width() as f32 - 1.0) as i32;
    let min_y = (a.y.min(b.y) - r - 1.0).floor().max(0.0) as i32;
    let max_y = (a.y.max(b.y) + r + 1.0)
        .ceil()
        .min(img.height() as f32 - 1.0) as i32;

    for py in min_y..=max_y {
        for px in min_x..=max_x {
            let p = eframe::egui::Pos2::new(px as f32 + 0.5, py as f32 + 0.5);
            let d = crate::geometry::point_to_segment_dist(p, a, b);
            if d <= r + 0.5 {
                let coverage = (r + 0.5 - d).clamp(0.0, 1.0);
                let eff_alpha = (color[3] as f32 * coverage).round() as u8;
                blend_pixel(img, px, py, [color[0], color[1], color[2], eff_alpha]);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_egui_text(
    img: &mut image::RgbaImage,
    fonts: &eframe::egui::text::Fonts,
    x_pt: f32,
    y_pt: f32,
    text: &str,
    font_size_pt: f32,
    color: [u8; 4],
    scale: f32,
) {
    if text.trim().is_empty() {
        return;
    }
    let font_id = eframe::egui::FontId::proportional(font_size_pt);
    let galley = fonts.layout_no_wrap(
        text.to_string(),
        font_id,
        eframe::egui::Color32::from_rgba_unmultiplied(color[0], color[1], color[2], color[3]),
    );

    let font_image = fonts.image();
    let atlas_w = font_image.width();
    let atlas_h = font_image.height();

    for row in &galley.rows {
        for glyph in &row.glyphs {
            let uv_rect = glyph.uv_rect;
            if uv_rect.is_nothing() {
                continue;
            }
            let glyph_x_px = ((x_pt + glyph.pos.x + uv_rect.offset.x) * scale).round() as i32;
            let glyph_y_px = ((y_pt + glyph.pos.y + uv_rect.offset.y) * scale).round() as i32;

            let u_min = uv_rect.min[0] as usize;
            let v_min = uv_rect.min[1] as usize;
            let u_max = uv_rect.max[0] as usize;
            let v_max = uv_rect.max[1] as usize;

            let w = u_max.saturating_sub(u_min);
            let h = v_max.saturating_sub(v_min);

            for dy in 0..h {
                let v = v_min + dy;
                if v >= atlas_h {
                    continue;
                }
                for dx in 0..w {
                    let u = u_min + dx;
                    if u >= atlas_w {
                        continue;
                    }
                    let coverage = font_image[(u, v)];
                    if coverage > 0.005 {
                        let eff_alpha = ((color[3] as f32 * coverage).clamp(0.0, 255.0)) as u8;
                        blend_pixel(
                            img,
                            glyph_x_px + dx as i32,
                            glyph_y_px + dy as i32,
                            [color[0], color[1], color[2], eff_alpha],
                        );
                    }
                }
            }
        }
    }
}

pub fn export_page_to_png(
    dir: &Path,
    notebook_id: &str,
    page: &Page,
    output_path: &Path,
) -> Result<(), String> {
    let scale = 2.0_f32; // 2x resolution
    let width_px = (page.width * scale).round() as u32;
    let height_px = (page.height * scale).round() as u32;

    let mut img = image::RgbaImage::new(width_px, height_px);
    let fonts =
        eframe::egui::text::Fonts::new(scale, 4096, eframe::egui::FontDefinitions::default());

    // 1. Isi warna kertas dasar
    let bg_color = image::Rgba([
        page.paper_color.r,
        page.paper_color.g,
        page.paper_color.b,
        255,
    ]);
    for pixel in img.pixels_mut() {
        *pixel = bg_color;
    }

    // 2. Render latar belakang halaman (PDF / Gambar atau Pola Template Kertas)
    match page.effective_background() {
        PageBackground::Image { asset_id } | PageBackground::Pdf { asset_id, .. } => {
            if let Ok(bytes) = load_asset_from_notebook(dir, notebook_id, &asset_id)
                && let Ok(bg_img) = image::load_from_memory(&bytes)
            {
                let rotated_img = match page.rotation.rem_euclid(360) {
                    90 => bg_img.rotate90(),
                    180 => bg_img.rotate180(),
                    270 => bg_img.rotate270(),
                    _ => bg_img,
                };
                let resized = rotated_img.resize_exact(
                    width_px,
                    height_px,
                    image::imageops::FilterType::Lanczos3,
                );
                image::imageops::overlay(&mut img, &resized, 0, 0);
            }
        }
        PageBackground::Template(tmpl) => {
            let spacing = 30.0 * scale;
            let line_col = [220, 226, 231, 255];
            let w_f = width_px as f32;
            let h_f = height_px as f32;
            match tmpl {
                crate::model::PageTemplate::Blank => {}
                crate::model::PageTemplate::Ruled => {
                    let mut y = spacing;
                    while y < h_f {
                        draw_capsule_segment(
                            &mut img,
                            eframe::egui::Pos2::new(0.0, y),
                            eframe::egui::Pos2::new(w_f, y),
                            0.75,
                            line_col,
                        );
                        y += spacing;
                    }
                }
                crate::model::PageTemplate::Grid => {
                    let mut y = spacing;
                    while y < h_f {
                        draw_capsule_segment(
                            &mut img,
                            eframe::egui::Pos2::new(0.0, y),
                            eframe::egui::Pos2::new(w_f, y),
                            0.75,
                            line_col,
                        );
                        y += spacing;
                    }
                    let mut x = spacing;
                    while x < w_f {
                        draw_capsule_segment(
                            &mut img,
                            eframe::egui::Pos2::new(x, 0.0),
                            eframe::egui::Pos2::new(x, h_f),
                            0.75,
                            line_col,
                        );
                        x += spacing;
                    }
                }
                crate::model::PageTemplate::Dotted => {
                    let mut y = spacing;
                    while y < h_f {
                        let mut x = spacing;
                        while x < w_f {
                            let pt = eframe::egui::Pos2::new(x, y);
                            draw_capsule_segment(&mut img, pt, pt, 1.5, [204, 211, 218, 255]);
                            x += spacing;
                        }
                        y += spacing;
                    }
                }
                crate::model::PageTemplate::Cornell => {
                    let summary_y = h_f * 0.84;
                    let cue_x = w_f * 0.28;
                    let mut y = spacing * 1.5;
                    while y < summary_y {
                        draw_capsule_segment(
                            &mut img,
                            eframe::egui::Pos2::new(0.0, y),
                            eframe::egui::Pos2::new(w_f, y),
                            0.75,
                            [230, 235, 240, 255],
                        );
                        y += spacing;
                    }
                    draw_capsule_segment(
                        &mut img,
                        eframe::egui::Pos2::new(cue_x, h_f * 0.04),
                        eframe::egui::Pos2::new(cue_x, summary_y),
                        1.4,
                        [200, 210, 220, 255],
                    );
                    draw_capsule_segment(
                        &mut img,
                        eframe::egui::Pos2::new(0.0, summary_y),
                        eframe::egui::Pos2::new(w_f, summary_y),
                        1.4,
                        [200, 210, 220, 255],
                    );
                }
            }
        }
    }

    // 3. Render seluruh objek halaman (Stroke, Shape, Image, StickyNote, Text) sesuai urutan Z
    for obj in &page.objects {
        match obj {
            crate::model::DrawObject::Stroke {
                points,
                color,
                width,
                is_highlighter,
                ..
            } => {
                if points.is_empty() {
                    continue;
                }
                let smoothed = crate::geometry::smooth_stroke_points(points, 3);
                let rgba = if *is_highlighter {
                    [color.r, color.g, color.b, 95]
                } else {
                    [color.r, color.g, color.b, color.a]
                };
                if smoothed.len() == 1 {
                    let p = eframe::egui::Pos2::new(smoothed[0].x * scale, smoothed[0].y * scale);
                    let r = (width * scale * 0.5).max(1.0);
                    draw_capsule_segment(&mut img, p, p, r, rgba);
                } else if *is_highlighter {
                    // Untuk highlighter semi-transparan, rasterisasi per piksel dalam bounding box
                    // agar sambungan antar segmen tidak saling menumpuk / menghitam
                    let r = (width * scale * 0.5).max(1.5);
                    let mut min_x = f32::INFINITY;
                    let mut min_y = f32::INFINITY;
                    let mut max_x = f32::NEG_INFINITY;
                    let mut max_y = f32::NEG_INFINITY;
                    for pt in &smoothed {
                        min_x = min_x.min(pt.x * scale);
                        min_y = min_y.min(pt.y * scale);
                        max_x = max_x.max(pt.x * scale);
                        max_y = max_y.max(pt.y * scale);
                    }
                    let px_min = (min_x - r - 1.0).floor().max(0.0) as i32;
                    let px_max = (max_x + r + 1.0).ceil().min(width_px as f32 - 1.0) as i32;
                    let py_min = (min_y - r - 1.0).floor().max(0.0) as i32;
                    let py_max = (max_y + r + 1.0).ceil().min(height_px as f32 - 1.0) as i32;

                    for py in py_min..=py_max {
                        for px in px_min..=px_max {
                            let p = eframe::egui::Pos2::new(px as f32 + 0.5, py as f32 + 0.5);
                            let mut min_d = f32::INFINITY;
                            for w in smoothed.windows(2) {
                                let a = eframe::egui::Pos2::new(w[0].x * scale, w[0].y * scale);
                                let b = eframe::egui::Pos2::new(w[1].x * scale, w[1].y * scale);
                                min_d = min_d.min(crate::geometry::point_to_segment_dist(p, a, b));
                            }
                            if min_d <= r {
                                blend_pixel(&mut img, px, py, rgba);
                            }
                        }
                    }
                } else {
                    for w in smoothed.windows(2) {
                        let a = eframe::egui::Pos2::new(w[0].x * scale, w[0].y * scale);
                        let b = eframe::egui::Pos2::new(w[1].x * scale, w[1].y * scale);
                        let avg_p = ((w[0].pressure + w[1].pressure) * 0.5).clamp(0.1, 1.0);
                        let r = (width * scale * 0.5 * (0.4 + 0.6 * avg_p)).max(0.8);
                        draw_capsule_segment(&mut img, a, b, r, rgba);
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
                fill,
                ..
            } => {
                let stroke_rgba = [color.r, color.g, color.b, color.a];
                let r_stroke = (stroke_width * scale * 0.5).max(1.0);
                let p1 = eframe::egui::Pos2::new(x * scale, y * scale);
                let p2 = eframe::egui::Pos2::new((x + width) * scale, (y + height) * scale);
                let rect = eframe::egui::Rect::from_two_pos(p1, p2);

                match shape_type {
                    crate::model::ShapeKind::Line => {
                        draw_capsule_segment(&mut img, p1, p2, r_stroke, stroke_rgba);
                    }
                    crate::model::ShapeKind::Ellipse => {
                        let center = rect.center();
                        let radius = (rect.width().min(rect.height()) * 0.5).max(1.0);
                        let px_min = (center.x - radius - r_stroke - 1.0).floor().max(0.0) as i32;
                        let px_max = (center.x + radius + r_stroke + 1.0)
                            .ceil()
                            .min(width_px as f32 - 1.0) as i32;
                        let py_min = (center.y - radius - r_stroke - 1.0).floor().max(0.0) as i32;
                        let py_max = (center.y + radius + r_stroke + 1.0)
                            .ceil()
                            .min(height_px as f32 - 1.0)
                            as i32;

                        for py in py_min..=py_max {
                            for px in px_min..=px_max {
                                let p = eframe::egui::Pos2::new(px as f32 + 0.5, py as f32 + 0.5);
                                let d = (p - center).length();
                                if let Some(f) = fill
                                    && d <= radius
                                {
                                    blend_pixel(&mut img, px, py, [f.r, f.g, f.b, f.a]);
                                }
                                if (d - radius).abs() <= r_stroke {
                                    blend_pixel(&mut img, px, py, stroke_rgba);
                                }
                            }
                        }
                    }
                    _ => {
                        let verts: Vec<eframe::egui::Pos2> = crate::geometry::shape_vertices(
                            *shape_type,
                            *x * scale,
                            *y * scale,
                            *width * scale,
                            *height * scale,
                        );
                        if let Some(f) = fill
                            && verts.len() >= 3
                        {
                            let px_min = rect.min.x.floor().max(0.0) as i32;
                            let px_max = rect.max.x.ceil().min(width_px as f32 - 1.0) as i32;
                            let py_min = rect.min.y.floor().max(0.0) as i32;
                            let py_max = rect.max.y.ceil().min(height_px as f32 - 1.0) as i32;
                            let fill_rgba = [f.r, f.g, f.b, f.a];
                            for py in py_min..=py_max {
                                for px in px_min..=px_max {
                                    let p =
                                        eframe::egui::Pos2::new(px as f32 + 0.5, py as f32 + 0.5);
                                    if crate::geometry::point_in_polygon(p, &verts) {
                                        blend_pixel(&mut img, px, py, fill_rgba);
                                    }
                                }
                            }
                        }
                        for i in 0..verts.len() {
                            draw_capsule_segment(
                                &mut img,
                                verts[i],
                                verts[(i + 1) % verts.len()],
                                r_stroke,
                                stroke_rgba,
                            );
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
                if let Ok(bytes) = load_asset_from_notebook(dir, notebook_id, asset_id)
                    && let Ok(obj_img) = image::load_from_memory(&bytes)
                {
                    let rot_deg = (rotation.to_degrees().round() as i32).rem_euclid(360);
                    let rotated = match rot_deg {
                        90 => obj_img.rotate90(),
                        180 => obj_img.rotate180(),
                        270 => obj_img.rotate270(),
                        _ => obj_img,
                    };
                    let cx = *x + *width / 2.0;
                    let cy = *y + *height / 2.0;
                    let (bb_w, bb_h) = if rot_deg == 90 || rot_deg == 270 {
                        (*height, *width)
                    } else {
                        (*width, *height)
                    };
                    let ox = ((cx - bb_w / 2.0) * scale).round() as i64;
                    let oy = ((cy - bb_h / 2.0) * scale).round() as i64;
                    let ow = (bb_w * scale).round().max(1.0) as u32;
                    let oh = (bb_h * scale).round().max(1.0) as u32;
                    let resized =
                        rotated.resize_exact(ow, oh, image::imageops::FilterType::Lanczos3);
                    image::imageops::overlay(&mut img, &resized, ox, oy);
                }
            }
            crate::model::DrawObject::StickyNote {
                x,
                y,
                width,
                height,
                text,
                color,
                ..
            } => {
                let x0 = (*x * scale).round().max(0.0) as i32;
                let y0 = (*y * scale).round().max(0.0) as i32;
                let x1 = ((*x + *width) * scale).round().min(width_px as f32 - 1.0) as i32;
                let y1 = ((*y + *height) * scale).round().min(height_px as f32 - 1.0) as i32;
                let fill_rgba = [color.r, color.g, color.b, color.a];
                for py in y0..=y1 {
                    for px in x0..=x1 {
                        blend_pixel(&mut img, px, py, fill_rgba);
                    }
                }
                let tl = eframe::egui::Pos2::new(x0 as f32, y0 as f32);
                let tr = eframe::egui::Pos2::new(x1 as f32, y0 as f32);
                let br = eframe::egui::Pos2::new(x1 as f32, y1 as f32);
                let bl = eframe::egui::Pos2::new(x0 as f32, y1 as f32);
                let border_col = [235, 208, 116, 255];
                draw_capsule_segment(&mut img, tl, tr, 1.2, border_col);
                draw_capsule_segment(&mut img, tr, br, 1.2, border_col);
                draw_capsule_segment(&mut img, br, bl, 1.2, border_col);
                draw_capsule_segment(&mut img, bl, tl, 1.2, border_col);
                draw_egui_text(
                    &mut img,
                    &fonts,
                    *x + 10.0,
                    *y + 10.0,
                    text,
                    14.0,
                    [40, 40, 40, 255],
                    scale,
                );
            }
            crate::model::DrawObject::Text {
                x,
                y,
                text,
                font_size,
                color,
                ..
            } => {
                draw_egui_text(
                    &mut img,
                    &fonts,
                    *x,
                    *y,
                    text,
                    *font_size,
                    [color.r, color.g, color.b, color.a],
                    scale,
                );
            }
        }
    }

    img.save(output_path)
        .map_err(|e| format!("Gagal menyimpan berkas PNG: {e}"))
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

pub fn save_library(library: &Library) -> Result<(), String> {
    save_library_to_dir(&app_storage_dir(), library)
}

pub fn save_library_to_dir(dir: &Path, library: &Library) -> Result<(), String> {
    if library.schema_version > CURRENT_SCHEMA_VERSION {
        return Err(
            "Cannot overwrite file: library schema version is newer than application version."
                .to_string(),
        );
    }

    let current_path = dir.join("library.json");
    let previous_path = dir.join("library.previous.json");

    let mut to_save = library.clone();
    to_save.schema_version = CURRENT_SCHEMA_VERSION;

    let content = serde_json::to_string_pretty(&to_save)
        .map_err(|e| format!("Failed to serialize library: {e}"))?;

    atomic_write_file(&current_path, &content, Some(&previous_path))
}

// --- Fase 5: Custom Templates Storage ---

pub fn save_custom_template(
    base_dir: &Path,
    tmpl: &crate::model::CustomTemplate,
) -> Result<(), String> {
    let t_dir = base_dir.join("templates");
    let _ = fs::create_dir_all(&t_dir);
    let path = t_dir.join(format!("{}.json", tmpl.id));
    let json = serde_json::to_string_pretty(tmpl).map_err(|e| e.to_string())?;
    fs::write(&path, json).map_err(|e| e.to_string())
}

pub fn load_custom_templates(base_dir: &Path) -> Vec<crate::model::CustomTemplate> {
    let t_dir = base_dir.join("templates");
    if !t_dir.exists() {
        return Vec::new();
    }
    let mut list = Vec::new();
    if let Ok(entries) = fs::read_dir(t_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json")
                && let Ok(content) = fs::read_to_string(&path)
                && let Ok(tmpl) = serde_json::from_str::<crate::model::CustomTemplate>(&content)
            {
                list.push(tmpl);
            }
        }
    }
    list
}

pub fn delete_custom_template(base_dir: &Path, template_id: &str) -> Result<(), String> {
    let path = base_dir
        .join("templates")
        .join(format!("{template_id}.json"));
    if path.exists() {
        fs::remove_file(path).map_err(|e| e.to_string())
    } else {
        Ok(())
    }
}

// --- Fase 8.4 & 8.7: User Settings Storage (`settings.json`) ---

pub fn save_settings_to_dir(
    base_dir: &Path,
    settings: &crate::model::AppSettings,
) -> Result<(), String> {
    let _ = fs::create_dir_all(base_dir);
    let path = base_dir.join("settings.json");
    let json = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    atomic_write_file(&path, &json, None)
}

pub fn load_settings_from_dir(base_dir: &Path) -> crate::model::AppSettings {
    let path = base_dir.join("settings.json");
    if let Ok(content) = fs::read_to_string(&path)
        && let Ok(settings) = serde_json::from_str::<crate::model::AppSettings>(&content)
    {
        return settings;
    }
    crate::model::AppSettings::default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{CURRENT_SCHEMA_VERSION, Library, Notebook, PageTemplate};
    use eframe::egui::Color32;

    #[test]
    fn test_migrate_v1_to_v2() {
        let raw = serde_json::json!({
            "version": 1,
            "notebooks": [],
            "folders": []
        });

        let lib = migrate(raw).expect("migrate v1");
        assert_eq!(lib.schema_version, CURRENT_SCHEMA_VERSION);
    }

    #[test]
    fn test_migrate_rejects_future_version() {
        let future_ver = CURRENT_SCHEMA_VERSION + 99;
        let raw = serde_json::json!({
            "schema_version": future_ver,
            "notebooks": [],
            "folders": []
        });

        let res = migrate(raw);
        assert!(res.is_err());
    }

    #[test]
    fn test_storage_normal_save_and_load() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path();

        let mut lib = Library::default();
        lib.notebooks.push(Notebook::new(
            "Temp Note",
            PageTemplate::Grid,
            Color32::from_rgb(10, 20, 30),
        ));

        save_library_to_dir(dir, &lib).expect("save");
        let load_res = load_library_from_dir(dir);

        assert_eq!(load_res.status, LoadStatus::LoadedDirect);
        assert_eq!(load_res.library.notebooks.len(), 1);
        assert_eq!(load_res.library.notebooks[0].title, "Temp Note");
    }

    #[test]
    fn test_storage_recovery_from_previous_when_current_corrupt() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path();

        let mut lib1 = Library::default();
        lib1.notebooks.push(Notebook::new(
            "Version 1 Note",
            PageTemplate::Ruled,
            Color32::from_rgb(10, 20, 30),
        ));
        save_library_to_dir(dir, &lib1).expect("save first");

        let mut lib2 = lib1.clone();
        lib2.notebooks.push(Notebook::new(
            "Version 2 Note",
            PageTemplate::Blank,
            Color32::from_rgb(40, 50, 60),
        ));
        save_library_to_dir(dir, &lib2).expect("save second");

        // Now library.json has lib2, and library.previous.json has lib1!
        // Corrupt current library.json:
        fs::write(dir.join("library.json"), "{ NOT A VALID JSON :::").expect("write corrupt");

        // Load should recover from previous:
        let load_res = load_library_from_dir(dir);
        assert_eq!(load_res.status, LoadStatus::RecoveredFromPrevious);
        assert_eq!(load_res.library.notebooks.len(), 1);
        assert_eq!(load_res.library.notebooks[0].title, "Version 1 Note");
    }

    #[test]
    fn test_storage_quarantine_when_both_corrupt() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path();

        // Write corrupt data to both
        fs::write(dir.join("library.json"), "CORRUPT JSON 1").expect("write corrupt");
        fs::write(dir.join("library.previous.json"), "CORRUPT JSON 2").expect("write corrupt");

        let load_res = load_library_from_dir(dir);
        match load_res.status {
            LoadStatus::CorruptedQuarantined { path } => {
                assert!(path.exists());
                assert!(path.to_string_lossy().contains("library.corrupt."));
            }
            other => panic!("Expected CorruptedQuarantined, got {:?}", other),
        }

        // Library returned should be empty fresh library
        assert_eq!(load_res.library.notebooks.len(), 0);
    }

    #[test]
    fn test_storage_future_version_readonly_and_no_overwrite() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path();

        let future_json = serde_json::json!({
            "schema_version": 999,
            "notebooks": [],
            "folders": []
        });
        fs::write(dir.join("library.json"), future_json.to_string()).expect("write future json");

        let load_res = load_library_from_dir(dir);
        match load_res.status {
            LoadStatus::NewerVersionReadOnly { version } => {
                assert_eq!(version, 999);
            }
            other => panic!("Expected NewerVersionReadOnly, got {:?}", other),
        }

        // Trying to save should fail
        let save_res = save_library_to_dir(dir, &load_res.library);
        assert!(save_res.is_err());
    }

    #[test]
    fn test_notebook_atomic_save_and_load() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path();

        let nb = Notebook::new("Fase 6 Test Book", PageTemplate::Grid, Color32::RED);
        let nb_id = nb.id.clone();

        save_notebook_to_dir(dir, &nb).expect("save notebook");
        assert!(
            dir.join("notebooks")
                .join(&nb_id)
                .join("notebook.json")
                .exists()
        );

        let loaded = load_notebook_from_dir(dir, &nb_id).expect("load notebook");
        assert_eq!(loaded.id, nb_id);
        assert_eq!(loaded.title, "Fase 6 Test Book");
    }

    #[test]
    fn test_notebook_recovery_and_quarantine() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path();

        let mut nb1 = Notebook::new("V1", PageTemplate::Blank, Color32::BLUE);
        let nb_id = nb1.id.clone();
        save_notebook_to_dir(dir, &nb1).expect("save v1");

        nb1.title = "V2".to_string();
        save_notebook_to_dir(dir, &nb1).expect("save v2");

        let nb_file = dir.join("notebooks").join(&nb_id).join("notebook.json");
        fs::write(&nb_file, "{ corrupted JSON...").expect("corrupt current");

        // Should recover from previous
        let rec = load_notebook_from_dir(dir, &nb_id).expect("recover");
        assert_eq!(rec.title, "V1");

        // Now corrupt both
        let prev_file = dir
            .join("notebooks")
            .join(&nb_id)
            .join("notebook.previous.json");
        fs::write(&nb_file, "{ bad1").expect("corrupt 1");
        fs::write(&prev_file, "{ bad2").expect("corrupt 2");

        let err = load_notebook_from_dir(dir, &nb_id);
        assert!(err.is_err());
        assert!(err.unwrap_err().contains("quarantined"));
    }

    #[test]
    fn test_library_index_migration_from_legacy() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path();

        // 1. Setup legacy library.json with 2 notebooks
        let mut legacy_lib = Library::default();
        legacy_lib.notebooks.push(Notebook::new(
            "Old Book 1",
            PageTemplate::Ruled,
            Color32::GREEN,
        ));
        legacy_lib.notebooks.push(Notebook::new(
            "Old Book 2",
            PageTemplate::Grid,
            Color32::KHAKI,
        ));
        let nb1_id = legacy_lib.notebooks[0].id.clone();
        let nb2_id = legacy_lib.notebooks[1].id.clone();

        save_library_to_dir(dir, &legacy_lib).expect("save legacy");
        assert!(dir.join("library.json").exists());
        assert!(!dir.join("library_index.json").exists());

        // 2. Load index - should trigger automatic migration
        let index_res = load_library_index_from_dir(dir);
        assert_eq!(index_res.status, LoadIndexStatus::MigratedFromLegacy);
        assert_eq!(index_res.index.notebooks.len(), 2);
        assert!(dir.join("library_index.json").exists());
        assert!(!dir.join("library.json").exists()); // Old file safely moved to backup
        assert!(dir.join("backup").exists());

        // 3. Verify individual notebook files exist on disk
        let nb1 = load_notebook_from_dir(dir, &nb1_id).expect("load nb1");
        assert_eq!(nb1.title, "Old Book 1");
        let nb2 = load_notebook_from_dir(dir, &nb2_id).expect("load nb2");
        assert_eq!(nb2.title, "Old Book 2");
    }

    #[test]
    fn test_notebook_zip_export_and_import() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path();

        let nb = Notebook::new("Zip Export Test", PageTemplate::Cornell, Color32::BLUE);
        let nb_id = nb.id.clone();
        save_notebook_to_dir(dir, &nb).expect("save");

        // Update index
        let mut index = LibraryIndex::default();
        index.notebooks.push(NotebookMeta::from_notebook(&nb));
        save_library_index_to_dir(dir, &index).expect("save index");

        let zip_path = dir.join("exported_notebook.zip");
        export_notebook_to_zip(dir, &nb_id, &zip_path).expect("export zip");
        assert!(zip_path.exists());

        // Now import into a separate directory
        let tmp_dest = tempfile::tempdir().expect("tempdir dest");
        let dest_dir = tmp_dest.path();

        let imported = import_notebook_from_zip(dest_dir, &zip_path).expect("import zip");
        assert_eq!(imported.id, nb_id);
        assert_eq!(imported.title, "Zip Export Test");

        let dest_index = load_library_index_from_dir(dest_dir);
        assert_eq!(dest_index.index.notebooks.len(), 1);
        assert_eq!(dest_index.index.notebooks[0].title, "Zip Export Test");
    }

    #[test]
    fn test_asset_saving_and_loading() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path();
        let nb_id = "test-nb-assets";

        let sample_data = b"FAKE_IMAGE_DATA_123456";
        let asset_id = save_asset_to_notebook(dir, nb_id, sample_data, "png").expect("save asset");
        assert!(asset_id.ends_with(".png"));

        let loaded = load_asset_from_notebook(dir, nb_id, &asset_id).expect("load asset");
        assert_eq!(loaded, sample_data);
    }

    #[test]
    fn test_export_page_to_png() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path();
        let nb_id = "png-export-test";

        let mut page = Page::with_size(PageTemplate::Blank, 200.0, 300.0);
        // Tambahkan coretan (Stroke) merah di (50, 50) -> (150, 50)
        page.objects.push(crate::model::DrawObject::Stroke {
            id: "s-export-1".to_string(),
            points: vec![
                crate::model::StrokePoint {
                    x: 50.0,
                    y: 50.0,
                    pressure: 1.0,
                },
                crate::model::StrokePoint {
                    x: 150.0,
                    y: 50.0,
                    pressure: 1.0,
                },
            ],
            color: crate::model::ColorData {
                r: 220,
                g: 20,
                b: 20,
                a: 255,
            },
            width: 6.0,
            is_highlighter: false,
        });
        // Tambahkan teks di (40, 100)
        page.objects.push(crate::model::DrawObject::Text {
            id: "t-export-1".to_string(),
            x: 40.0,
            y: 100.0,
            text: "HELLO".to_string(),
            font_size: 20.0,
            color: crate::model::ColorData {
                r: 10,
                g: 20,
                b: 200,
                a: 255,
            },
        });

        let out_png = dir.join("test_page.png");
        export_page_to_png(dir, nb_id, &page, &out_png).expect("export png");
        assert!(out_png.exists());
        let img = image::open(&out_png).expect("load exported png").to_rgba8();
        // Pada skala 2x, titik tengah coretan (100, 50) berada di piksel (200, 100) dan harus berwarna merah
        let px = img.get_pixel(200, 100);
        assert_eq!(px[0], 220);
        assert_eq!(px[1], 20);
        assert_eq!(px[2], 20);
    }

    #[test]
    fn test_export_notebook_to_pdf_and_reimport() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path();

        let nb = Notebook::new("PDF Roundtrip", PageTemplate::Blank, Color32::WHITE);
        let out_pdf = dir.join("exported.pdf");

        // Export to PDF
        let export_res = export_notebook_to_pdf(dir, &nb, &out_pdf);
        // If pdfium.dll is available, export should succeed and re-import should work!
        if export_res.is_ok() {
            assert!(out_pdf.exists());
            let pdf_bytes = fs::read(&out_pdf).expect("read exported pdf");
            let imported_pages =
                import_pdf_to_pages(dir, &nb.id, &pdf_bytes).expect("import pages");
            assert_eq!(imported_pages.len(), nb.pages.len());
            assert!(imported_pages[0].background.is_some());
        }
    }

    #[test]
    fn test_custom_template_storage_roundtrip() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path();

        let tmpl = crate::model::CustomTemplate {
            id: "tpl-abc-123".to_string(),
            name: "Template Sketsa".to_string(),
            template: PageTemplate::Dotted,
            background: None,
            paper_color: crate::model::ColorData {
                r: 240,
                g: 240,
                b: 240,
                a: 255,
            },
            width: 800.0,
            height: 1100.0,
        };

        // 1. Save
        save_custom_template(dir, &tmpl).expect("save custom template");

        // 2. Load
        let loaded = load_custom_templates(dir);
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, tmpl.id);
        assert_eq!(loaded[0].name, tmpl.name);

        // 3. Delete
        delete_custom_template(dir, &tmpl.id).expect("delete custom template");
        let after_del = load_custom_templates(dir);
        assert!(after_del.is_empty());
    }

    #[test]
    fn test_export_and_import_rnote() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path();

        let nb = Notebook::new(
            "Catatan Resmi",
            PageTemplate::Ruled,
            Color32::from_rgb(40, 72, 114),
        );
        save_notebook_to_dir(dir, &nb).expect("save notebook");

        let rnote_file = dir.join("catatan.rnote");
        export_notebook_to_rnote(dir, &nb.id, &rnote_file).expect("export rnote");
        assert!(rnote_file.exists());

        // Impor kembali
        let imported = import_notebook_from_rnote(dir, &rnote_file).expect("import rnote");
        assert_eq!(imported.id, nb.id);
        assert_eq!(imported.title, nb.title);
        assert_eq!(imported.pages.len(), nb.pages.len());
    }

    #[test]
    fn test_settings_save_and_load() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path();

        // Default jika file belum ada
        let initial = load_settings_from_dir(dir);
        assert_eq!(initial.theme_mode, crate::model::ThemeMode::Dark);

        // Ubah dan simpan
        let mut custom = initial;
        custom.theme_mode = crate::model::ThemeMode::Light;
        custom.pen_widths = vec![2.0, 4.0, 6.0];
        save_settings_to_dir(dir, &custom).expect("save settings");

        let reloaded = load_settings_from_dir(dir);
        assert_eq!(reloaded.theme_mode, crate::model::ThemeMode::Light);
        assert_eq!(reloaded.pen_widths, vec![2.0, 4.0, 6.0]);
    }
}

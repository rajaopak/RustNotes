use super::library_store::{load_library_index_from_dir, save_library_index_to_dir};
use super::notebook_store::{load_notebook_from_dir, save_notebook_to_dir};
use crate::model::{CURRENT_SCHEMA_VERSION, Notebook, NotebookMeta};
use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use zip::write::SimpleFileOptions;

pub fn export_notebook_to_zip(
    dir: &Path,
    notebook_id: &str,
    zip_dest: &Path,
) -> Result<(), String> {
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

/// Mengimpor buku catatan dari berkas arsip zip.
/// Dilengkapi perlindungan:
/// 1. Sanitasi ID mencegah Path Traversal (`../../`).
/// 2. Deteksi tabrakan ID (Opsi 1A): Jika ID sudah ada di library lokal, otomatis buat UUID baru dan beri penanda "(Imported)".
/// 3. Pencegahan Zip Bomb / OOM dengan pembatasan ukuran dekompresi (50MB per asset, 500MB total).
/// 4. Sanitasi nama berkas asset.
pub fn import_notebook_from_zip(dir: &Path, zip_source: &Path) -> Result<Notebook, String> {
    const MAX_NOTEBOOK_JSON_BYTES: u64 = 20 * 1024 * 1024; // 20MB
    const MAX_ASSET_BYTES: u64 = 50 * 1024 * 1024; // 50MB per asset
    const MAX_TOTAL_DECOMPRESSED_BYTES: u64 = 500 * 1024 * 1024; // 500MB total

    let file =
        fs::File::open(zip_source).map_err(|e| format!("Failed to open zip archive: {e}"))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("Failed to read zip archive: {e}"))?;

    // 1. Baca notebook.json dengan pembatasan ukuran
    let nb_file = archive
        .by_name("notebook.json")
        .map_err(|e| format!("Archive missing notebook.json: {e}"))?;
    let mut content = String::new();
    nb_file
        .take(MAX_NOTEBOOK_JSON_BYTES + 1)
        .read_to_string(&mut content)
        .map_err(|e| format!("Failed to read notebook.json from archive: {e}"))?;
    if content.len() as u64 > MAX_NOTEBOOK_JSON_BYTES {
        return Err("notebook.json in archive exceeds maximum allowed size (20MB)".to_string());
    }

    let mut nb: Notebook =
        serde_json::from_str(&content).map_err(|e| format!("Invalid notebook.json in zip: {e}"))?;

    // 2. Sanitasi ID terhadap path traversal
    let is_safe_id = !nb.id.is_empty()
        && nb
            .id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        && !nb.id.contains("..");

    if !is_safe_id {
        // ID tidak aman, regenerasi ke UUID valid
        nb.id = uuid::Uuid::new_v4().to_string();
    }

    // 3. Resolusi tabrakan ID (Opsi 1A): Jangan timpa notebook pengguna yang sudah ada
    let existing_target = dir.join("notebooks").join(&nb.id);
    if existing_target.exists() {
        nb.id = uuid::Uuid::new_v4().to_string();
        nb.title = format!("{} (Imported)", nb.title);
    }

    // Pastikan direktori tujuan tersedia
    let nb_dir = dir.join("notebooks").join(&nb.id);
    let assets_dir = nb_dir.join("assets");
    let _ = fs::create_dir_all(&assets_dir);

    // 4. Ekstraksi aset dengan sanitasi nama berkas dan proteksi Zip Bomb
    let mut total_decompressed: u64 = content.len() as u64;

    for i in 0..archive.len() {
        let entry = archive
            .by_index(i)
            .map_err(|e| format!("Zip entry error: {e}"))?;
        let entry_name = entry.name().to_string();

        if entry_name.starts_with("assets/") && !entry.is_dir() {
            let Some(file_name) = Path::new(&entry_name).file_name().and_then(|f| f.to_str())
            else {
                continue;
            };

            // Validasi nama berkas aset
            let is_safe_filename = !file_name.is_empty()
                && !file_name.starts_with('.')
                && file_name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_')
                && !file_name.contains("..");

            if !is_safe_filename {
                return Err(format!("Unsafe asset filename detected: {file_name}"));
            }

            let dest_path = assets_dir.join(file_name);
            let mut buf = Vec::new();
            entry
                .take(MAX_ASSET_BYTES + 1)
                .read_to_end(&mut buf)
                .map_err(|e| format!("Read asset error: {e}"))?;

            if buf.len() as u64 > MAX_ASSET_BYTES {
                return Err(format!(
                    "Asset '{file_name}' exceeds maximum allowed size (50MB)"
                ));
            }

            total_decompressed += buf.len() as u64;
            if total_decompressed > MAX_TOTAL_DECOMPRESSED_BYTES {
                return Err("Archive total uncompressed content exceeds limit (500MB)".to_string());
            }

            let mut out =
                fs::File::create(dest_path).map_err(|e| format!("Write asset error: {e}"))?;
            out.write_all(&buf)
                .map_err(|e| format!("Asset write error: {e}"))?;
        }
    }

    // 5. Simpan notebook ke disk
    save_notebook_to_dir(dir, &nb)?;

    // 6. Perbarui library index
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

/// Impor buku catatan dari berkas `.rnote`.
pub fn import_notebook_from_rnote(dir: &Path, rnote_source: &Path) -> Result<Notebook, String> {
    import_notebook_from_zip(dir, rnote_source)
}

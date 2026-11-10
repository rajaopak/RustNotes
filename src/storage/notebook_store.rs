use super::io::{atomic_write_file, rotate_notebook_backups};
use crate::model::{CURRENT_NOTEBOOK_SCHEMA_VERSION, Notebook};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

pub fn save_notebook_to_dir(dir: &Path, notebook: &Notebook) -> Result<(), String> {
    let nb_dir = dir.join("notebooks").join(&notebook.id);
    let _ = fs::create_dir_all(&nb_dir);
    let _ = fs::create_dir_all(nb_dir.join("assets"));

    let target_path = nb_dir.join("notebook.json");
    let previous_path = nb_dir.join("notebook.previous.json");

    // Pastikan rotasi backup 3 generasi dilakukan sebelum penulisan berkas baru
    rotate_notebook_backups(&nb_dir, 3);

    let mut nb_to_save = notebook.clone();
    nb_to_save.schema_version = CURRENT_NOTEBOOK_SCHEMA_VERSION;

    let content = serde_json::to_string_pretty(&nb_to_save)
        .map_err(|e| format!("Failed to serialize notebook {}: {e}", notebook.id))?;

    atomic_write_file(&target_path, &content, Some(&previous_path))
}

pub fn load_notebook_from_dir(dir: &Path, notebook_id: &str) -> Result<Notebook, String> {
    let nb_dir = dir.join("notebooks").join(notebook_id);
    let target_path = nb_dir.join("notebook.json");

    if !nb_dir.exists() {
        return Err(format!("Notebook {} not found", notebook_id));
    }

    enum ReadOutcome {
        Success(Notebook),
        NewerVersion(u32),
        IoError(String),
        Corrupt(String),
    }

    let try_read = |p: &Path| -> ReadOutcome {
        if !p.exists() {
            return ReadOutcome::IoError("File does not exist".to_string());
        }
        match fs::read_to_string(p) {
            Ok(content) => match serde_json::from_str::<Notebook>(&content) {
                Ok(nb) => {
                    if nb.schema_version > CURRENT_NOTEBOOK_SCHEMA_VERSION {
                        ReadOutcome::NewerVersion(nb.schema_version)
                    } else {
                        ReadOutcome::Success(nb)
                    }
                }
                Err(e) => ReadOutcome::Corrupt(format!("JSON error: {e}")),
            },
            Err(e) => ReadOutcome::IoError(format!("I/O read error: {e}")),
        }
    };

    // 1. Coba baca notebook.json utama
    match try_read(&target_path) {
        ReadOutcome::Success(nb) => return Ok(nb),
        ReadOutcome::NewerVersion(ver) => {
            return Err(format!(
                "Notebook '{notebook_id}' dibuat dengan versi RustNotes yang lebih baru (schema v{ver}). Harap perbarui aplikasi Anda."
            ));
        }
        ReadOutcome::IoError(e) => {
            // Transient error (misal file lock) — jangan langsung anggap korup!
            // Coba backup generasi jika ada
            for backup_name in &[
                "notebook.bak.1.json",
                "notebook.previous.json",
                "notebook.bak.2.json",
                "notebook.bak.3.json",
            ] {
                let backup_path = nb_dir.join(backup_name);
                if let ReadOutcome::Success(nb) = try_read(&backup_path) {
                    return Ok(nb);
                }
            }
            return Err(format!("Gagal membaca notebook '{notebook_id}': {e}"));
        }
        ReadOutcome::Corrupt(err_msg) => {
            eprintln!(
                "[Storage] Berkas utama notebook '{notebook_id}' korup: {err_msg}. Mencoba pemulihan backup..."
            );
            // 2. Coba pulihkan dari backup rolling generasi
            for backup_name in &[
                "notebook.bak.1.json",
                "notebook.previous.json",
                "notebook.bak.2.json",
                "notebook.bak.3.json",
            ] {
                let backup_path = nb_dir.join(backup_name);
                if let ReadOutcome::Success(nb) = try_read(&backup_path) {
                    eprintln!(
                        "[Storage] Berhasil memulihkan notebook '{notebook_id}' dari {backup_name}"
                    );
                    let _ = save_notebook_to_dir(dir, &nb);
                    return Ok(nb);
                }
            }

            // 3. Hanya karantina jika berkas utama benar-benar terbaca tapi rusak secara sintaksis dan seluruh backup gagal
            if target_path.exists() {
                let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string();
                let corrupt_path = nb_dir.join(format!("notebook.corrupt.{timestamp}.json"));
                let _ = fs::rename(&target_path, &corrupt_path);
                return Err(format!(
                    "Notebook corrupted; quarantined to {}",
                    corrupt_path.display()
                ));
            }
        }
    }

    Err(format!("Notebook {} not found", notebook_id))
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

/// Menyalin seluruh aset gambar/PDF dari direktori notebook sumber ke direktori notebook target.
pub fn copy_notebook_assets(
    dir: &Path,
    source_notebook_id: &str,
    target_notebook_id: &str,
) -> Result<(), String> {
    let src_assets = dir
        .join("notebooks")
        .join(source_notebook_id)
        .join("assets");
    let dst_assets = dir
        .join("notebooks")
        .join(target_notebook_id)
        .join("assets");

    if !src_assets.exists() {
        let _ = fs::create_dir_all(&dst_assets);
        return Ok(());
    }

    fs::create_dir_all(&dst_assets)
        .map_err(|e| format!("Gagal membuat direktori aset tujuan: {e}"))?;

    let entries = fs::read_dir(&src_assets)
        .map_err(|e| format!("Gagal membaca direktori aset sumber: {e}"))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("Gagal membaca item aset: {e}"))?;
        let file_type = entry
            .file_type()
            .map_err(|e| format!("Gagal membaca tipe berkas aset: {e}"))?;
        if file_type.is_file() {
            let file_name = entry.file_name();
            let src_path = entry.path();
            let dst_path = dst_assets.join(&file_name);
            fs::copy(&src_path, &dst_path)
                .map_err(|e| format!("Gagal menyalin aset {}: {e}", file_name.to_string_lossy()))?;
        }
    }

    Ok(())
}

/// Menduplikasi buku catatan di tingkat disk:
/// 1. Menyimpan data notebook klon ke `notebooks/<new_id>/notebook.json`.
/// 2. Menyalin seluruh aset fisik dari buku sumber ke `notebooks/<new_id>/assets/`.
/// 3. Rollback otomatis (menghapus folder klon) jika terjadi kegagalan.
pub fn duplicate_notebook_storage(
    dir: &Path,
    source_notebook_id: &str,
    target_notebook: &Notebook,
) -> Result<(), String> {
    let target_nb_dir = dir.join("notebooks").join(&target_notebook.id);

    // 1. Simpan notebook.json target
    if let Err(e) = save_notebook_to_dir(dir, target_notebook) {
        if target_nb_dir.exists() {
            let _ = fs::remove_dir_all(&target_nb_dir);
        }
        return Err(format!("Gagal menyimpan berkas buku catatan duplikat: {e}"));
    }

    // 2. Salin seluruh aset
    if let Err(e) = copy_notebook_assets(dir, source_notebook_id, &target_notebook.id) {
        if target_nb_dir.exists() {
            let _ = fs::remove_dir_all(&target_nb_dir);
        }
        return Err(format!("Gagal menyalin aset buku catatan duplikat: {e}"));
    }

    Ok(())
}

/// Memvalidasi seluruh referensi aset di dalam notebook apakah berkas fisiknya ada di disk.
/// Mengembalikan daftar `asset_id` yang hilang (jika ada).
pub fn validate_notebook_assets(dir: &Path, notebook: &Notebook) -> Vec<String> {
    let assets_dir = dir.join("notebooks").join(&notebook.id).join("assets");
    let mut missing = Vec::new();
    let mut checked = std::collections::HashSet::new();

    for page in &notebook.pages {
        // Cek background
        match page.effective_background() {
            crate::model::PageBackground::Image { asset_id }
            | crate::model::PageBackground::Pdf { asset_id, .. }
                if checked.insert(asset_id.clone()) =>
            {
                let asset_path = assets_dir.join(&asset_id);
                if !asset_path.exists() {
                    missing.push(asset_id);
                }
            }
            _ => {}
        }

        // Cek objek
        for obj in &page.objects {
            if let crate::model::DrawObject::Image { asset_id, .. } = obj
                && checked.insert(asset_id.clone())
            {
                let asset_path = assets_dir.join(asset_id);
                if !asset_path.exists() {
                    missing.push(asset_id.clone());
                }
            }
        }
    }

    missing
}

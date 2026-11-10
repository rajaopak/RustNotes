use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Direktori penyimpanan lokal default untuk data RustNotes.
///
/// Binary aplikasi (debug maupun release) memakai `%LOCALAPPDATA%\RustNotes`.
#[cfg(not(test))]
pub fn app_storage_dir() -> PathBuf {
    user_storage_dir()
}

/// Build unit test (`cargo test`) memakai direktori sementara yang terisolasi
/// per-thread test, sehingga pengujian tidak pernah membaca maupun menulis
/// library milik pengguna (mis. membuat notebook uji di aplikasi release).
#[cfg(test)]
pub fn app_storage_dir() -> PathBuf {
    test_isolation::thread_storage_dir()
}

/// Lokasi data pengguna yang sebenarnya di sistem operasi.
fn user_storage_dir() -> PathBuf {
    dirs::data_local_dir()
        .map(|p| p.join("RustNotes"))
        .unwrap_or_else(|| PathBuf::from("./rustnotes_data"))
}

/// Rotasi berkas backup notebook hingga `max_generations` generasi.
/// Menyimpan `notebook.bak.1.json`, `notebook.bak.2.json`, `notebook.bak.3.json`,
/// dan memperbarui `notebook.previous.json` sebagai alias generasi 1.
pub fn rotate_notebook_backups(nb_dir: &Path, max_generations: usize) {
    let target_path = nb_dir.join("notebook.json");
    if !target_path.exists() {
        return;
    }

    // Rotasi generasi mundur: e.g. 2 -> 3, 1 -> 2
    for generation in (1..max_generations).rev() {
        let src = nb_dir.join(format!("notebook.bak.{generation}.json"));
        let dst = nb_dir.join(format!("notebook.bak.{}.json", generation + 1));
        if src.exists() {
            let _ = fs::rename(&src, &dst);
        }
    }

    // Target saat ini disalin sebagai generasi 1 dan alias previous
    let bak1 = nb_dir.join("notebook.bak.1.json");
    let prev = nb_dir.join("notebook.previous.json");
    let _ = fs::copy(&target_path, &bak1);
    let _ = fs::copy(&target_path, &prev);
}

/// Penulisan berkas secara atomik menggunakan file temporer dan rename.
/// Menjamin berkas target tidak terpotong atau korup jika terjadi crash di tengah proses.
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

#[cfg(test)]
mod test_isolation {
    use std::path::PathBuf;

    thread_local! {
        /// Libtest menjalankan setiap test pada thread tersendiri, sehingga setiap
        /// test memperoleh root storage unik. Direktori dihapus otomatis saat
        /// thread selesai (drop `TempDir`).
        static STORAGE_ROOT: tempfile::TempDir = tempfile::Builder::new()
            .prefix("rustnotes-test-")
            .tempdir()
            .expect("failed to create isolated test storage directory");
    }

    pub(super) fn thread_storage_dir() -> PathBuf {
        STORAGE_ROOT.with(|root| root.path().to_path_buf())
    }

    #[test]
    fn test_storage_dir_is_isolated_from_user_data() {
        let test_dir = super::app_storage_dir();
        assert_ne!(test_dir, super::user_storage_dir());
        assert!(test_dir.starts_with(std::env::temp_dir()));
        // Stabil dalam satu test (thread) yang sama.
        assert_eq!(test_dir, super::app_storage_dir());
    }

    #[test]
    fn test_storage_dir_is_unique_per_test_thread() {
        let here = super::app_storage_dir();
        let other = std::thread::spawn(super::app_storage_dir)
            .join()
            .expect("thread panicked");
        assert_ne!(here, other);
    }
}

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
    atomic_write_file_with_hook(target_path, content, backup_path, None)
}

/// Penulisan berkas secara atomik dengan hook opsional yang dijalankan tepat setelah
/// berkas temporer selesai ditulis dan di-sync, sesaat sebelum penggantian target.
pub fn atomic_write_file_with_hook(
    target_path: &Path,
    content: &str,
    backup_path: Option<&Path>,
    pre_replace_hook: Option<&dyn Fn()>,
) -> Result<(), String> {
    if target_path.is_dir() {
        return Err(format!(
            "Target path {} is a directory",
            target_path.display()
        ));
    }
    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            format!(
                "Failed to create parent directory {}: {e}",
                parent.display()
            )
        })?;
    }
    let unique_id = uuid::Uuid::new_v4();
    let temp_path = target_path.with_extension(format!("tmp.{}", unique_id));

    let write_res = (|| -> Result<(), String> {
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
        Ok(())
    })();

    if let Err(e) = write_res {
        let _ = fs::remove_file(&temp_path);
        return Err(e);
    }

    if let Some(hook) = pre_replace_hook {
        hook();
    }

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

    // Strategi penggantian dua tahap yang aman untuk Windows apabila rename langsung gagal:
    // pindahkan target ke rollback sementara, pindahkan temp ke target, lalu hapus rollback.
    // Jika langkah kedua gagal, pulihkan rollback agar target sebelumnya tetap utuh.
    if !rename_success && target_path.exists() {
        let rollback_path = target_path.with_extension(format!("rollback.{}", unique_id));
        if fs::rename(target_path, &rollback_path).is_ok() {
            let mut replaced = false;
            for _ in 0..3 {
                if fs::rename(&temp_path, target_path).is_ok() {
                    replaced = true;
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(30));
            }
            if replaced {
                let _ = fs::remove_file(&rollback_path);
                rename_success = true;
            } else {
                let _ = fs::rename(&rollback_path, target_path);
            }
        }
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

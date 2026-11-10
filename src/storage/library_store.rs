use super::io::atomic_write_file;
use super::notebook_store::save_notebook_to_dir;
use crate::model::{CURRENT_SCHEMA_VERSION, Library, LibraryIndex, NotebookMeta};
use std::fs;
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

    if version < CURRENT_SCHEMA_VERSION
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
    load_library_from_dir(&super::io::app_storage_dir())
}

pub fn load_library_from_dir(dir: &Path) -> LoadResult {
    let current_path = dir.join("library.json");
    let previous_path = dir.join("library.previous.json");

    // 1. Try reading and migrating current library.json
    if current_path.exists()
        && let Ok(content) = fs::read_to_string(&current_path)
        && let Ok(value) = serde_json::from_str::<serde_json::Value>(&content)
    {
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
        let _ = save_library_to_dir(dir, &lib);
        return LoadResult {
            library: lib,
            status: LoadStatus::RecoveredFromPrevious,
        };
    }

    // 3. Both failed. Quarantine corrupt file if it exists
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

pub fn save_library(library: &Library) -> Result<(), String> {
    save_library_to_dir(&super::io::app_storage_dir(), library)
}

pub fn save_library_to_dir(dir: &Path, library: &Library) -> Result<(), String> {
    if library.schema_version > CURRENT_SCHEMA_VERSION {
        return Err(format!(
            "Cannot save library: schema version {} is newer than supported {}.",
            library.schema_version, CURRENT_SCHEMA_VERSION
        ));
    }
    let target_path = dir.join("library.json");
    let previous_path = dir.join("library.previous.json");
    let content = serde_json::to_string_pretty(library)
        .map_err(|e| format!("Failed to serialize library: {e}"))?;
    atomic_write_file(&target_path, &content, Some(&previous_path))
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

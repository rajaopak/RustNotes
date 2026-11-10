use super::io::atomic_write_file;
use crate::model::{AppSettings, CustomTemplate};
use std::fs;
use std::path::Path;

pub fn save_custom_template(base_dir: &Path, tmpl: &CustomTemplate) -> Result<(), String> {
    let t_dir = base_dir.join("templates");
    let _ = fs::create_dir_all(&t_dir);
    let path = t_dir.join(format!("{}.json", tmpl.id));
    let json = serde_json::to_string_pretty(tmpl).map_err(|e| e.to_string())?;
    fs::write(&path, json).map_err(|e| e.to_string())
}

pub fn load_custom_templates(base_dir: &Path) -> Vec<CustomTemplate> {
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
                && let Ok(tmpl) = serde_json::from_str::<CustomTemplate>(&content)
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

pub fn save_settings_to_dir(base_dir: &Path, settings: &AppSettings) -> Result<(), String> {
    let _ = fs::create_dir_all(base_dir);
    let path = base_dir.join("settings.json");
    let json = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    atomic_write_file(&path, &json, None)
}

pub fn load_settings_from_dir(base_dir: &Path) -> AppSettings {
    let path = base_dir.join("settings.json");
    if let Ok(content) = fs::read_to_string(&path)
        && let Ok(settings) = serde_json::from_str::<AppSettings>(&content)
    {
        return settings;
    }
    AppSettings::default()
}

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ModsJsonEntry {
    #[serde(rename = "FileName")]
    pub file_name: String,

    #[serde(rename = "Title")]
    pub title: String,

    #[serde(rename = "Enabled")]
    pub enabled: bool,

    #[serde(rename = "LoadPriority")]
    pub load_priority: u32,
}

pub fn load_mods_json(mods_json_path: &Path) -> Result<Vec<ModsJsonEntry>, String> {
    if !mods_json_path.exists() {
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(mods_json_path)
        .map_err(|e| format!("Failed to read Mods.json: {}", e))?;

    let entries: Vec<ModsJsonEntry> = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse Mods.json: {}", e))?;

    Ok(entries)
}

pub fn save_mods_json(mods_json_path: &Path, entries: &[ModsJsonEntry]) -> Result<(), String> {
    if let Some(parent) = mods_json_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create parent directory: {}", e))?;
    }

    let json = serde_json::to_string_pretty(entries)
        .map_err(|e| format!("Failed to serialize Mods.json: {}", e))?;

    fs::write(mods_json_path, json)
        .map_err(|e| format!("Failed to write Mods.json: {}", e))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_load_nonexistent_mods_json() {
        let temp_dir = TempDir::new().unwrap();
        let mods_json_path = temp_dir.path().join("Mods.json");

        let result = load_mods_json(&mods_json_path).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_save_and_load_mods_json() {
        let temp_dir = TempDir::new().unwrap();
        let mods_json_path = temp_dir.path().join("Mods.json");

        let entries = vec![
            ModsJsonEntry {
                file_name: "mod1".to_string(),
                title: "Mod 1".to_string(),
                enabled: true,
                load_priority: 0,
            },
            ModsJsonEntry {
                file_name: "mod2".to_string(),
                title: "Mod 2".to_string(),
                enabled: false,
                load_priority: 1,
            },
        ];

        save_mods_json(&mods_json_path, &entries).unwrap();
        let loaded = load_mods_json(&mods_json_path).unwrap();

        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].file_name, "mod1");
        assert_eq!(loaded[1].file_name, "mod2");
    }

}

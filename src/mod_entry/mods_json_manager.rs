use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use crate::mod_entry::dfmod_parser::parse_dfmod;

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

pub fn generate_mods_json(
    enabled_mod_folders: &[(String, PathBuf)],
    existing_entries: &[ModsJsonEntry],
) -> Result<Vec<ModsJsonEntry>, String> {
    let mut new_entries = Vec::new();
    let mut priority = 0u32;

    for (_mod_name, mod_path) in enabled_mod_folders {
        let dfmod_infos = parse_dfmod(mod_path)?;

        for dfmod_info in dfmod_infos {
            let existing_entry = existing_entries
                .iter()
                .find(|e| e.file_name == dfmod_info.file_name);

            let entry = ModsJsonEntry {
                file_name: dfmod_info.file_name.clone(),
                title: existing_entry.map_or(dfmod_info.title.clone(), |e| e.title.clone()),
                enabled: existing_entry.map_or(true, |e| e.enabled),
                load_priority: existing_entry.map_or(priority, |e| e.load_priority),
            };

            new_entries.push(entry);
            priority += 1;
        }
    }

    new_entries.sort_by_key(|e| e.load_priority);

    for (i, entry) in new_entries.iter_mut().enumerate() {
        entry.load_priority = i as u32;
    }

    Ok(new_entries)
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

    #[test]
    fn test_generate_mods_json_empty() {
        let result = generate_mods_json(&[], &[]).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_generate_mods_json_preserves_existing_title() {
        use std::fs;

        let temp_dir = TempDir::new().unwrap();
        let mod_path = temp_dir.path().join("TestMod");
        fs::create_dir(&mod_path).unwrap();

        let mods_subfolder = mod_path.join("Mods");
        fs::create_dir(&mods_subfolder).unwrap();

        // Create a .dfmod file with lowercase filename
        fs::write(mods_subfolder.join("dream 90s - backgrounds.dfmod"), "binary data").unwrap();

        // Simulate existing Mods.json entry with all-caps title from DFU
        let existing_entries = vec![ModsJsonEntry {
            file_name: "dream 90s - backgrounds".to_string(),
            title: "DREAM 90s - BACKGROUNDS".to_string(), // From asset bundle
            enabled: false,
            load_priority: 5,
        }];

        let enabled_mods = vec![("TestMod".to_string(), mod_path.clone())];
        let result = generate_mods_json(&enabled_mods, &existing_entries).unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].file_name, "dream 90s - backgrounds");
        // Should preserve the original all-caps title from DFU, not generate title case
        assert_eq!(result[0].title, "DREAM 90s - BACKGROUNDS");
        // Should preserve enabled and priority
        assert_eq!(result[0].enabled, false);
        assert_eq!(result[0].load_priority, 0); // Gets renormalized to 0
    }
}

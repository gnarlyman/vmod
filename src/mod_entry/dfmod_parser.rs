use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Deserialize, Serialize, Debug)]
struct DfmodMetadata {
    #[serde(rename = "Title")]
    title: String,

    #[serde(rename = "ModVersion")]
    #[serde(default)]
    mod_version: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DfmodInfo {
    pub title: String,
    pub file_name: String,
    pub dfmod_path: PathBuf,
}

pub fn parse_dfmod(mod_folder: &Path) -> Result<Vec<DfmodInfo>, String> {
    let mods_subfolder = mod_folder.join("Mods");

    if !mods_subfolder.exists() {
        return Ok(Vec::new());
    }

    let mut results = Vec::new();

    for entry in fs::read_dir(&mods_subfolder)
        .map_err(|e| format!("Failed to read Mods folder: {}", e))?
    {
        let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
        let path = entry.path();

        if path.is_file() && path.extension().map_or(false, |ext| ext == "dfmod") {
            let content = fs::read_to_string(&path)
                .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;

            let metadata: DfmodMetadata = serde_json::from_str(&content)
                .map_err(|e| format!("Failed to parse JSON in {}: {}", path.display(), e))?;

            results.push(DfmodInfo {
                title: metadata.title,
                file_name: mod_folder
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Unknown")
                    .to_string(),
                dfmod_path: path,
            });
        }
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_parse_dfmod_no_mods_folder() {
        let temp_dir = TempDir::new().unwrap();
        let mod_path = temp_dir.path().join("test_mod");
        fs::create_dir(&mod_path).unwrap();

        let result = parse_dfmod(&mod_path).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_dfmod_valid() {
        let temp_dir = TempDir::new().unwrap();
        let mod_path = temp_dir.path().join("test_mod");
        fs::create_dir(&mod_path).unwrap();

        let mods_subfolder = mod_path.join("Mods");
        fs::create_dir(&mods_subfolder).unwrap();

        let dfmod_content = r#"{
            "Title": "Test Mod",
            "ModVersion": "1.0.0"
        }"#;

        fs::write(mods_subfolder.join("test.dfmod"), dfmod_content).unwrap();

        let result = parse_dfmod(&mod_path).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].title, "Test Mod");
        assert_eq!(result[0].file_name, "test_mod");
    }

    #[test]
    fn test_parse_dfmod_malformed() {
        let temp_dir = TempDir::new().unwrap();
        let mod_path = temp_dir.path().join("test_mod");
        fs::create_dir(&mod_path).unwrap();

        let mods_subfolder = mod_path.join("Mods");
        fs::create_dir(&mods_subfolder).unwrap();

        fs::write(mods_subfolder.join("test.dfmod"), "not json").unwrap();

        let result = parse_dfmod(&mod_path);
        assert!(result.is_err());
    }
}

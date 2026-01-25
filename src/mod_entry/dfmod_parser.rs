use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct DfmodInfo {
    pub title: String,
    pub file_name: String,
}

/// Capitalize first letter of each word in a string
/// E.g., "aquatic sprites" → "Aquatic Sprites"
/// E.g., "archaeologists" → "Archaeologists"
fn capitalize_words(s: &str) -> String {
    s.split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Scan mod folder for .dfmod files
/// Returns DfmodInfo for each .dfmod file found
///
/// FileName: The .dfmod filename without extension (e.g., "archaeologists")
/// Title: Capitalized version of FileName (e.g., "Archaeologists")
///
/// NOTE: .dfmod files are Unity asset bundles and cannot be parsed without Unity APIs.
/// We use the .dfmod filename itself to match Daggerfall Unity's behavior.
pub fn parse_dfmod(mod_folder: &Path) -> Result<Vec<DfmodInfo>, String> {
    let mods_subfolder = mod_folder.join("Mods");

    if !mods_subfolder.exists() {
        return Ok(Vec::new());
    }

    let mut results = Vec::new();

    // Enumerate all .dfmod files and create an entry for each
    for entry in fs::read_dir(&mods_subfolder)
        .map_err(|e| format!("Failed to read Mods folder: {}", e))?
    {
        let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
        let path = entry.path();

        if path.is_file() && path.extension().map_or(false, |ext| ext == "dfmod") {
            // Get the filename without extension
            let file_name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string();

            // Capitalize for title
            let title = capitalize_words(&file_name);

            results.push(DfmodInfo {
                file_name,
                title,
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
    fn test_capitalize_words() {
        assert_eq!(capitalize_words("archaeologists"), "Archaeologists");
        assert_eq!(capitalize_words("aquatic sprites"), "Aquatic Sprites");
        assert_eq!(capitalize_words("ambienttext"), "Ambienttext");
        assert_eq!(capitalize_words("my awesome mod"), "My Awesome Mod");
    }

    #[test]
    fn test_parse_dfmod_no_mods_folder() {
        let temp_dir = TempDir::new().unwrap();
        let mod_path = temp_dir.path().join("test_mod");
        fs::create_dir(&mod_path).unwrap();

        let result = parse_dfmod(&mod_path).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_dfmod_with_dfmod_file() {
        let temp_dir = TempDir::new().unwrap();
        let mod_path = temp_dir.path().join("TestMod-1.0");
        fs::create_dir(&mod_path).unwrap();

        let mods_subfolder = mod_path.join("Mods");
        fs::create_dir(&mods_subfolder).unwrap();

        // Create a .dfmod file (filename is what matters, not folder name)
        fs::write(mods_subfolder.join("my awesome mod.dfmod"), "binary data").unwrap();

        let result = parse_dfmod(&mod_path).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].file_name, "my awesome mod");
        assert_eq!(result[0].title, "My Awesome Mod");
    }

    #[test]
    fn test_parse_dfmod_multiple_files() {
        let temp_dir = TempDir::new().unwrap();
        let mod_path = temp_dir.path().join("test_mod");
        fs::create_dir(&mod_path).unwrap();

        let mods_subfolder = mod_path.join("Mods");
        fs::create_dir(&mods_subfolder).unwrap();

        // Create multiple .dfmod files
        fs::write(mods_subfolder.join("archaeologists.dfmod"), "binary").unwrap();
        fs::write(mods_subfolder.join("aquatic sprites.dfmod"), "binary").unwrap();

        let result = parse_dfmod(&mod_path).unwrap();
        assert_eq!(result.len(), 2);

        // Results should contain both files
        let file_names: Vec<_> = result.iter().map(|r| r.file_name.as_str()).collect();
        assert!(file_names.contains(&"archaeologists"));
        assert!(file_names.contains(&"aquatic sprites"));
    }

    #[test]
    fn test_parse_dfmod_no_dfmod_files() {
        let temp_dir = TempDir::new().unwrap();
        let mod_path = temp_dir.path().join("test_mod");
        fs::create_dir(&mod_path).unwrap();

        let mods_subfolder = mod_path.join("Mods");
        fs::create_dir(&mods_subfolder).unwrap();

        // Create non-.dfmod file
        fs::write(mods_subfolder.join("readme.txt"), "text").unwrap();

        let result = parse_dfmod(&mod_path).unwrap();
        assert!(result.is_empty());
    }
}

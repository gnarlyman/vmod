use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct DfmodInfo {
    pub title: String,
    pub file_name: String,
}

/// Extract a readable title from a mod folder name
/// E.g., "ArchaeologistsGuild-2.6.1-14-2-6-1-1712582888" → "Archaeologists Guild"
fn extract_title_from_folder_name(folder_name: &str) -> String {
    // Split on common delimiters and take the first part
    let base_name = folder_name
        .split('-')
        .next()
        .unwrap_or(folder_name);

    // Insert spaces before capital letters (camelCase to Title Case)
    let mut result = String::new();
    let mut chars = base_name.chars().peekable();

    while let Some(c) = chars.next() {
        if c.is_uppercase() && !result.is_empty() {
            // Only add space if the previous char wasn't a space
            if !result.ends_with(' ') {
                result.push(' ');
            }
        }
        result.push(c);
    }

    result.trim().to_string()
}

/// Scan mod folder for .dfmod files
/// Returns DfmodInfo with folder name as both FileName and a generated Title
///
/// NOTE: .dfmod files are Unity asset bundles and cannot be parsed without Unity APIs.
/// We use the folder name as a fallback for the title.
pub fn parse_dfmod(mod_folder: &Path) -> Result<Vec<DfmodInfo>, String> {
    let mods_subfolder = mod_folder.join("Mods");

    if !mods_subfolder.exists() {
        return Ok(Vec::new());
    }

    let mut results = Vec::new();
    let folder_name = mod_folder
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("Unknown")
        .to_string();

    // Check if there are any .dfmod files
    let mut has_dfmod = false;
    for entry in fs::read_dir(&mods_subfolder)
        .map_err(|e| format!("Failed to read Mods folder: {}", e))?
    {
        let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
        let path = entry.path();

        if path.is_file() && path.extension().map_or(false, |ext| ext == "dfmod") {
            has_dfmod = true;
            break;
        }
    }

    // If we found at least one .dfmod file, create an entry for this mod
    if has_dfmod {
        results.push(DfmodInfo {
            title: extract_title_from_folder_name(&folder_name),
            file_name: folder_name,
        });
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_extract_title_from_folder_name() {
        assert_eq!(extract_title_from_folder_name("ArchaeologistsGuild-2.6.1"), "Archaeologists Guild");
        assert_eq!(extract_title_from_folder_name("MyAwesomeMod-1.0"), "My Awesome Mod");
        assert_eq!(extract_title_from_folder_name("simple"), "simple");
        assert_eq!(extract_title_from_folder_name("Test-Mod-123"), "Test");
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

        // Create a dummy .dfmod file (doesn't need real content)
        fs::write(mods_subfolder.join("test.dfmod"), "binary data").unwrap();

        let result = parse_dfmod(&mod_path).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].title, "Test Mod");
        assert_eq!(result[0].file_name, "TestMod-1.0");
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

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct DfmodInfo {
    pub title: String,
    pub file_name: String,
    /// Asset paths contained in the dfmod bundle (for conflict detection)
    pub asset_paths: Vec<String>,
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

/// Extract asset paths from a dfmod file (Unity asset bundle)
/// Returns a list of asset paths contained in the bundle
pub fn extract_dfmod_assets(dfmod_path: &Path) -> Vec<String> {
    use io_unity::unityfs::UnityFS;

    let file = match File::open(dfmod_path) {
        Ok(f) => f,
        Err(_) => return Vec::new(),
    };

    let reader = BufReader::new(file);

    // Load the UnityFS bundle
    let fs = match UnityFS::read(Box::new(reader), None) {
        Ok(f) => f,
        Err(_) => return Vec::new(),
    };

    let mut all_paths = Vec::new();

    // Extract paths from each CAB file in the bundle
    for cab_path in fs.get_cab_path() {
        if let Ok(data) = fs.get_file_data_by_path(&cab_path) {
            let paths = extract_asset_paths_from_data(&data);
            all_paths.extend(paths);
        }
    }

    // Deduplicate while preserving order
    let mut seen = std::collections::HashSet::new();
    all_paths.retain(|p| seen.insert(p.clone()));

    all_paths
}

/// Extract asset paths from serialized file data
/// Asset paths are stored with a length prefix followed by the path string
fn extract_asset_paths_from_data(data: &[u8]) -> Vec<String> {
    let mut paths = Vec::new();

    let mut i = 0;
    while i < data.len().saturating_sub(4) {
        // Read potential string length (little-endian u32)
        let len = u32::from_le_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]]) as usize;

        // Valid string length range for asset paths
        if len >= 4 && len < 512 && i + 4 + len <= data.len() {
            let potential_str = &data[i + 4..i + 4 + len];

            // Check if it looks like a valid UTF-8 string with path-like content
            if let Ok(s) = std::str::from_utf8(potential_str) {
                if is_asset_path(s) {
                    paths.push(s.to_string());
                    i += 4 + len; // Skip past this string
                    continue;
                }
            }
        }
        i += 1;
    }

    paths
}

/// Check if a string looks like an asset path
fn is_asset_path(s: &str) -> bool {
    // Must be printable ASCII
    if !s.chars().all(|c| c.is_ascii() && !c.is_ascii_control()) {
        return false;
    }

    let lower = s.to_lowercase();

    // Common asset path patterns
    if s.contains("assets/") || s.contains("Assets/") {
        return true;
    }

    // File extensions commonly in Unity assets
    let extensions = [
        ".cs", ".dll", ".txt", ".json", ".xml", ".png", ".jpg", ".wav", ".mp3", ".ogg", ".prefab",
        ".asset", ".mat", ".shader",
    ];
    for ext in &extensions {
        if lower.ends_with(ext) {
            return true;
        }
    }

    // Daggerfall-specific patterns
    if lower.contains("daggerfall")
        || lower.contains("dfunity")
        || lower.contains("questpack")
        || lower.contains("dfmod")
    {
        return true;
    }

    false
}

/// Scan mod folder for .dfmod files
/// Returns DfmodInfo for each .dfmod file found
///
/// FileName: The .dfmod filename without extension (e.g., "archaeologists")
/// Title: Capitalized version of FileName (e.g., "Archaeologists")
/// AssetPaths: List of asset paths contained in the dfmod (for conflict detection)
pub fn parse_dfmod(mod_folder: &Path) -> Result<Vec<DfmodInfo>, String> {
    parse_dfmod_with_options(mod_folder, true)
}

/// Scan mod folder for .dfmod files with option to skip asset extraction
/// This is useful when you only need basic info and want faster performance
pub fn parse_dfmod_basic(mod_folder: &Path) -> Result<Vec<DfmodInfo>, String> {
    parse_dfmod_with_options(mod_folder, false)
}

fn parse_dfmod_with_options(
    mod_folder: &Path,
    extract_assets: bool,
) -> Result<Vec<DfmodInfo>, String> {
    use std::fs;

    let mods_subfolder = mod_folder.join("Mods");

    if !mods_subfolder.exists() {
        return Ok(Vec::new());
    }

    let mut results = Vec::new();

    // Enumerate all .dfmod files and create an entry for each
    for entry in
        fs::read_dir(&mods_subfolder).map_err(|e| format!("Failed to read Mods folder: {}", e))?
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

            // Extract asset paths for conflict detection (if requested)
            let asset_paths = if extract_assets {
                extract_dfmod_assets(&path)
            } else {
                Vec::new()
            };

            results.push(DfmodInfo {
                file_name,
                title,
                asset_paths,
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

        let result = parse_dfmod_basic(&mod_path).unwrap();
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

        let result = parse_dfmod_basic(&mod_path).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].file_name, "my awesome mod");
        assert_eq!(result[0].title, "My Awesome Mod");
        // asset_paths is empty when using parse_dfmod_basic
        assert!(result[0].asset_paths.is_empty());
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

        let result = parse_dfmod_basic(&mod_path).unwrap();
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

        let result = parse_dfmod_basic(&mod_path).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_is_asset_path() {
        // Should match asset paths
        assert!(is_asset_path("assets/game/mods/test.png"));
        assert!(is_asset_path("Assets/Scripts/MyMod.cs"));
        assert!(is_asset_path("test.dfmod.json"));
        assert!(is_asset_path("mymod.dll"));

        // Should not match
        assert!(!is_asset_path("just a random string"));
        assert!(!is_asset_path("12345"));
        assert!(!is_asset_path("")); // Empty string
    }
}

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use super::dfmod_parser::{extract_dfmod_assets_cached, DfmodCacheKey};

/// Summary of conflicts for a single mod
#[derive(Debug, Clone)]
pub struct ModConflictSummary {
    pub mod_path: PathBuf,
    pub mod_name: String,
    pub total_conflict_count: usize,
    pub conflicts: Vec<ConflictInfo>,
}

/// Progress updates during conflict scanning
#[derive(Debug, Clone)]
pub enum ConflictScanProgress {
    Started { total_mods: usize },
    Processing { mod_name: String, current: usize, total: usize },
    Completed { results: HashMap<PathBuf, ModConflictSummary> },
    Error { message: String },
}

/// Information about a conflict with another mod
#[derive(Debug, Clone)]
pub struct ConflictInfo {
    pub other_mod_name: String,
    pub other_mod_path: PathBuf,
    pub conflicting_files: Vec<String>, // Relative paths
}

/// Enumerate all files in a mod folder, returning a map of relative path -> absolute path
/// Scans standard DFU folders: Mods, Textures, Sound, Music, etc.
pub fn enumerate_mod_files(mod_path: &Path) -> HashMap<String, PathBuf> {
    let mut files = HashMap::new();

    if !mod_path.exists() || !mod_path.is_dir() {
        return files;
    }

    // Read all subfolders in the mod
    if let Ok(entries) = fs::read_dir(mod_path) {
        for entry in entries.flatten() {
            let entry_path = entry.path();
            if entry_path.is_dir() {
                let folder_name = entry_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("");

                // Recursively enumerate files in this subfolder
                enumerate_files_recursive(&entry_path, folder_name, &mut files);
            }
        }
    }

    files
}

/// Recursively enumerate files, building relative paths
fn enumerate_files_recursive(
    dir: &Path,
    base_prefix: &str,
    files: &mut HashMap<String, PathBuf>,
) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let entry_path = entry.path();
            let file_name = entry_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");

            let relative_path = if base_prefix.is_empty() {
                file_name.to_string()
            } else {
                format!("{}/{}", base_prefix, file_name)
            };

            if entry_path.is_dir() {
                enumerate_files_recursive(&entry_path, &relative_path, files);
            } else {
                files.insert(relative_path, entry_path);
            }
        }
    }
}

/// Normalize a dfmod asset path for conflict comparison
/// Unity asset paths like "assets/game/mods/mobs/foo.png" become "mobs/foo.png"
fn normalize_dfmod_asset_path(asset_path: &str) -> Option<String> {
    let path = asset_path.trim();

    // Skip non-content files (scripts, metadata, dfmod files themselves)
    let dominated_extensions = [".cs", ".dll", ".json", ".dfmod", ".txt", ".xml", ".meta"];
    let lower_path = path.to_lowercase();
    for ext in &dominated_extensions {
        if lower_path.ends_with(ext) {
            return None;
        }
    }

    // Common Unity prefixes to strip (case-insensitive)
    let prefixes_to_strip = [
        "assets/streamingassets/",
        "assets/game/mods/",
        "assets/mods/",
        "assets/",
    ];

    let lower = path.to_lowercase();

    for prefix in &prefixes_to_strip {
        if lower.starts_with(prefix) {
            let prefix_len = prefix.len();
            let stripped = &path[prefix_len..];
            if !stripped.is_empty() && stripped.contains('/') {
                // Has a subfolder - this is content
                return Some(stripped.to_string());
            } else if !stripped.is_empty() {
                // Just a filename at the root - still valid
                return Some(stripped.to_string());
            }
        }
    }

    // If path has content file extensions, keep it as-is
    let content_extensions = [".png", ".jpg", ".jpeg", ".webm", ".ogg", ".wav", ".mp3",
                              ".prefab", ".asset", ".mat", ".shader", ".img"];
    for ext in &content_extensions {
        if lower_path.ends_with(ext) {
            return Some(path.to_string());
        }
    }

    None
}

/// Enumerate all files in a mod folder including dfmod assets (with caching)
/// Returns a map of relative path -> source path (loose file path or dfmod path)
pub fn enumerate_mod_files_with_dfmod(
    mod_path: &Path,
    dfmod_cache: &mut HashMap<DfmodCacheKey, Vec<String>>,
) -> HashMap<String, PathBuf> {
    let mut files = enumerate_mod_files(mod_path); // Loose files

    // Add dfmod assets
    let mods_folder = mod_path.join("Mods");
    if mods_folder.exists() {
        if let Ok(entries) = fs::read_dir(&mods_folder) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map_or(false, |e| e == "dfmod") {
                    let assets = extract_dfmod_assets_cached(&path, dfmod_cache);
                    for asset in assets {
                        // Normalize the asset path to match loose file format
                        if let Some(normalized) = normalize_dfmod_asset_path(&asset) {
                            files.insert(normalized, path.clone());
                        }
                    }
                }
            }
        }
    }

    files
}

/// Detect conflicts for all enabled mods (batch operation)
/// Returns a map of mod path -> conflict summary
pub fn detect_all_conflicts<F>(
    enabled_mods: &[(String, PathBuf)],
    dfmod_cache: &mut HashMap<DfmodCacheKey, Vec<String>>,
    progress_callback: F,
) -> HashMap<PathBuf, ModConflictSummary>
where
    F: Fn(String, usize, usize),
{
    let total = enabled_mods.len();
    let mut results: HashMap<PathBuf, ModConflictSummary> = HashMap::new();

    // First pass: enumerate all files for all mods
    let mut mod_files: HashMap<PathBuf, HashMap<String, PathBuf>> = HashMap::new();
    for (idx, (mod_name, mod_path)) in enabled_mods.iter().enumerate() {
        progress_callback(mod_name.clone(), idx + 1, total);
        let files = enumerate_mod_files_with_dfmod(mod_path, dfmod_cache);
        mod_files.insert(mod_path.clone(), files);
    }

    // Second pass: find conflicts for each mod
    for (mod_name, mod_path) in enabled_mods.iter() {
        let mut conflicts: Vec<ConflictInfo> = Vec::new();
        let selected_files = match mod_files.get(mod_path) {
            Some(f) => f,
            None => continue,
        };

        // Compare against all other mods
        for (other_name, other_path) in enabled_mods.iter() {
            if other_path == mod_path {
                continue;
            }

            let other_files = match mod_files.get(other_path) {
                Some(f) => f,
                None => continue,
            };

            // Find intersection of relative paths
            let mut conflicting: Vec<String> = selected_files
                .keys()
                .filter(|key| other_files.contains_key(*key))
                .cloned()
                .collect();

            if !conflicting.is_empty() {
                conflicting.sort();
                conflicts.push(ConflictInfo {
                    other_mod_name: other_name.clone(),
                    other_mod_path: other_path.clone(),
                    conflicting_files: conflicting,
                });
            }
        }

        // Sort conflicts by mod name
        conflicts.sort_by(|a, b| a.other_mod_name.cmp(&b.other_mod_name));

        let total_conflict_count: usize = conflicts.iter().map(|c| c.conflicting_files.len()).sum();

        results.insert(
            mod_path.clone(),
            ModConflictSummary {
                mod_path: mod_path.clone(),
                mod_name: mod_name.clone(),
                total_conflict_count,
                conflicts,
            },
        );
    }

    results
}

/// Detect conflicts between the selected mod and other enabled mods
/// Returns a list of ConflictInfo, one per conflicting mod
pub fn detect_conflicts(
    selected_mod_path: &Path,
    _selected_mod_name: &str,
    all_enabled_mods: &[(String, PathBuf)], // (name, path) tuples
) -> Vec<ConflictInfo> {
    let mut conflicts = Vec::new();

    // Get files from the selected mod
    let selected_files = enumerate_mod_files(selected_mod_path);

    if selected_files.is_empty() {
        return conflicts;
    }

    // Check each other enabled mod for conflicts
    for (other_name, other_path) in all_enabled_mods {
        // Skip the selected mod itself
        if other_path == selected_mod_path {
            continue;
        }

        let other_files = enumerate_mod_files(other_path);

        // Find intersection of relative paths
        let mut conflicting: Vec<String> = selected_files
            .keys()
            .filter(|key| other_files.contains_key(*key))
            .cloned()
            .collect();

        if !conflicting.is_empty() {
            // Sort for consistent display
            conflicting.sort();

            conflicts.push(ConflictInfo {
                other_mod_name: other_name.clone(),
                other_mod_path: other_path.clone(),
                conflicting_files: conflicting,
            });
        }
    }

    // Sort conflicts by mod name
    conflicts.sort_by(|a, b| a.other_mod_name.cmp(&b.other_mod_name));

    conflicts
}

/// Build a tree structure of all files in a mod
/// Returns a list of (relative_path, is_directory) tuples, sorted for tree display
pub fn get_mod_file_tree(mod_path: &Path) -> Vec<(String, bool)> {
    let mut entries = Vec::new();

    if !mod_path.exists() || !mod_path.is_dir() {
        return entries;
    }

    // Read all subfolders in the mod
    if let Ok(dir_entries) = fs::read_dir(mod_path) {
        for entry in dir_entries.flatten() {
            let entry_path = entry.path();
            if entry_path.is_dir() {
                let folder_name = entry_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string();

                // Add the folder itself
                entries.push((folder_name.clone(), true));

                // Recursively add contents
                collect_tree_entries(&entry_path, &folder_name, &mut entries);
            }
        }
    }

    // Sort entries: folders first, then alphabetically within each level
    entries.sort_by(|a, b| {
        let a_depth = a.0.matches('/').count();
        let b_depth = b.0.matches('/').count();

        if a_depth != b_depth {
            return a_depth.cmp(&b_depth);
        }

        // At same depth, directories come before files
        match (a.1, b.1) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.0.cmp(&b.0),
        }
    });

    entries
}

/// Recursively collect tree entries
fn collect_tree_entries(dir: &Path, prefix: &str, entries: &mut Vec<(String, bool)>) {
    if let Ok(dir_entries) = fs::read_dir(dir) {
        for entry in dir_entries.flatten() {
            let entry_path = entry.path();
            let file_name = entry_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");

            let relative_path = format!("{}/{}", prefix, file_name);
            let is_dir = entry_path.is_dir();

            entries.push((relative_path.clone(), is_dir));

            if is_dir {
                collect_tree_entries(&entry_path, &relative_path, entries);
            }
        }
    }
}

/// Get immediate children of a path within a mod's file structure
/// Used for TreeListModel's child model creation
pub fn get_children_at_path(mod_path: &Path, relative_path: &str) -> Vec<(String, String, bool)> {
    let mut children = Vec::new();

    let target_path = if relative_path.is_empty() {
        mod_path.to_path_buf()
    } else {
        mod_path.join(relative_path)
    };

    if !target_path.exists() || !target_path.is_dir() {
        return children;
    }

    if let Ok(entries) = fs::read_dir(&target_path) {
        for entry in entries.flatten() {
            let entry_path = entry.path();
            let file_name = entry_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();

            let child_relative = if relative_path.is_empty() {
                file_name.clone()
            } else {
                format!("{}/{}", relative_path, file_name)
            };

            let is_dir = entry_path.is_dir();

            children.push((file_name, child_relative, is_dir));
        }
    }

    // Sort: directories first, then alphabetically
    children.sort_by(|a, b| {
        match (a.2, b.2) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.0.cmp(&b.0),
        }
    });

    children
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_enumerate_empty_folder() {
        let temp_dir = TempDir::new().unwrap();
        let files = enumerate_mod_files(temp_dir.path());
        assert!(files.is_empty());
    }

    #[test]
    fn test_enumerate_with_files() {
        let temp_dir = TempDir::new().unwrap();

        // Create structure: Mods/test.dfmod, Textures/tex.png
        let mods_folder = temp_dir.path().join("Mods");
        fs::create_dir(&mods_folder).unwrap();
        fs::write(mods_folder.join("test.dfmod"), "content").unwrap();

        let textures_folder = temp_dir.path().join("Textures");
        fs::create_dir(&textures_folder).unwrap();
        fs::write(textures_folder.join("tex.png"), "png").unwrap();

        let files = enumerate_mod_files(temp_dir.path());

        assert_eq!(files.len(), 2);
        assert!(files.contains_key("Mods/test.dfmod"));
        assert!(files.contains_key("Textures/tex.png"));
    }

    #[test]
    fn test_detect_conflicts() {
        let temp_dir = TempDir::new().unwrap();

        // Mod A
        let mod_a = temp_dir.path().join("mod_a");
        fs::create_dir_all(mod_a.join("Textures")).unwrap();
        fs::write(mod_a.join("Textures/shared.png"), "a").unwrap();
        fs::write(mod_a.join("Textures/unique_a.png"), "a").unwrap();

        // Mod B
        let mod_b = temp_dir.path().join("mod_b");
        fs::create_dir_all(mod_b.join("Textures")).unwrap();
        fs::write(mod_b.join("Textures/shared.png"), "b").unwrap();
        fs::write(mod_b.join("Textures/unique_b.png"), "b").unwrap();

        let enabled_mods = vec![
            ("Mod A".to_string(), mod_a.clone()),
            ("Mod B".to_string(), mod_b.clone()),
        ];

        let conflicts = detect_conflicts(&mod_a, "Mod A", &enabled_mods);

        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].other_mod_name, "Mod B");
        assert_eq!(conflicts[0].conflicting_files, vec!["Textures/shared.png"]);
    }

    #[test]
    fn test_get_children_at_path() {
        let temp_dir = TempDir::new().unwrap();

        let mods_folder = temp_dir.path().join("Mods");
        fs::create_dir(&mods_folder).unwrap();
        fs::write(mods_folder.join("test.dfmod"), "content").unwrap();

        let textures_folder = temp_dir.path().join("Textures");
        fs::create_dir(&textures_folder).unwrap();

        // Get root children
        let children = get_children_at_path(temp_dir.path(), "");
        assert_eq!(children.len(), 2);

        // Both should be directories
        assert!(children.iter().all(|(_, _, is_dir)| *is_dir));
    }

    #[test]
    fn test_normalize_dfmod_asset_path() {
        // Unity asset paths should be normalized
        assert_eq!(
            normalize_dfmod_asset_path("assets/game/mods/mobs/490_25-1.png"),
            Some("mobs/490_25-1.png".to_string())
        );
        assert_eq!(
            normalize_dfmod_asset_path("Assets/Game/Mods/backgrounds/foo.png"),
            Some("backgrounds/foo.png".to_string())
        );
        assert_eq!(
            normalize_dfmod_asset_path("assets/streamingassets/Textures/bar.png"),
            Some("Textures/bar.png".to_string())
        );

        // Content files without prefix should be kept
        assert_eq!(
            normalize_dfmod_asset_path("Textures/test.png"),
            Some("Textures/test.png".to_string())
        );

        // Scripts and metadata should be filtered out
        assert_eq!(normalize_dfmod_asset_path("MyMod.cs"), None);
        assert_eq!(normalize_dfmod_asset_path("assets/game/mods/mod.dfmod.json"), None);
        assert_eq!(normalize_dfmod_asset_path("test.dfmod"), None);

        // Video and audio content should be kept
        assert_eq!(
            normalize_dfmod_asset_path("Assets/Game/Mods/- CINEMATICS/ANIM0001.webm"),
            Some("- CINEMATICS/ANIM0001.webm".to_string())
        );
        assert_eq!(
            normalize_dfmod_asset_path("assets/game/mods/- music/song_02.ogg"),
            Some("- music/song_02.ogg".to_string())
        );
    }
}

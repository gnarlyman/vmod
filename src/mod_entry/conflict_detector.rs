use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

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
}

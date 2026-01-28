use std::fs;
use std::path::{Path, PathBuf};
use std::os::unix::fs as unix_fs;

/// Virtual File System manager using symlinks
/// Handles DFU standard archive structure and load order
#[derive(Clone, Debug)]
pub struct VirtualFileSystem {
    pub game_mods_folder: PathBuf,
}

impl VirtualFileSystem {
    pub fn new(game_mods_folder: PathBuf) -> Self {
        Self { game_mods_folder }
    }

    /// Get the StreamingAssets base directory from the Mods folder
    fn get_streaming_assets_folder(&self) -> PathBuf {
        // game_mods_folder is typically: .../StreamingAssets/Mods
        // We want: .../StreamingAssets
        self.game_mods_folder.parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| self.game_mods_folder.clone())
    }

    /// Enables a mod by creating symlinks from mod folder to StreamingAssets
    /// Supports standard DFU archive structure (Mods/, Docs/, Textures/, etc.)
    /// Files are symlinked, not folders, to allow proper overriding
    pub fn enable_mod(&self, mod_path: &Path) -> Result<(), String> {
        if !mod_path.exists() {
            return Err(format!("Mod path does not exist: {}", mod_path.display()));
        }

        let streaming_assets = self.get_streaming_assets_folder();

        // Read all subfolders in the mod (Mods/, Docs/, Textures/, etc.)
        let entries = fs::read_dir(mod_path)
            .map_err(|e| format!("Failed to read mod folder: {}", e))?;

        for entry in entries {
            let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
            let source_path = entry.path();

            // Skip non-directories and Docs folder (documentation doesn't go to game)
            if !source_path.is_dir() {
                continue;
            }

            let folder_name = source_path
                .file_name()
                .and_then(|n| n.to_str())
                .ok_or("Invalid folder name")?;

            // Target is the corresponding folder in StreamingAssets
            let target_base = streaming_assets.join(folder_name);

            // Create target folder if it doesn't exist
            if !target_base.exists() {
                fs::create_dir_all(&target_base)
                    .map_err(|e| format!("Failed to create target folder {}: {}", folder_name, e))?;
            }

            // Symlink all files from this subfolder
            self.symlink_files_recursive(&source_path, &target_base)?;
        }

        Ok(())
    }

    /// Recursively symlink files from source to target
    /// Overwrites existing symlinks (for load order)
    fn symlink_files_recursive(&self, source: &Path, target: &Path) -> Result<(), String> {
        let entries = fs::read_dir(source)
            .map_err(|e| format!("Failed to read directory {}: {}", source.display(), e))?;

        for entry in entries {
            let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
            let source_path = entry.path();
            let file_name = source_path
                .file_name()
                .ok_or("Invalid file name")?;
            let target_path = target.join(file_name);

            if source_path.is_dir() {
                // Create subdirectory if needed
                if !target_path.exists() {
                    fs::create_dir_all(&target_path)
                        .map_err(|e| format!("Failed to create subdirectory: {}", e))?;
                }
                // Recurse into subdirectory
                self.symlink_files_recursive(&source_path, &target_path)?;
            } else {
                // Remove existing symlink if present (for overriding)
                if target_path.symlink_metadata().is_ok() {
                    fs::remove_file(&target_path)
                        .map_err(|e| format!("Failed to remove existing symlink: {}", e))?;
                }

                // Create symlink
                unix_fs::symlink(&source_path, &target_path)
                    .map_err(|e| format!("Failed to create symlink for {}: {}", file_name.to_string_lossy(), e))?;
            }
        }

        Ok(())
    }

    /// Disables a mod by removing all its symlinks
    /// This requires removing ALL symlinks since we can't track which mod owns which
    pub fn disable_mod(&self, _mod_path: &Path) -> Result<(), String> {
        // Since we can't track individual mod ownership of symlinks,
        // we need to rebuild all symlinks when any mod is disabled
        // This will be handled by rebuild_all_symlinks() called from the window
        Ok(())
    }

    /// Removes all symlinks from StreamingAssets
    pub fn clear_all_symlinks(&self) -> Result<(), String> {
        let streaming_assets = self.get_streaming_assets_folder();

        // Read all entries in StreamingAssets and clear symlinks from each
        let entries = match fs::read_dir(&streaming_assets) {
            Ok(entries) => entries,
            Err(e) => {
                // If StreamingAssets doesn't exist yet, nothing to clear
                log::debug!("StreamingAssets not found ({}), nothing to clear", e);
                return Ok(());
            }
        };

        for entry in entries {
            let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
            let entry_path = entry.path();

            // Only process directories
            if entry_path.is_dir() {
                self.remove_symlinks_recursive(&entry_path)?;
            }
        }

        Ok(())
    }

    /// Recursively remove symlinks from a directory
    fn remove_symlinks_recursive(&self, path: &Path) -> Result<(), String> {
        let entries = fs::read_dir(path)
            .map_err(|e| format!("Failed to read directory {}: {}", path.display(), e))?;

        for entry in entries {
            let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
            let entry_path = entry.path();

            if entry_path.is_dir() {
                // Recurse into subdirectories
                self.remove_symlinks_recursive(&entry_path)?;
            } else {
                // Check if it's a symlink and remove it
                if let Ok(metadata) = entry_path.symlink_metadata() {
                    if metadata.file_type().is_symlink() {
                        fs::remove_file(&entry_path)
                            .map_err(|e| format!("Failed to remove symlink: {}", e))?;
                    }
                }
            }
        }

        Ok(())
    }

    /// Checks if a mod is currently enabled (placeholder - actual state tracked by ModEntry)
    pub fn is_mod_enabled(&self, _mod_name: &str) -> bool {
        // This is now tracked by ModEntry.enabled property
        // We keep this for compatibility but it's not used
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_vfs_creation() {
        let vfs = VirtualFileSystem::new(PathBuf::from("/test/path"));
        assert_eq!(vfs.game_mods_folder, PathBuf::from("/test/path"));
    }

    #[test]
    fn test_get_streaming_assets_folder() {
        let game_mods = PathBuf::from("/game/StreamingAssets/Mods");
        let vfs = VirtualFileSystem::new(game_mods);

        let streaming_assets = vfs.get_streaming_assets_folder();
        assert_eq!(streaming_assets, PathBuf::from("/game/StreamingAssets"));
    }

    #[test]
    fn test_enable_mod_with_archive_structure() {
        let temp_dir = TempDir::new().unwrap();
        let game_streaming_assets = temp_dir.path().join("game").join("StreamingAssets");
        let game_mods = game_streaming_assets.join("Mods");
        fs::create_dir_all(&game_mods).unwrap();

        // Create mod with archive structure
        let mod_dir = temp_dir.path().join("mods").join("test_mod");
        fs::create_dir_all(&mod_dir).unwrap();

        let mods_folder = mod_dir.join("Mods");
        fs::create_dir(&mods_folder).unwrap();
        fs::write(mods_folder.join("test.dfmod"), "content").unwrap();

        let textures_folder = mod_dir.join("Textures");
        fs::create_dir(&textures_folder).unwrap();
        fs::write(textures_folder.join("texture.png"), "png").unwrap();

        let vfs = VirtualFileSystem::new(game_mods);
        let result = vfs.enable_mod(&mod_dir);

        assert!(result.is_ok());
        assert!(game_streaming_assets.join("Mods").join("test.dfmod").exists());
        assert!(game_streaming_assets.join("Textures").join("texture.png").exists());
    }

    #[test]
    fn test_enable_mod_includes_docs() {
        let temp_dir = TempDir::new().unwrap();
        let game_streaming_assets = temp_dir.path().join("game").join("StreamingAssets");
        let game_mods = game_streaming_assets.join("Mods");
        fs::create_dir_all(&game_mods).unwrap();

        let mod_dir = temp_dir.path().join("mods").join("test_mod");
        fs::create_dir_all(&mod_dir).unwrap();

        let docs_folder = mod_dir.join("Docs");
        fs::create_dir(&docs_folder).unwrap();
        fs::write(docs_folder.join("readme.txt"), "docs").unwrap();

        let vfs = VirtualFileSystem::new(game_mods);
        vfs.enable_mod(&mod_dir).unwrap();

        // Docs should be symlinked
        assert!(game_streaming_assets.join("Docs").join("readme.txt").exists());
    }

    #[test]
    fn test_clear_all_symlinks() {
        let temp_dir = TempDir::new().unwrap();
        let game_streaming_assets = temp_dir.path().join("game").join("StreamingAssets");
        let game_mods = game_streaming_assets.join("Mods");
        fs::create_dir_all(&game_mods).unwrap();

        // Create some symlinks
        let source_file = temp_dir.path().join("source.txt");
        fs::write(&source_file, "test").unwrap();

        let symlink_path = game_mods.join("link.txt");
        unix_fs::symlink(&source_file, &symlink_path).unwrap();

        let vfs = VirtualFileSystem::new(game_mods.clone());
        let result = vfs.clear_all_symlinks();

        assert!(result.is_ok());
        assert!(!symlink_path.exists());
    }
}

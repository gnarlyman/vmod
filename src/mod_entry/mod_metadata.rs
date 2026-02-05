//! Per-mod metadata files (`vmod_meta.json`) stored inside each mod folder.
//!
//! These files persist Nexus Mods metadata (real mod name, version, IDs)
//! so that display names survive across sessions without re-querying the API.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// The metadata file name stored inside each mod folder
const METADATA_FILE_NAME: &str = "vmod_meta.json";

/// Metadata for a single mod, stored in `vmod_meta.json` inside the mod folder.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModMetadata {
    /// Human-readable name from Nexus Mods
    pub mod_name: String,
    /// Nexus mod ID
    pub nexus_id: String,
    /// Installed file version (from Nexus file info)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Nexus file ID
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_id: Option<u64>,
    /// Game domain (e.g. "daggerfallunity")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub game_domain: Option<String>,
    /// Unix timestamp when this metadata was fetched
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fetched_at: Option<i64>,
    /// Version status: 0=unknown, 1=up-to-date, 2=outdated
    #[serde(default)]
    pub version_status: u8,
    /// Latest version available on Nexus (if checked)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_version: Option<String>,
    /// Unix timestamp when version was last checked
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version_checked_at: Option<u64>,
}

/// Load metadata from a mod folder's `vmod_meta.json`.
/// Returns `None` if the file doesn't exist or can't be parsed.
pub fn load_metadata(mod_folder: &Path) -> Option<ModMetadata> {
    let meta_path = mod_folder.join(METADATA_FILE_NAME);
    if !meta_path.exists() {
        return None;
    }

    match std::fs::read_to_string(&meta_path) {
        Ok(contents) => {
            match serde_json::from_str(&contents) {
                Ok(metadata) => Some(metadata),
                Err(e) => {
                    log::warn!("Failed to parse {}: {}", meta_path.display(), e);
                    None
                }
            }
        }
        Err(e) => {
            log::warn!("Failed to read {}: {}", meta_path.display(), e);
            None
        }
    }
}

/// Save metadata to a mod folder's `vmod_meta.json`.
pub fn save_metadata(mod_folder: &Path, metadata: &ModMetadata) -> Result<(), String> {
    let meta_path = mod_folder.join(METADATA_FILE_NAME);

    let contents = serde_json::to_string_pretty(metadata)
        .map_err(|e| format!("Failed to serialize metadata: {}", e))?;

    std::fs::write(&meta_path, contents)
        .map_err(|e| format!("Failed to write {}: {}", meta_path.display(), e))?;

    log::debug!("Saved metadata for '{}' to {:?}", metadata.mod_name, meta_path);
    Ok(())
}

/// Check if a mod folder has a `vmod_meta.json` file.
#[cfg(test)]
fn has_metadata(mod_folder: &Path) -> bool {
    mod_folder.join(METADATA_FILE_NAME).exists()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_save_and_load_metadata() {
        let temp_dir = TempDir::new().unwrap();
        let mod_folder = temp_dir.path();

        let metadata = ModMetadata {
            mod_name: "Test Mod".to_string(),
            nexus_id: "123".to_string(),
            version: Some("1.5".to_string()),
            file_id: Some(456),
            game_domain: Some("daggerfallunity".to_string()),
            fetched_at: Some(1700000000),
            version_status: 1,
            latest_version: Some("1.5".to_string()),
            version_checked_at: Some(1700000000),
        };

        // Save
        save_metadata(mod_folder, &metadata).unwrap();

        // Verify file exists
        assert!(has_metadata(mod_folder));

        // Load and verify
        let loaded = load_metadata(mod_folder).unwrap();
        assert_eq!(loaded.mod_name, "Test Mod");
        assert_eq!(loaded.nexus_id, "123");
        assert_eq!(loaded.version, Some("1.5".to_string()));
        assert_eq!(loaded.file_id, Some(456));
        assert_eq!(loaded.game_domain, Some("daggerfallunity".to_string()));
        assert_eq!(loaded.fetched_at, Some(1700000000));
    }

    #[test]
    fn test_load_nonexistent() {
        let temp_dir = TempDir::new().unwrap();
        assert!(!has_metadata(temp_dir.path()));
        assert!(load_metadata(temp_dir.path()).is_none());
    }

    #[test]
    fn test_optional_fields() {
        let temp_dir = TempDir::new().unwrap();

        let metadata = ModMetadata {
            mod_name: "Minimal Mod".to_string(),
            nexus_id: "42".to_string(),
            version: None,
            file_id: None,
            game_domain: None,
            fetched_at: None,
            version_status: 0,
            latest_version: None,
            version_checked_at: None,
        };

        save_metadata(temp_dir.path(), &metadata).unwrap();
        let loaded = load_metadata(temp_dir.path()).unwrap();
        assert_eq!(loaded.mod_name, "Minimal Mod");
        assert_eq!(loaded.nexus_id, "42");
        assert!(loaded.version.is_none());
        assert!(loaded.file_id.is_none());
    }

    #[test]
    fn test_serialization_roundtrip() {
        let metadata = ModMetadata {
            mod_name: "An Adventurer's Guide".to_string(),
            nexus_id: "697".to_string(),
            version: Some("1.3".to_string()),
            file_id: Some(789),
            game_domain: Some("daggerfallunity".to_string()),
            fetched_at: Some(1747406905),
            version_status: 0,
            latest_version: None,
            version_checked_at: None,
        };

        let json = serde_json::to_string(&metadata).unwrap();
        let restored: ModMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.mod_name, metadata.mod_name);
        assert_eq!(restored.nexus_id, metadata.nexus_id);
    }
}

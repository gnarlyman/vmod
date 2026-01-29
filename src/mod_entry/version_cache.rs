//! Version cache for storing Nexus Mods version check results.
//!
//! Persists version status to disk so that version colors are restored
//! on app restart without needing to re-check the API.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Cache entry for a single mod folder
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionCacheEntry {
    /// Version status: 0=unknown, 1=up-to-date, 2=outdated
    pub status: u8,
    /// Latest version from Nexus (if known)
    pub latest_version: Option<String>,
    /// Unix timestamp when this was last checked
    pub checked_at: u64,
    /// Nexus mod ID (for reference)
    pub nexus_id: Option<String>,
}

/// Version cache storing check results keyed by folder name
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VersionCache {
    /// Map of folder name -> cache entry
    pub entries: HashMap<String, VersionCacheEntry>,
}

impl VersionCache {
    /// Get the cache file path
    fn cache_path() -> Option<PathBuf> {
        dirs::cache_dir().map(|d| d.join("vmod").join("version_cache.json"))
    }

    /// Load cache from disk, or return empty cache if not found
    pub fn load() -> Self {
        let Some(path) = Self::cache_path() else {
            return Self::default();
        };

        if !path.exists() {
            return Self::default();
        }

        match std::fs::read_to_string(&path) {
            Ok(contents) => {
                match serde_json::from_str(&contents) {
                    Ok(cache) => {
                        log::debug!("Loaded version cache with {} entries",
                            Self::entries_count(&cache));
                        cache
                    }
                    Err(e) => {
                        log::warn!("Failed to parse version cache: {}", e);
                        Self::default()
                    }
                }
            }
            Err(e) => {
                log::warn!("Failed to read version cache: {}", e);
                Self::default()
            }
        }
    }

    fn entries_count(cache: &Self) -> usize {
        cache.entries.len()
    }

    /// Save cache to disk
    pub fn save(&self) -> Result<(), String> {
        let Some(path) = Self::cache_path() else {
            return Err("Could not determine cache directory".to_string());
        };

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create cache directory: {}", e))?;
        }

        let contents = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize cache: {}", e))?;

        std::fs::write(&path, contents)
            .map_err(|e| format!("Failed to write cache: {}", e))?;

        log::debug!("Saved version cache with {} entries to {:?}",
            self.entries.len(), path);
        Ok(())
    }

    /// Get cache entry for a folder
    pub fn get(&self, folder_name: &str) -> Option<&VersionCacheEntry> {
        self.entries.get(folder_name)
    }

    /// Set cache entry for a folder
    pub fn set(&mut self, folder_name: String, status: u8, latest_version: Option<String>, nexus_id: Option<String>) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        self.entries.insert(folder_name, VersionCacheEntry {
            status,
            latest_version,
            checked_at: now,
            nexus_id,
        });
    }

    /// Check if a folder needs a version check
    /// Returns true if not in cache or status is unknown (0)
    pub fn needs_check(&self, folder_name: &str) -> bool {
        match self.entries.get(folder_name) {
            None => true,
            Some(entry) => entry.status == 0,
        }
    }

    /// Remove an entry from the cache (e.g., when folder is renamed/deleted)
    pub fn remove(&mut self, folder_name: &str) {
        self.entries.remove(folder_name);
    }

    /// Clear all entries
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_cache_basic() {
        let mut cache = VersionCache::default();

        // Initially empty
        assert!(cache.get("test-mod").is_none());
        assert!(cache.needs_check("test-mod"));

        // Set an entry
        cache.set(
            "test-mod".to_string(),
            1, // up-to-date
            Some("1.5".to_string()),
            Some("123".to_string()),
        );

        // Now exists and doesn't need check
        assert!(cache.get("test-mod").is_some());
        assert!(!cache.needs_check("test-mod"));

        let entry = cache.get("test-mod").unwrap();
        assert_eq!(entry.status, 1);
        assert_eq!(entry.latest_version, Some("1.5".to_string()));
        assert_eq!(entry.nexus_id, Some("123".to_string()));
    }

    #[test]
    fn test_unknown_status_needs_check() {
        let mut cache = VersionCache::default();

        // Set with unknown status
        cache.set(
            "test-mod".to_string(),
            0, // unknown
            None,
            Some("123".to_string()),
        );

        // Status 0 still needs check
        assert!(cache.needs_check("test-mod"));
    }

    #[test]
    fn test_serialization() {
        let mut cache = VersionCache::default();
        cache.set(
            "my-mod-123-1-0-1234567890".to_string(),
            2, // outdated
            Some("2.0".to_string()),
            Some("123".to_string()),
        );

        let json = serde_json::to_string(&cache).unwrap();
        let restored: VersionCache = serde_json::from_str(&json).unwrap();

        assert!(restored.get("my-mod-123-1-0-1234567890").is_some());
        let entry = restored.get("my-mod-123-1-0-1234567890").unwrap();
        assert_eq!(entry.status, 2);
    }
}

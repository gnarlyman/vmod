//! Nexus Mods API configuration storage.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

const CONFIG_FILENAME: &str = "nexus_api.json";

/// Nexus API configuration stored on disk
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NexusConfig {
    /// API key for authentication
    #[serde(default)]
    pub api_key: Option<String>,
    /// Cached user name (for display, not auth)
    #[serde(default)]
    pub user_name: Option<String>,
    /// Cached user ID
    #[serde(default)]
    pub user_id: Option<u64>,
    /// Whether user has premium membership
    #[serde(default)]
    pub is_premium: bool,
    /// Default game domain
    #[serde(default = "default_game_domain")]
    pub game_domain: String,
}

fn default_game_domain() -> String {
    "daggerfallunity".to_string()
}

impl Default for NexusConfig {
    fn default() -> Self {
        Self {
            api_key: None,
            user_name: None,
            user_id: None,
            is_premium: false,
            game_domain: default_game_domain(),
        }
    }
}

impl NexusConfig {
    /// Get the config file path
    fn config_path() -> Option<PathBuf> {
        dirs::config_dir().map(|p| p.join("vmod").join(CONFIG_FILENAME))
    }

    /// Load configuration from disk, or return defaults if not found
    pub fn load() -> Self {
        let Some(path) = Self::config_path() else {
            log::warn!("Could not determine config directory");
            return Self::default();
        };

        if !path.exists() {
            log::debug!("Nexus config file not found at {:?}, using defaults", path);
            return Self::default();
        }

        match fs::read_to_string(&path) {
            Ok(contents) => match serde_json::from_str(&contents) {
                Ok(config) => {
                    log::debug!("Loaded Nexus config from {:?}", path);
                    config
                }
                Err(e) => {
                    log::error!("Failed to parse Nexus config: {}", e);
                    Self::default()
                }
            },
            Err(e) => {
                log::error!("Failed to read Nexus config: {}", e);
                Self::default()
            }
        }
    }

    /// Save configuration to disk
    pub fn save(&self) -> Result<(), std::io::Error> {
        let Some(path) = Self::config_path() else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Could not determine config directory",
            ));
        };

        // Ensure directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let contents = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        fs::write(&path, contents)?;
        log::debug!("Saved Nexus config to {:?}", path);
        Ok(())
    }

    /// Check if an API key is configured
    pub fn has_api_key(&self) -> bool {
        self.api_key.as_ref().map(|k| !k.is_empty()).unwrap_or(false)
    }

    /// Set the API key and optionally user info
    pub fn set_credentials(&mut self, api_key: String, user_name: Option<String>, user_id: Option<u64>, is_premium: bool) {
        self.api_key = Some(api_key);
        self.user_name = user_name;
        self.user_id = user_id;
        self.is_premium = is_premium;
    }

    /// Clear stored credentials
    pub fn clear_credentials(&mut self) {
        self.api_key = None;
        self.user_name = None;
        self.user_id = None;
        self.is_premium = false;
    }
}

/// Get the downloads staging directory
pub fn downloads_dir() -> Option<PathBuf> {
    dirs::cache_dir().map(|p| p.join("vmod").join("downloads"))
}

/// Ensure the downloads directory exists
pub fn ensure_downloads_dir() -> Result<PathBuf, std::io::Error> {
    let Some(path) = downloads_dir() else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Could not determine cache directory",
        ));
    };

    fs::create_dir_all(&path)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = NexusConfig::default();
        assert!(!config.has_api_key());
        assert_eq!(config.game_domain, "daggerfallunity");
    }

    #[test]
    fn test_set_credentials() {
        let mut config = NexusConfig::default();
        config.set_credentials(
            "test_key".to_string(),
            Some("TestUser".to_string()),
            Some(12345),
            true,
        );
        assert!(config.has_api_key());
        assert_eq!(config.api_key, Some("test_key".to_string()));
        assert!(config.is_premium);
    }

    #[test]
    fn test_clear_credentials() {
        let mut config = NexusConfig::default();
        config.set_credentials("test_key".to_string(), Some("User".to_string()), Some(1), false);
        config.clear_credentials();
        assert!(!config.has_api_key());
        assert!(config.user_name.is_none());
    }
}

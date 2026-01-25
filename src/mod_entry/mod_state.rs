use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Stores the enabled state of mods for a profile
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct ModState {
    /// Map of mod folder name -> enabled state
    pub enabled_mods: HashMap<String, bool>,
}

impl ModState {
    pub fn new() -> Self {
        Self {
            enabled_mods: HashMap::new(),
        }
    }

    /// Load mod state for a profile
    pub fn load(profile_name: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let config_dir = dirs::config_dir()
            .ok_or("Could not find config directory")?
            .join("vmod")
            .join("profiles")
            .join(profile_name);

        let state_file = config_dir.join("mod_state.json");

        if !state_file.exists() {
            return Ok(Self::new());
        }

        let data = std::fs::read_to_string(state_file)?;
        let state: ModState = serde_json::from_str(&data)?;
        Ok(state)
    }

    /// Save mod state for a profile
    pub fn save(&self, profile_name: &str) -> Result<(), Box<dyn std::error::Error>> {
        let config_dir = dirs::config_dir()
            .ok_or("Could not find config directory")?
            .join("vmod")
            .join("profiles")
            .join(profile_name);

        std::fs::create_dir_all(&config_dir)?;

        let state_file = config_dir.join("mod_state.json");
        let data = serde_json::to_string_pretty(self)?;
        std::fs::write(state_file, data)?;
        Ok(())
    }

    /// Check if a mod is enabled
    pub fn is_enabled(&self, mod_name: &str) -> bool {
        self.enabled_mods.get(mod_name).copied().unwrap_or(false)
    }

    /// Set enabled state for a mod
    pub fn set_enabled(&mut self, mod_name: String, enabled: bool) {
        self.enabled_mods.insert(mod_name, enabled);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mod_state_new() {
        let state = ModState::new();
        assert!(state.enabled_mods.is_empty());
    }

    #[test]
    fn test_mod_state_set_enabled() {
        let mut state = ModState::new();
        state.set_enabled("test_mod".to_string(), true);
        assert!(state.is_enabled("test_mod"));
    }

    #[test]
    fn test_mod_state_is_enabled_default() {
        let state = ModState::new();
        assert!(!state.is_enabled("nonexistent"));
    }

    #[test]
    fn test_mod_state_serialization() {
        let mut state = ModState::new();
        state.set_enabled("mod1".to_string(), true);
        state.set_enabled("mod2".to_string(), false);

        let json = serde_json::to_string(&state).unwrap();
        let deserialized: ModState = serde_json::from_str(&json).unwrap();

        assert!(deserialized.is_enabled("mod1"));
        assert!(!deserialized.is_enabled("mod2"));
    }
}

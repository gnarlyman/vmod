use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    pub game_path: PathBuf,
    pub launcher_path: Option<PathBuf>,
    pub mods_json_path: Option<PathBuf>,
}

impl Profile {
    pub fn new(name: String, game_path: PathBuf) -> Self {
        Self {
            name,
            game_path,
            launcher_path: None,
            mods_json_path: None,
        }
    }

    /// Validates that the game executable exists in the game path
    pub fn validate_game_installation(&self) -> Result<(), String> {
        // Look for DaggerfallUnity executable
        let exe_path = self.game_path.join("DaggerfallUnity");
        let exe_path_with_ext = self.game_path.join("DaggerfallUnity.x86_64");

        if exe_path.exists() || exe_path_with_ext.exists() {
            Ok(())
        } else {
            Err(format!(
                "DaggerfallUnity executable not found in {}",
                self.game_path.display()
            ))
        }
    }

    /// Gets the default mods folder path for this profile
    pub fn get_mods_folder(&self) -> PathBuf {
        self.game_path
            .join("DaggerfallUnity_Data")
            .join("StreamingAssets")
            .join("Mods")
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProfileList {
    pub profiles: Vec<Profile>,
    pub active_profile: Option<usize>,
}

impl ProfileList {
    pub fn new() -> Self {
        Self {
            profiles: Vec::new(),
            active_profile: None,
        }
    }

    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let config_dir = dirs::config_dir()
            .ok_or("Could not find config directory")?
            .join("vmod");

        let profiles_file = config_dir.join("profiles.json");

        if !profiles_file.exists() {
            return Ok(Self::new());
        }

        let data = std::fs::read_to_string(profiles_file)?;
        let profiles: ProfileList = serde_json::from_str(&data)?;
        Ok(profiles)
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let config_dir = dirs::config_dir()
            .ok_or("Could not find config directory")?
            .join("vmod");

        std::fs::create_dir_all(&config_dir)?;

        let profiles_file = config_dir.join("profiles.json");
        let data = serde_json::to_string_pretty(self)?;
        std::fs::write(profiles_file, data)?;
        Ok(())
    }

    pub fn add_profile(&mut self, profile: Profile) {
        self.profiles.push(profile);
        if self.active_profile.is_none() {
            self.active_profile = Some(0);
        }
    }

    pub fn get_active_profile(&self) -> Option<&Profile> {
        self.active_profile
            .and_then(|idx| self.profiles.get(idx))
    }

    pub fn set_active_profile(&mut self, index: usize) {
        if index < self.profiles.len() {
            self.active_profile = Some(index);
        }
    }
}

impl Default for ProfileList {
    fn default() -> Self {
        Self::new()
    }
}

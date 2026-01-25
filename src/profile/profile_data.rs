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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profile_new() {
        let profile = Profile::new(
            "Test Profile".to_string(),
            PathBuf::from("/test/path"),
        );

        assert_eq!(profile.name, "Test Profile");
        assert_eq!(profile.game_path, PathBuf::from("/test/path"));
        assert!(profile.launcher_path.is_none());
        assert!(profile.mods_json_path.is_none());
    }

    #[test]
    fn test_profile_get_mods_folder() {
        let profile = Profile::new(
            "Test".to_string(),
            PathBuf::from("/game"),
        );

        let mods_folder = profile.get_mods_folder();
        assert_eq!(
            mods_folder,
            PathBuf::from("/game/DaggerfallUnity_Data/StreamingAssets/Mods")
        );
    }

    #[test]
    fn test_profile_validation_fails_for_missing_executable() {
        let profile = Profile::new(
            "Test".to_string(),
            PathBuf::from("/nonexistent/path"),
        );

        let result = profile.validate_game_installation();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("DaggerfallUnity executable not found"));
    }

    #[test]
    fn test_profile_serialization() -> Result<(), Box<dyn std::error::Error>> {
        let profile = Profile::new(
            "Test Profile".to_string(),
            PathBuf::from("/test/path"),
        );

        let json = serde_json::to_string(&profile)?;
        let deserialized: Profile = serde_json::from_str(&json)?;

        assert_eq!(profile.name, deserialized.name);
        assert_eq!(profile.game_path, deserialized.game_path);
        Ok(())
    }

    #[test]
    fn test_profile_list_new() {
        let list = ProfileList::new();
        assert!(list.profiles.is_empty());
        assert!(list.active_profile.is_none());
    }

    #[test]
    fn test_profile_list_add_profile() {
        let mut list = ProfileList::new();
        let profile = Profile::new("Test".to_string(), PathBuf::from("/test"));

        list.add_profile(profile);

        assert_eq!(list.profiles.len(), 1);
        assert_eq!(list.active_profile, Some(0));
        assert_eq!(list.profiles[0].name, "Test");
    }

    #[test]
    fn test_profile_list_add_multiple_profiles() {
        let mut list = ProfileList::new();

        list.add_profile(Profile::new("Profile 1".to_string(), PathBuf::from("/path1")));
        list.add_profile(Profile::new("Profile 2".to_string(), PathBuf::from("/path2")));
        list.add_profile(Profile::new("Profile 3".to_string(), PathBuf::from("/path3")));

        assert_eq!(list.profiles.len(), 3);
        assert_eq!(list.active_profile, Some(0)); // Should remain at first profile
        assert_eq!(list.profiles[0].name, "Profile 1");
        assert_eq!(list.profiles[1].name, "Profile 2");
        assert_eq!(list.profiles[2].name, "Profile 3");
    }

    #[test]
    fn test_profile_list_get_active_profile() {
        let mut list = ProfileList::new();
        list.add_profile(Profile::new("Test".to_string(), PathBuf::from("/test")));

        let active = list.get_active_profile();
        assert!(active.is_some());
        assert_eq!(active.unwrap().name, "Test");
    }

    #[test]
    fn test_profile_list_get_active_profile_empty() {
        let list = ProfileList::new();
        assert!(list.get_active_profile().is_none());
    }

    #[test]
    fn test_profile_list_set_active_profile() {
        let mut list = ProfileList::new();
        list.add_profile(Profile::new("Profile 1".to_string(), PathBuf::from("/path1")));
        list.add_profile(Profile::new("Profile 2".to_string(), PathBuf::from("/path2")));

        list.set_active_profile(1);
        assert_eq!(list.active_profile, Some(1));
        assert_eq!(list.get_active_profile().unwrap().name, "Profile 2");
    }

    #[test]
    fn test_profile_list_set_active_profile_invalid_index() {
        let mut list = ProfileList::new();
        list.add_profile(Profile::new("Test".to_string(), PathBuf::from("/test")));

        list.set_active_profile(10); // Out of bounds
        assert_eq!(list.active_profile, Some(0)); // Should remain unchanged
    }

    #[test]
    fn test_profile_list_serialization() -> Result<(), Box<dyn std::error::Error>> {
        let mut list = ProfileList::new();
        list.add_profile(Profile::new("Profile 1".to_string(), PathBuf::from("/path1")));
        list.add_profile(Profile::new("Profile 2".to_string(), PathBuf::from("/path2")));

        let json = serde_json::to_string(&list)?;
        let deserialized: ProfileList = serde_json::from_str(&json)?;

        assert_eq!(list.profiles.len(), deserialized.profiles.len());
        assert_eq!(list.active_profile, deserialized.active_profile);
        assert_eq!(list.profiles[0].name, deserialized.profiles[0].name);
        Ok(())
    }

    #[test]
    fn test_profile_list_default() {
        let list = ProfileList::default();
        assert!(list.profiles.is_empty());
        assert!(list.active_profile.is_none());
    }

    #[test]
    fn test_profile_active_changes_persist() {
        // Create a list with multiple profiles
        let mut list = ProfileList::new();
        list.add_profile(Profile::new("Profile A".to_string(), PathBuf::from("/pathA")));
        list.add_profile(Profile::new("Profile B".to_string(), PathBuf::from("/pathB")));
        list.add_profile(Profile::new("Profile C".to_string(), PathBuf::from("/pathC")));

        // First profile should be active
        assert_eq!(list.active_profile, Some(0));
        assert_eq!(list.get_active_profile().unwrap().name, "Profile A");

        // Change active profile to second one
        list.set_active_profile(1);
        assert_eq!(list.active_profile, Some(1));
        assert_eq!(list.get_active_profile().unwrap().name, "Profile B");

        // Change to third profile
        list.set_active_profile(2);
        assert_eq!(list.active_profile, Some(2));
        assert_eq!(list.get_active_profile().unwrap().name, "Profile C");

        // Serialize and deserialize to simulate save/load
        let json = serde_json::to_string(&list).unwrap();
        let loaded: ProfileList = serde_json::from_str(&json).unwrap();

        // Verify active profile persisted
        assert_eq!(loaded.active_profile, Some(2));
        assert_eq!(loaded.get_active_profile().unwrap().name, "Profile C");
    }
}

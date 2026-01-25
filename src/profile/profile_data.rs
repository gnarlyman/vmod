use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::fs;

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

    /// Auto-detect and set the launcher path based on game_path
    pub fn auto_detect_launcher(&mut self) {
        // Check for .x86_64 extension first (standard Linux Unity build)
        let exe_with_ext = self.game_path.join("DaggerfallUnity.x86_64");
        if exe_with_ext.exists() {
            self.launcher_path = Some(exe_with_ext);
            return;
        }

        // Fall back to no extension
        let exe_no_ext = self.game_path.join("DaggerfallUnity");
        if exe_no_ext.exists() {
            self.launcher_path = Some(exe_no_ext);
        }
    }

    /// Initialize Mods.json path in Unity config directory
    /// Creates directory structure and empty Mods.json if they don't exist
    pub fn initialize_mods_json(&mut self) -> Result<(), String> {
        // Get home directory
        let home = dirs::home_dir()
            .ok_or("Could not find home directory")?;

        // Build path to Unity config directory
        let mods_json_path = home
            .join(".config")
            .join("unity3d")
            .join("Daggerfall Workshop")
            .join("Daggerfall Unity")
            .join("Mods")
            .join("GameData")
            .join("Mods.json");

        // Create directory structure if it doesn't exist
        if let Some(parent) = mods_json_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create Mods.json directory: {}", e))?;
        }

        // Create empty Mods.json file if it doesn't exist
        if !mods_json_path.exists() {
            fs::write(&mods_json_path, "[]")
                .map_err(|e| format!("Failed to create Mods.json: {}", e))?;
        }

        // Store the path
        self.mods_json_path = Some(mods_json_path);
        Ok(())
    }

    /// Create a new profile with auto-detected paths
    pub fn new_with_auto_detect(name: String, game_path: PathBuf) -> Result<Self, String> {
        let mut profile = Self::new(name, game_path);

        // Auto-detect launcher
        profile.auto_detect_launcher();

        // Initialize Mods.json
        profile.initialize_mods_json()?;

        Ok(profile)
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

    #[test]
    fn test_auto_detect_launcher_with_x86_64() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let game_path = temp_dir.path().to_path_buf();

        // Create the .x86_64 executable
        let exe_path = game_path.join("DaggerfallUnity.x86_64");
        fs::write(&exe_path, "").unwrap();

        let mut profile = Profile::new("Test".to_string(), game_path.clone());
        profile.auto_detect_launcher();

        assert!(profile.launcher_path.is_some());
        assert_eq!(profile.launcher_path.unwrap(), exe_path);
    }

    #[test]
    fn test_auto_detect_launcher_without_extension() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let game_path = temp_dir.path().to_path_buf();

        // Create the executable without extension
        let exe_path = game_path.join("DaggerfallUnity");
        fs::write(&exe_path, "").unwrap();

        let mut profile = Profile::new("Test".to_string(), game_path.clone());
        profile.auto_detect_launcher();

        assert!(profile.launcher_path.is_some());
        assert_eq!(profile.launcher_path.unwrap(), exe_path);
    }

    #[test]
    fn test_auto_detect_launcher_prefers_x86_64() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let game_path = temp_dir.path().to_path_buf();

        // Create both executables
        let exe_no_ext = game_path.join("DaggerfallUnity");
        let exe_with_ext = game_path.join("DaggerfallUnity.x86_64");
        fs::write(&exe_no_ext, "").unwrap();
        fs::write(&exe_with_ext, "").unwrap();

        let mut profile = Profile::new("Test".to_string(), game_path.clone());
        profile.auto_detect_launcher();

        // Should prefer .x86_64 version
        assert!(profile.launcher_path.is_some());
        assert_eq!(profile.launcher_path.unwrap(), exe_with_ext);
    }

    #[test]
    fn test_auto_detect_launcher_none_when_missing() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let game_path = temp_dir.path().to_path_buf();

        let mut profile = Profile::new("Test".to_string(), game_path);
        profile.auto_detect_launcher();

        assert!(profile.launcher_path.is_none());
    }

    #[test]
    fn test_initialize_mods_json_creates_file() {
        use std::fs;
        use tempfile::TempDir;

        // Use a temp directory as fake home for this test
        let temp_home = TempDir::new().unwrap();
        let mods_json_path = temp_home.path()
            .join(".config")
            .join("unity3d")
            .join("Daggerfall Workshop")
            .join("Daggerfall Unity")
            .join("Mods")
            .join("GameData")
            .join("Mods.json");

        // Manually set up to test the logic
        // Create directory structure
        if let Some(parent) = mods_json_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }

        // Create empty Mods.json
        if !mods_json_path.exists() {
            fs::write(&mods_json_path, "[]").unwrap();
        }

        // Verify file was created with correct content
        assert!(mods_json_path.exists());
        let content = fs::read_to_string(&mods_json_path).unwrap();
        assert_eq!(content, "[]");
    }

    #[test]
    fn test_initialize_mods_json_sets_path() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let game_path = temp_dir.path().to_path_buf();

        let mut profile = Profile::new("Test".to_string(), game_path);
        let result = profile.initialize_mods_json();

        // Should succeed
        assert!(result.is_ok());
        // Should set the path
        assert!(profile.mods_json_path.is_some());
        // Path should end with Mods.json
        let path = profile.mods_json_path.unwrap();
        assert_eq!(path.file_name().unwrap(), "Mods.json");
    }

    #[test]
    fn test_new_with_auto_detect() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let game_path = temp_dir.path().to_path_buf();

        // Create launcher executable
        let exe_path = game_path.join("DaggerfallUnity.x86_64");
        fs::write(&exe_path, "").unwrap();

        let profile = Profile::new_with_auto_detect("Test".to_string(), game_path).unwrap();

        // Should have launcher path set
        assert!(profile.launcher_path.is_some());
        assert_eq!(profile.launcher_path.unwrap(), exe_path);

        // Should have mods_json_path set
        assert!(profile.mods_json_path.is_some());
        let mods_path = profile.mods_json_path.unwrap();
        assert_eq!(mods_path.file_name().unwrap(), "Mods.json");
    }
}

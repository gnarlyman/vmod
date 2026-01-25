use std::path::PathBuf;
use vmod::profile::profile_data::{Profile, ProfileList};

#[test]
fn test_profile_list_serialization_roundtrip() {
    // Create a profile list
    let mut list = ProfileList::new();
    list.add_profile(Profile::new(
        "Test Profile 1".to_string(),
        PathBuf::from("/game/path1"),
    ));
    list.add_profile(Profile::new(
        "Test Profile 2".to_string(),
        PathBuf::from("/game/path2"),
    ));
    list.set_active_profile(1);

    // Serialize to JSON
    let json = serde_json::to_string_pretty(&list).unwrap();

    // Deserialize back
    let loaded: ProfileList = serde_json::from_str(&json).unwrap();

    // Verify
    assert_eq!(loaded.profiles.len(), 2);
    assert_eq!(loaded.active_profile, Some(1));
    assert_eq!(loaded.profiles[0].name, "Test Profile 1");
    assert_eq!(loaded.profiles[1].name, "Test Profile 2");
    assert_eq!(loaded.profiles[0].game_path, PathBuf::from("/game/path1"));
    assert_eq!(loaded.profiles[1].game_path, PathBuf::from("/game/path2"));
}

#[test]
fn test_empty_profile_list_serialization() {
    let list = ProfileList::new();

    let json = serde_json::to_string(&list).unwrap();
    let loaded: ProfileList = serde_json::from_str(&json).unwrap();

    assert!(loaded.profiles.is_empty());
    assert!(loaded.active_profile.is_none());
}

#[test]
fn test_profile_with_all_fields_serialization() {
    let mut profile = Profile::new("Full Profile".to_string(), PathBuf::from("/game"));
    profile.launcher_path = Some(PathBuf::from("/game/launcher"));
    profile.mods_json_path = Some(PathBuf::from("/game/mods.json"));

    let mut list = ProfileList::new();
    list.add_profile(profile);

    let json = serde_json::to_string(&list).unwrap();
    let loaded: ProfileList = serde_json::from_str(&json).unwrap();

    let loaded_profile = &loaded.profiles[0];
    assert_eq!(loaded_profile.name, "Full Profile");
    assert_eq!(loaded_profile.game_path, PathBuf::from("/game"));
    assert_eq!(loaded_profile.launcher_path, Some(PathBuf::from("/game/launcher")));
    assert_eq!(loaded_profile.mods_json_path, Some(PathBuf::from("/game/mods.json")));
}

#[test]
fn test_profile_active_profile_serialization() {
    let mut list = ProfileList::new();
    list.add_profile(Profile::new("Profile 0".to_string(), PathBuf::from("/path0")));
    list.add_profile(Profile::new("Profile 1".to_string(), PathBuf::from("/path1")));
    list.add_profile(Profile::new("Profile 2".to_string(), PathBuf::from("/path2")));
    list.set_active_profile(2);

    let json = serde_json::to_string(&list).unwrap();
    let loaded: ProfileList = serde_json::from_str(&json).unwrap();

    assert_eq!(loaded.active_profile, Some(2));
    assert_eq!(loaded.get_active_profile().unwrap().name, "Profile 2");
}

#[test]
fn test_json_compatibility() {
    // Test that we can parse a manually created JSON
    let json = r#"{
  "profiles": [
    {
      "name": "Test Profile",
      "game_path": "/test/path",
      "launcher_path": null,
      "mods_json_path": null
    }
  ],
  "active_profile": 0
}"#;

    let loaded: ProfileList = serde_json::from_str(json).unwrap();
    assert_eq!(loaded.profiles.len(), 1);
    assert_eq!(loaded.profiles[0].name, "Test Profile");
    assert_eq!(loaded.active_profile, Some(0));
}

#[test]
fn test_profile_workflow() {
    // Simulate a complete workflow
    let mut list = ProfileList::new();

    // Start with empty list
    assert!(list.profiles.is_empty());
    assert!(list.active_profile.is_none());
    assert!(list.get_active_profile().is_none());

    // Add first profile - should become active
    list.add_profile(Profile::new("Profile 1".to_string(), PathBuf::from("/path1")));
    assert_eq!(list.profiles.len(), 1);
    assert_eq!(list.active_profile, Some(0));
    assert_eq!(list.get_active_profile().unwrap().name, "Profile 1");

    // Add second profile - first should remain active
    list.add_profile(Profile::new("Profile 2".to_string(), PathBuf::from("/path2")));
    assert_eq!(list.profiles.len(), 2);
    assert_eq!(list.active_profile, Some(0));
    assert_eq!(list.get_active_profile().unwrap().name, "Profile 1");

    // Switch to second profile
    list.set_active_profile(1);
    assert_eq!(list.active_profile, Some(1));
    assert_eq!(list.get_active_profile().unwrap().name, "Profile 2");

    // Try invalid profile index - should not change active
    list.set_active_profile(99);
    assert_eq!(list.active_profile, Some(1));
    assert_eq!(list.get_active_profile().unwrap().name, "Profile 2");
}

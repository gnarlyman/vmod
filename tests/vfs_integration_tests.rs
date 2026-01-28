use std::fs;
use tempfile::TempDir;

// Import from vmod lib
use vmod::mod_entry::VirtualFileSystem;

/// Test that VFS correctly handles load order priority
#[test]
fn test_vfs_load_order_priority() {
    let temp_dir = TempDir::new().unwrap();
    let game_streaming_assets = temp_dir.path().join("game").join("StreamingAssets");
    let game_mods = game_streaming_assets.join("Mods");
    fs::create_dir_all(&game_mods).unwrap();

    let profile_mods = temp_dir.path().join("profile_mods");
    fs::create_dir_all(&profile_mods).unwrap();

    // Create 3 mods with overlapping files
    // Mod A (order 0)
    let mod_a = profile_mods.join("mod_a");
    fs::create_dir_all(mod_a.join("Textures")).unwrap();
    fs::create_dir_all(mod_a.join("Mods")).unwrap();
    fs::write(mod_a.join("Textures").join("shared.png"), "texture_from_mod_a").unwrap();
    fs::write(mod_a.join("Textures").join("only_in_a.png"), "only_a").unwrap();
    fs::write(mod_a.join("Mods").join("plugin_a.dfmod"), "plugin_a").unwrap();

    // Mod B (order 1) - overwrites shared.png
    let mod_b = profile_mods.join("mod_b");
    fs::create_dir_all(mod_b.join("Textures")).unwrap();
    fs::create_dir_all(mod_b.join("Sound")).unwrap();
    fs::write(mod_b.join("Textures").join("shared.png"), "texture_from_mod_b").unwrap();
    fs::write(mod_b.join("Sound").join("sound.ogg"), "sound_b").unwrap();

    // Mod C (order 2) - also overwrites shared.png
    let mod_c = profile_mods.join("mod_c");
    fs::create_dir_all(mod_c.join("Textures")).unwrap();
    fs::write(mod_c.join("Textures").join("shared.png"), "texture_from_mod_c").unwrap();
    fs::write(mod_c.join("Textures").join("only_in_c.png"), "only_c").unwrap();

    let vfs = VirtualFileSystem::new(game_mods);

    // Clear any existing symlinks
    vfs.clear_all_symlinks().unwrap();

    // Apply mods in order (0 -> 1 -> 2)
    vfs.enable_mod(&mod_a).unwrap();
    vfs.enable_mod(&mod_b).unwrap();
    vfs.enable_mod(&mod_c).unwrap();

    // Verify results
    let shared_texture = game_streaming_assets.join("Textures").join("shared.png");
    let only_a_texture = game_streaming_assets.join("Textures").join("only_in_a.png");
    let only_c_texture = game_streaming_assets.join("Textures").join("only_in_c.png");
    let sound_file = game_streaming_assets.join("Sound").join("sound.ogg");
    let plugin_file = game_streaming_assets.join("Mods").join("plugin_a.dfmod");

    // shared.png should point to mod_c (highest priority)
    assert!(shared_texture.exists());
    let content = fs::read_to_string(&shared_texture).unwrap();
    assert_eq!(content, "texture_from_mod_c", "shared.png should be from mod_c (highest priority)");

    // only_in_a.png should point to mod_a
    assert!(only_a_texture.exists());
    let content = fs::read_to_string(&only_a_texture).unwrap();
    assert_eq!(content, "only_a");

    // only_in_c.png should point to mod_c
    assert!(only_c_texture.exists());
    let content = fs::read_to_string(&only_c_texture).unwrap();
    assert_eq!(content, "only_c");

    // sound.ogg should point to mod_b
    assert!(sound_file.exists());
    let content = fs::read_to_string(&sound_file).unwrap();
    assert_eq!(content, "sound_b");

    // plugin_a.dfmod should point to mod_a
    assert!(plugin_file.exists());
    let content = fs::read_to_string(&plugin_file).unwrap();
    assert_eq!(content, "plugin_a");
}

/// Test nested directory structures are handled correctly
#[test]
fn test_vfs_nested_directories() {
    let temp_dir = TempDir::new().unwrap();
    let game_streaming_assets = temp_dir.path().join("game").join("StreamingAssets");
    let game_mods = game_streaming_assets.join("Mods");
    fs::create_dir_all(&game_mods).unwrap();

    let profile_mods = temp_dir.path().join("profile_mods");

    // Create mod with deeply nested structure
    let mod_path = profile_mods.join("nested_mod");
    fs::create_dir_all(mod_path.join("Textures").join("UI").join("Buttons")).unwrap();
    fs::create_dir_all(mod_path.join("Sound").join("Effects").join("Combat")).unwrap();

    fs::write(
        mod_path.join("Textures").join("UI").join("Buttons").join("button.png"),
        "button_texture"
    ).unwrap();

    fs::write(
        mod_path.join("Sound").join("Effects").join("Combat").join("sword.ogg"),
        "sword_sound"
    ).unwrap();

    let vfs = VirtualFileSystem::new(game_mods);
    vfs.enable_mod(&mod_path).unwrap();

    // Verify nested files are symlinked correctly
    let button_texture = game_streaming_assets
        .join("Textures")
        .join("UI")
        .join("Buttons")
        .join("button.png");

    let sword_sound = game_streaming_assets
        .join("Sound")
        .join("Effects")
        .join("Combat")
        .join("sword.ogg");

    assert!(button_texture.exists(), "Nested texture should exist");
    assert!(sword_sound.exists(), "Nested sound should exist");

    let button_content = fs::read_to_string(&button_texture).unwrap();
    assert_eq!(button_content, "button_texture");

    let sound_content = fs::read_to_string(&sword_sound).unwrap();
    assert_eq!(sound_content, "sword_sound");
}

/// Test that all recognized DFU folders are symlinked
#[test]
fn test_vfs_all_folder_types() {
    let temp_dir = TempDir::new().unwrap();
    let game_streaming_assets = temp_dir.path().join("game").join("StreamingAssets");
    let game_mods = game_streaming_assets.join("Mods");
    fs::create_dir_all(&game_mods).unwrap();

    let profile_mods = temp_dir.path().join("profile_mods");
    let mod_path = profile_mods.join("complete_mod");

    // Create all recognized folder types
    let folders = vec!["Mods", "Textures", "Sound", "Music", "QuestPacks", "Fonts", "Books", "Text"];

    for folder in &folders {
        let folder_path = mod_path.join(folder);
        fs::create_dir_all(&folder_path).unwrap();
        fs::write(folder_path.join("test_file.dat"), format!("content_{}", folder)).unwrap();
    }

    let vfs = VirtualFileSystem::new(game_mods);
    vfs.enable_mod(&mod_path).unwrap();

    // Verify all folders and files are symlinked
    for folder in &folders {
        let file_path = game_streaming_assets.join(folder).join("test_file.dat");
        assert!(file_path.exists(), "{} folder file should be symlinked", folder);

        let content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, format!("content_{}", folder));
    }
}

/// Test clearing symlinks removes all symlinked files
#[test]
fn test_vfs_clear_all_symlinks() {
    let temp_dir = TempDir::new().unwrap();
    let game_streaming_assets = temp_dir.path().join("game").join("StreamingAssets");
    let game_mods = game_streaming_assets.join("Mods");
    fs::create_dir_all(&game_mods).unwrap();

    let profile_mods = temp_dir.path().join("profile_mods");
    let mod_path = profile_mods.join("test_mod");

    fs::create_dir_all(mod_path.join("Textures")).unwrap();
    fs::create_dir_all(mod_path.join("Mods")).unwrap();
    fs::write(mod_path.join("Textures").join("texture.png"), "texture").unwrap();
    fs::write(mod_path.join("Mods").join("plugin.dfmod"), "plugin").unwrap();

    let vfs = VirtualFileSystem::new(game_mods.clone());
    vfs.enable_mod(&mod_path).unwrap();

    // Verify files are symlinked
    assert!(game_streaming_assets.join("Textures").join("texture.png").exists());
    assert!(game_streaming_assets.join("Mods").join("plugin.dfmod").exists());

    // Clear all symlinks
    vfs.clear_all_symlinks().unwrap();

    // Verify symlinks are removed
    assert!(!game_streaming_assets.join("Textures").join("texture.png").exists());
    assert!(!game_streaming_assets.join("Mods").join("plugin.dfmod").exists());

    // Verify directories still exist (only symlinks removed)
    assert!(game_streaming_assets.join("Textures").exists());
    assert!(game_streaming_assets.join("Mods").exists());
}

/// Test priority override with 5 mods
#[test]
fn test_vfs_priority_with_many_mods() {
    let temp_dir = TempDir::new().unwrap();
    let game_streaming_assets = temp_dir.path().join("game").join("StreamingAssets");
    let game_mods = game_streaming_assets.join("Mods");
    fs::create_dir_all(&game_mods).unwrap();

    let profile_mods = temp_dir.path().join("profile_mods");

    // Create 5 mods that all modify the same file
    for i in 0..5 {
        let mod_path = profile_mods.join(format!("mod_{}", i));
        fs::create_dir_all(mod_path.join("Textures")).unwrap();
        fs::write(
            mod_path.join("Textures").join("common.png"),
            format!("from_mod_{}", i)
        ).unwrap();
    }

    let vfs = VirtualFileSystem::new(game_mods);
    vfs.clear_all_symlinks().unwrap();

    // Apply in order 0 -> 4
    for i in 0..5 {
        let mod_path = profile_mods.join(format!("mod_{}", i));
        vfs.enable_mod(&mod_path).unwrap();
    }

    // Verify last mod wins
    let common_file = game_streaming_assets.join("Textures").join("common.png");
    assert!(common_file.exists());

    let content = fs::read_to_string(&common_file).unwrap();
    assert_eq!(content, "from_mod_4", "Last mod (mod_4) should win");
}

/// Test that symlinks are actual symlinks, not copies
#[test]
fn test_vfs_creates_symlinks_not_copies() {
    let temp_dir = TempDir::new().unwrap();
    let game_streaming_assets = temp_dir.path().join("game").join("StreamingAssets");
    let game_mods = game_streaming_assets.join("Mods");
    fs::create_dir_all(&game_mods).unwrap();

    let profile_mods = temp_dir.path().join("profile_mods");
    let mod_path = profile_mods.join("test_mod");

    fs::create_dir_all(mod_path.join("Textures")).unwrap();
    fs::write(mod_path.join("Textures").join("test.png"), "original").unwrap();

    let vfs = VirtualFileSystem::new(game_mods);
    vfs.enable_mod(&mod_path).unwrap();

    let symlink_path = game_streaming_assets.join("Textures").join("test.png");
    assert!(symlink_path.exists());

    // Verify it's a symlink
    let metadata = symlink_path.symlink_metadata().unwrap();
    assert!(metadata.file_type().is_symlink(), "Should be a symlink, not a regular file");

    // Verify it points to the correct source
    let target = fs::read_link(&symlink_path).unwrap();
    assert_eq!(
        target,
        mod_path.join("Textures").join("test.png"),
        "Symlink should point to original file"
    );
}

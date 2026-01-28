use glib::Object;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::subclass::prelude::*;
use std::cell::RefCell;
use std::path::{Path, PathBuf};

/// Parse Nexus Mods metadata from folder name
/// Format: <name>-<nexus_id>-<version_with_dashes>-<timestamp>
/// Or: <name>-<version_with_dots>-<nexus_id>-<version_with_dashes>-<timestamp>
///
/// Examples:
/// - "An Adventurer's Guide to Witch Covens-697-1-3-1747406905" → (version="1.3", id="697")
/// - "ArchaeologistsGuild-2.6.1-14-2-6-1-1712582888" → (version="2.6.1", id="14")
fn parse_nexus_metadata(folder_name: &str) -> (Option<String>, Option<String>) {
    let parts: Vec<&str> = folder_name.split('-').collect();

    // Need at least 4 parts: name, nexus_id, version_part, timestamp
    if parts.len() < 4 {
        return (None, None);
    }

    // Work backwards from the end
    // Last component should be timestamp (10-digit number starting with 1)
    let timestamp = parts[parts.len() - 1];
    if timestamp.len() != 10 || !timestamp.chars().all(|c| c.is_ascii_digit()) {
        return (None, None);
    }

    // Find the version (components before timestamp, typically 2-3 numeric parts separated by dashes)
    // Also need to find the nexus ID (all-digit component before the version)

    // Scan backwards to collect version components (numbers only)
    let mut version_parts = Vec::new();
    let mut idx = parts.len() - 2; // Start before timestamp

    while idx > 0 {
        let part = parts[idx];
        if part.chars().all(|c| c.is_ascii_digit()) && !part.is_empty() {
            version_parts.insert(0, part);
            idx -= 1;
        } else {
            break;
        }
    }

    // Need at least 2 numeric parts for version (e.g., "1-3" or "2-6-1")
    if version_parts.len() < 2 {
        return (None, None);
    }

    // The last numeric group before version parts should be the nexus ID
    // It's the first element we collected (before the version)
    let nexus_id = version_parts.remove(0);

    // Remaining parts form the version (join with dots)
    let version = version_parts.join(".");

    (Some(version), Some(nexus_id.to_string()))
}

mod imp {
    use super::*;
    use glib::Properties;
    use std::cell::Cell;

    #[derive(Properties, Default)]
    #[properties(wrapper_type = super::ModEntry)]
    pub struct ModEntry {
        #[property(get, set)]
        pub name: RefCell<String>,
        #[property(get, set)]
        pub version: RefCell<String>,
        #[property(get, set)]
        pub enabled: Cell<bool>,
        #[property(get, set)]
        pub order: Cell<u32>,
        pub path: RefCell<PathBuf>,
        #[property(get, set)]
        pub nexus_id: RefCell<Option<String>>,
        #[property(get, set)]
        pub conflict_count: Cell<u32>,
        #[property(get, set)]
        pub section_id: RefCell<Option<String>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ModEntry {
        const NAME: &'static str = "ModEntry";
        type Type = super::ModEntry;
    }

    #[glib::derived_properties]
    impl ObjectImpl for ModEntry {}
}

glib::wrapper! {
    pub struct ModEntry(ObjectSubclass<imp::ModEntry>);
}

impl ModEntry {
    pub fn new(name: String, path: PathBuf, order: u32) -> Self {
        // Parse Nexus metadata from folder name
        let (version, nexus_id) = parse_nexus_metadata(&name);

        // Use parsed version or default to "1.0"
        let version = version.unwrap_or_else(|| "1.0".to_string());

        let obj: Self = Object::builder()
            .property("name", &name)
            .property("version", &version)
            .property("enabled", false)
            .property("order", order)
            .property("nexus-id", &nexus_id)
            .property("conflict-count", 0u32)
            .build();

        obj.imp().path.replace(path);
        obj
    }

    pub fn path(&self) -> PathBuf {
        self.imp().path.borrow().clone()
    }

    pub fn set_path(&self, path: PathBuf) {
        self.imp().path.replace(path);
    }
}

/// Scans a mods directory and returns a list of ModEntry objects
pub struct ModList;

impl ModList {
    /// Scans the mods folder and returns a Vec of ModEntry objects
    pub fn scan_mods_folder(mods_folder: &Path) -> Vec<ModEntry> {
        let mut mods = Vec::new();

        if !mods_folder.exists() {
            log::warn!("Mods folder does not exist: {}", mods_folder.display());
            return mods;
        }

        let entries = match std::fs::read_dir(mods_folder) {
            Ok(entries) => entries,
            Err(e) => {
                log::error!("Failed to read mods folder: {}", e);
                return mods;
            }
        };

        let mut order = 0;
        for entry in entries {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };

            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            // Check if this is a valid mod folder
            if Self::is_valid_mod_folder(&path) {
                let mod_name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("Unknown")
                    .to_string();

                let mod_entry = ModEntry::new(mod_name, path, order);
                mods.push(mod_entry);
                order += 1;
            }
        }

        mods
    }

    /// Checks if a folder is a valid mod folder
    /// Valid mod folders have standard DFU archive structure (Mods/, Textures/, Sound/, etc.)
    /// OR contain recognized content folders/files at the top level
    fn is_valid_mod_folder(path: &Path) -> bool {
        if !path.is_dir() {
            return false;
        }

        let entries = match std::fs::read_dir(path) {
            Ok(e) => e,
            Err(_) => return false,
        };

        // DFU recognized folders (can appear in archive or as loose structure)
        let recognized_folders = vec![
            "Mods", "Textures", "Sound", "Music", "QuestPacks", "Fonts",
            "textures", "models", "scripts", "Models", "Scripts",
            "Books", "Docs", "Text"
        ];

        for entry in entries.flatten() {
            let entry_path = entry.path();
            let file_name = entry_path.file_name().and_then(|n| n.to_str()).unwrap_or("");

            if entry_path.is_dir() {
                // Check if it's a recognized DFU folder
                if recognized_folders.iter().any(|&f| f == file_name) {
                    return true;
                }
            }

            // Check for .dfmod files at top level
            if entry_path.is_file() && file_name.ends_with(".dfmod") {
                return true;
            }
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_mod_entry_creation() {
        let mod_entry = ModEntry::new(
            "TestMod".to_string(),
            PathBuf::from("/test/path"),
            0,
        );

        assert_eq!(mod_entry.name(), "TestMod");
        assert_eq!(mod_entry.version(), "1.0");
        assert!(!mod_entry.enabled());
        assert_eq!(mod_entry.order(), 0);
        assert_eq!(mod_entry.path(), PathBuf::from("/test/path"));
    }

    #[test]
    fn test_mod_entry_property_setters() {
        let mod_entry = ModEntry::new(
            "TestMod".to_string(),
            PathBuf::from("/test/path"),
            0,
        );

        mod_entry.set_name("NewName");
        mod_entry.set_version("2.0.0");
        mod_entry.set_enabled(true);
        mod_entry.set_order(5);

        assert_eq!(mod_entry.name(), "NewName");
        assert_eq!(mod_entry.version(), "2.0.0");
        assert!(mod_entry.enabled());
        assert_eq!(mod_entry.order(), 5);
    }

    #[test]
    fn test_scan_nonexistent_folder() {
        let mods = ModList::scan_mods_folder(&PathBuf::from("/nonexistent/path"));
        assert!(mods.is_empty());
    }

    #[test]
    fn test_scan_empty_folder() {
        let temp_dir = TempDir::new().unwrap();
        let mods = ModList::scan_mods_folder(temp_dir.path());
        assert!(mods.is_empty());
    }

    #[test]
    fn test_scan_folder_with_valid_mods() {
        let temp_dir = TempDir::new().unwrap();
        let mods_path = temp_dir.path();

        // Create mod folders with valid content
        let mod1 = mods_path.join("mod1");
        fs::create_dir(&mod1).unwrap();
        fs::create_dir(mod1.join("textures")).unwrap();

        let mod2 = mods_path.join("mod2");
        fs::create_dir(&mod2).unwrap();
        fs::write(mod2.join("test.dfmod"), "").unwrap();

        let mods = ModList::scan_mods_folder(mods_path);
        assert_eq!(mods.len(), 2);
    }

    #[test]
    fn test_is_valid_mod_folder_with_textures() {
        let temp_dir = TempDir::new().unwrap();
        let mod_path = temp_dir.path().join("test_mod");
        fs::create_dir(&mod_path).unwrap();
        fs::create_dir(mod_path.join("textures")).unwrap();

        assert!(ModList::is_valid_mod_folder(&mod_path));
    }

    #[test]
    fn test_is_valid_mod_folder_with_dfmod() {
        let temp_dir = TempDir::new().unwrap();
        let mod_path = temp_dir.path().join("test_mod");
        fs::create_dir(&mod_path).unwrap();
        fs::write(mod_path.join("test.dfmod"), "").unwrap();

        assert!(ModList::is_valid_mod_folder(&mod_path));
    }

    #[test]
    fn test_is_valid_mod_folder_empty() {
        let temp_dir = TempDir::new().unwrap();
        let mod_path = temp_dir.path().join("test_mod");
        fs::create_dir(&mod_path).unwrap();

        assert!(!ModList::is_valid_mod_folder(&mod_path));
    }

    #[test]
    fn test_is_valid_mod_folder_with_mods_subfolder() {
        let temp_dir = TempDir::new().unwrap();
        let mod_path = temp_dir.path().join("test_mod");
        fs::create_dir(&mod_path).unwrap();

        // Create standard DFU archive structure with Mods folder
        let mods_subfolder = mod_path.join("Mods");
        fs::create_dir(&mods_subfolder).unwrap();
        fs::write(mods_subfolder.join("test.dfmod"), "").unwrap();

        assert!(ModList::is_valid_mod_folder(&mod_path));
    }

    #[test]
    fn test_is_valid_mod_folder_with_textures_only() {
        let temp_dir = TempDir::new().unwrap();
        let mod_path = temp_dir.path().join("test_mod");
        fs::create_dir(&mod_path).unwrap();

        // Create mod with only Textures folder (no .dfmod)
        let textures_folder = mod_path.join("Textures");
        fs::create_dir(&textures_folder).unwrap();
        fs::write(textures_folder.join("texture.png"), "").unwrap();

        assert!(ModList::is_valid_mod_folder(&mod_path));
    }

    #[test]
    fn test_is_valid_mod_folder_with_docs_only() {
        let temp_dir = TempDir::new().unwrap();
        let mod_path = temp_dir.path().join("test_mod");
        fs::create_dir(&mod_path).unwrap();

        // Create only Docs folder
        let docs_folder = mod_path.join("Docs");
        fs::create_dir(&docs_folder).unwrap();
        fs::write(docs_folder.join("readme.txt"), "").unwrap();

        // Docs is in the recognized folders list, so this is valid
        assert!(ModList::is_valid_mod_folder(&mod_path));
    }

    #[test]
    fn test_parse_nexus_metadata() {
        // 3-digit ID (with space)
        assert_eq!(
            parse_nexus_metadata("Aquatic Sprites 1.0-276-1-0-1642914017"),
            (Some("1.0".to_string()), Some("276".to_string()))
        );

        // 2-digit ID (no space - hyphen goes straight to version)
        assert_eq!(
            parse_nexus_metadata("ArchaeologistsGuild-2.6.1-14-2-6-1-1712582888"),
            (Some("2.6.1".to_string()), Some("14".to_string()))
        );

        // 3-digit ID
        assert_eq!(
            parse_nexus_metadata("Ambient Text 1.7-303-1-7-1743021606"),
            (Some("1.7".to_string()), Some("303".to_string()))
        );

        // 1-digit ID
        assert_eq!(
            parse_nexus_metadata("Test Mod 1.0-5-1-0-1234567890"),
            (Some("1.0".to_string()), Some("5".to_string()))
        );

        // 4-digit ID
        assert_eq!(
            parse_nexus_metadata("Big Mod 2.0-1234-5-6-1234567890"),
            (Some("5.6".to_string()), Some("1234".to_string()))
        );

        // 5-digit ID (future-proofing)
        assert_eq!(
            parse_nexus_metadata("Huge Mod 3.0-12345-1-2-1234567890"),
            (Some("1.2".to_string()), Some("12345".to_string()))
        );

        // Edge case: no metadata
        assert_eq!(
            parse_nexus_metadata("CustomMod"),
            (None, None)
        );

        // Edge case: incomplete metadata
        assert_eq!(
            parse_nexus_metadata("Custom Mod 1.0"),
            (None, None)
        );

        // Mod name with apostrophe and multiple words
        assert_eq!(
            parse_nexus_metadata("An Adventurer's Guide to Witch Covens-697-1-3-1747406905"),
            (Some("1.3".to_string()), Some("697".to_string()))
        );
    }

    #[test]
    fn test_mod_entry_with_nexus_metadata() {
        // Test with space
        let mod_entry = ModEntry::new(
            "Aquatic Sprites 1.0-276-1-0-1642914017".to_string(),
            PathBuf::from("/test/path"),
            0,
        );

        assert_eq!(mod_entry.version(), "1.0");
        assert_eq!(mod_entry.nexus_id(), Some("276".to_string()));

        // Test without space
        let mod_entry2 = ModEntry::new(
            "ArchaeologistsGuild-2.6.1-14-2-6-1-1712582888".to_string(),
            PathBuf::from("/test/path"),
            1,
        );

        assert_eq!(mod_entry2.version(), "2.6.1");
        assert_eq!(mod_entry2.nexus_id(), Some("14".to_string()));
    }

    #[test]
    fn test_mod_entry_without_nexus_metadata() {
        let mod_entry = ModEntry::new(
            "CustomMod".to_string(),
            PathBuf::from("/test/path"),
            0,
        );

        assert_eq!(mod_entry.version(), "1.0");
        assert_eq!(mod_entry.nexus_id(), None);
    }
}

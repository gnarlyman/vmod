use glib::Object;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::subclass::prelude::*;
use std::cell::RefCell;
use std::path::{Path, PathBuf};

/// Parse Nexus Mods metadata from folder name
/// Formats supported:
/// - With space: "Mod Name 1.0-276-1-0-1642914017" → (version="1.0", id="276")
/// - Without space: "ModName-2.6.1-14-2-6-1-1712582888" → (version="2.6.1", id="14")
fn parse_nexus_metadata(folder_name: &str) -> (Option<String>, Option<String>) {
    // Try splitting by space first (for "Mod Name 1.0-123-..." format)
    let metadata_part = if let Some(space_pos) = folder_name.rfind(' ') {
        // Has space - take everything after the last space
        &folder_name[space_pos + 1..]
    } else {
        // No space - need to find where version-ID pattern starts
        // Split by hyphens and find first segment that looks like a version
        // followed by an all-numeric segment (the Nexus ID)
        let parts: Vec<&str> = folder_name.split('-').collect();

        for i in 0..parts.len().saturating_sub(1) {
            let current = parts[i];
            let next = parts[i + 1];

            // Check if current part contains a digit (could be version like "2.6.1" or "1.0")
            // and next part is all digits (the Nexus ID)
            if current.chars().any(|c| c.is_ascii_digit())
               && next.chars().all(|c| c.is_ascii_digit())
               && !next.is_empty() {
                // Found version-id pattern
                return (
                    Some(current.to_string()),
                    Some(next.to_string())
                );
            }
        }
        return (None, None);
    };

    // Split metadata part by '-'
    let components: Vec<&str> = metadata_part.split('-').collect();
    if components.len() < 2 {
        return (None, None);
    }

    let version = components[0].trim();
    let nexus_id = components[1].trim();

    // Validate version is not empty
    let version_opt = if version.is_empty() {
        None
    } else {
        Some(version.to_string())
    };

    // Validate nexus_id is numeric
    let nexus_id_opt = if nexus_id.chars().all(|c| c.is_ascii_digit()) && !nexus_id.is_empty() {
        Some(nexus_id.to_string())
    } else {
        None
    };

    (version_opt, nexus_id_opt)
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

        // Use parsed version or default to "Unknown"
        let version = version.unwrap_or_else(|| "Unknown".to_string());

        let obj: Self = Object::builder()
            .property("name", &name)
            .property("version", &version)
            .property("enabled", false)
            .property("order", order)
            .property("nexus-id", &nexus_id)
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
            eprintln!("Mods folder does not exist: {}", mods_folder.display());
            return mods;
        }

        let entries = match std::fs::read_dir(mods_folder) {
            Ok(entries) => entries,
            Err(e) => {
                eprintln!("Failed to read mods folder: {}", e);
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
        assert_eq!(mod_entry.version(), "1.0.0");
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

        // Create only Docs folder (not a recognized content folder)
        let docs_folder = mod_path.join("Docs");
        fs::create_dir(&docs_folder).unwrap();
        fs::write(docs_folder.join("readme.txt"), "").unwrap();

        // Should be invalid - Docs alone doesn't make it a valid mod
        assert!(!ModList::is_valid_mod_folder(&mod_path));
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
            (Some("2.0".to_string()), Some("1234".to_string()))
        );

        // 5-digit ID (future-proofing)
        assert_eq!(
            parse_nexus_metadata("Huge Mod 3.0-12345-1-2-1234567890"),
            (Some("3.0".to_string()), Some("12345".to_string()))
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

        assert_eq!(mod_entry.version(), "Unknown");
        assert_eq!(mod_entry.nexus_id(), None);
    }
}

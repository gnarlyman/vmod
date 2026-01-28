use glib::Object;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::subclass::prelude::*;
use std::cell::{Cell, RefCell};
use serde::{Deserialize, Serialize};

mod imp {
    use super::*;
    use glib::Properties;

    #[derive(Properties, Default)]
    #[properties(wrapper_type = super::SectionHeader)]
    pub struct SectionHeader {
        #[property(get, set)]
        pub name: RefCell<String>,
        #[property(get, set)]
        pub expanded: Cell<bool>,
        #[property(get, set)]
        pub section_id: RefCell<String>,
        /// Order position in the list (for persistence)
        #[property(get, set)]
        pub order: Cell<u32>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SectionHeader {
        const NAME: &'static str = "SectionHeader";
        type Type = super::SectionHeader;
    }

    #[glib::derived_properties]
    impl ObjectImpl for SectionHeader {}
}

glib::wrapper! {
    pub struct SectionHeader(ObjectSubclass<imp::SectionHeader>);
}

impl SectionHeader {
    pub fn new(name: &str, order: u32) -> Self {
        let section_id = uuid::Uuid::new_v4().to_string();
        Object::builder()
            .property("name", name)
            .property("expanded", true)
            .property("section-id", &section_id)
            .property("order", order)
            .build()
    }

    pub fn from_data(data: &SectionData) -> Self {
        Object::builder()
            .property("name", &data.name)
            .property("expanded", data.expanded)
            .property("section-id", &data.section_id)
            .property("order", data.order)
            .build()
    }

    pub fn to_data(&self) -> SectionData {
        SectionData {
            name: self.name(),
            expanded: self.expanded(),
            section_id: self.section_id(),
            order: self.order(),
        }
    }
}

/// Serializable section data for persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectionData {
    pub name: String,
    pub expanded: bool,
    pub section_id: String,
    pub order: u32,
}

/// Sections configuration file
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SectionsConfig {
    pub sections: Vec<SectionData>,
    /// Maps mod folder names to section IDs
    pub mod_assignments: std::collections::HashMap<String, String>,
}

impl SectionsConfig {
    pub fn load(profile_path: &std::path::Path) -> Self {
        let config_path = profile_path.join("sections.json");
        if config_path.exists() {
            match std::fs::read_to_string(&config_path) {
                Ok(content) => {
                    match serde_json::from_str(&content) {
                        Ok(config) => return config,
                        Err(e) => log::error!("Failed to parse sections.json: {}", e),
                    }
                }
                Err(e) => log::error!("Failed to read sections.json: {}", e),
            }
        }
        Self::default()
    }

    pub fn save(&self, profile_path: &std::path::Path) -> Result<(), std::io::Error> {
        let config_path = profile_path.join("sections.json");
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(config_path, content)
    }

    pub fn get_section_for_mod(&self, mod_folder_name: &str) -> Option<&str> {
        self.mod_assignments.get(mod_folder_name).map(|s| s.as_str())
    }

    pub fn assign_mod_to_section(&mut self, mod_folder_name: &str, section_id: &str) {
        self.mod_assignments.insert(mod_folder_name.to_string(), section_id.to_string());
    }

    pub fn remove_mod_from_section(&mut self, mod_folder_name: &str) {
        self.mod_assignments.remove(mod_folder_name);
    }

    pub fn add_section(&mut self, section: SectionData) {
        // Remove existing section with same ID if any
        self.sections.retain(|s| s.section_id != section.section_id);
        self.sections.push(section);
        self.sections.sort_by_key(|s| s.order);
    }

    pub fn remove_section(&mut self, section_id: &str) {
        self.sections.retain(|s| s.section_id != section_id);
        // Also remove all mod assignments to this section
        self.mod_assignments.retain(|_, sid| sid != section_id);
    }

    pub fn update_section_expanded(&mut self, section_id: &str, expanded: bool) {
        if let Some(section) = self.sections.iter_mut().find(|s| s.section_id == section_id) {
            section.expanded = expanded;
        }
    }

    pub fn rename_section(&mut self, section_id: &str, new_name: &str) {
        if let Some(section) = self.sections.iter_mut().find(|s| s.section_id == section_id) {
            section.name = new_name.to_string();
        }
    }
}

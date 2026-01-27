//! Data loading functions for mod list.

use gtk4::prelude::*;
use std::path::PathBuf;

use crate::mod_entry::{ModList, ModState, SectionHeader, SectionsConfig, VirtualFileSystem};
use super::imp::ModListView;
use super::model_utils::update_section_assignments;

impl ModListView {
    pub fn load_mods(&self, mods_folder: &std::path::Path, game_mods_folder: &std::path::Path, profile_name: &str, mods_json_path: &std::path::Path) {
        // Store profile name
        self.profile_name.replace(Some(profile_name.to_string()));

        // Store mods_json_path
        self.mods_json_path.replace(Some(mods_json_path.to_path_buf()));

        // Store profile path for sections config
        self.profile_path.replace(Some(mods_folder.to_path_buf()));

        // Store paths for reload
        self.mods_folder.replace(Some(mods_folder.to_path_buf()));
        self.game_mods_folder.replace(Some(game_mods_folder.to_path_buf()));

        // Create VFS manager
        let vfs = VirtualFileSystem::new(game_mods_folder.to_path_buf());
        self.vfs.replace(Some(vfs));

        // Load saved mod state
        let mod_state = match ModState::load(profile_name) {
            Ok(state) => state,
            Err(_) => ModState::default(),
        };

        // Load sections config
        let sections_config = SectionsConfig::load(mods_folder);
        self.sections_config.replace(sections_config.clone());

        // Scan mods folder
        let mut mods = ModList::scan_mods_folder(mods_folder);

        // Restore enabled state, order, and section_id for all mods
        for mod_entry in &mods {
            // Get mod folder name for state lookup
            let mod_folder_name = mod_entry.path()
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();

            // Restore enabled state from saved state
            let is_enabled = mod_state.is_enabled(&mod_folder_name);
            if is_enabled {
                mod_entry.set_enabled(true);
            }

            // Restore order from saved state if available
            if let Some(saved_order) = mod_state.get_order(&mod_folder_name) {
                mod_entry.set_order(saved_order);
            }

            // Restore section assignment
            if let Some(section_id) = sections_config.get_section_for_mod(&mod_folder_name) {
                mod_entry.set_section_id(section_id);
            }
        }

        // Sort mods by order before adding to model
        mods.sort_by_key(|m| m.order());

        // Create section headers from config
        let sections: Vec<SectionHeader> = sections_config.sections.iter()
            .map(|data| SectionHeader::from_data(data))
            .collect();

        // Populate collapsed_sections from config
        {
            let mut collapsed = self.collapsed_sections.borrow_mut();
            collapsed.clear();
            for section_data in &sections_config.sections {
                if !section_data.expanded {
                    collapsed.insert(section_data.section_id.clone());
                }
            }
        }

        // Build a combined list with sections and mods interleaved by order
        // Create sortable items (order, is_section_priority, object)
        // Sections come before mods at the same order position
        let model = self.model.borrow();
        if let Some(model) = model.as_ref() {
            model.remove_all();

            // Create a combined vec of (order, priority, object) where priority 0=section, 1=mod
            let mut items: Vec<(u32, u8, gtk4::glib::Object)> = Vec::new();

            for section in sections {
                items.push((section.order(), 0, section.upcast()));
            }
            for mod_entry in mods {
                items.push((mod_entry.order(), 1, mod_entry.upcast()));
            }

            // Sort by order, then by priority (sections first at same position)
            items.sort_by_key(|(order, priority, _)| (*order, *priority));

            // Add to model
            for (_, _, obj) in items {
                model.append(&obj);
            }

            // Assign mods to sections based on position
            update_section_assignments(model);
        }

        // Load Mods.json into ModsJsonView
        if let Some(mods_json_view) = self.mods_json_view.borrow().as_ref() {
            mods_json_view.load_mods_json(mods_json_path);
        }

        // Rebuild VFS with loaded state
        self.rebuild_vfs();
    }

    pub fn refresh_list(&self) {
        // Trigger a re-render by signaling model changed
        if let Some(model) = self.model.borrow().as_ref() {
            model.items_changed(0, model.n_items(), model.n_items());
        }
    }

    /// Reload mods using stored paths (used after backup restore)
    /// Also triggers conflict scanning to refresh dfmod data
    pub fn reload(&self) {
        let mods_folder = self.mods_folder.borrow().clone();
        let game_mods_folder = self.game_mods_folder.borrow().clone();
        let profile_name = self.profile_name.borrow().clone();
        let mods_json_path = self.mods_json_path.borrow().clone();

        if let (Some(mods_folder), Some(game_mods_folder), Some(profile_name), Some(mods_json_path)) =
            (mods_folder, game_mods_folder, profile_name, mods_json_path)
        {
            self.load_mods(&mods_folder, &game_mods_folder, &profile_name, &mods_json_path);

            // Trigger conflict scan to load dfmod data and gather conflicts
            self.trigger_conflict_scan();
        }
    }

    pub fn get_vfs(&self) -> Option<VirtualFileSystem> {
        self.vfs.borrow().as_ref().map(|vfs| {
            VirtualFileSystem::new(PathBuf::from(&vfs.game_mods_folder))
        })
    }
}

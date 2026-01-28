//! Mods.json synchronization logic.

use gtk4::prelude::*;

use crate::mod_entry::{load_mods_json, save_mods_json, ModEntry, ModsJsonEntry};
use super::imp::ModListView;

impl ModListView {
    /// Apply all changes: rebuild VFS and update Mods.json
    pub fn apply_changes(&self) {
        // Step 1: Rebuild VFS
        self.rebuild_vfs();

        // Step 2: Save current ModsJsonView state first (preserves manual reordering)
        // Then sync with enabled mods
        if let Err(e) = self.sync_mods_json() {
            log::error!("Failed to sync Mods.json: {}", e);
        }
    }

    /// Sync Mods.json with enabled mods, preserving manual reordering
    fn sync_mods_json(&self) -> Result<(), String> {
        let model = self.model.borrow();
        let mods_json_path = self.mods_json_path.borrow();
        let mods_json_view = self.mods_json_view.borrow();

        let model = model.as_ref().ok_or("Model not initialized")?;
        let mods_json_path = mods_json_path.as_ref().ok_or("Mods.json path not set")?;
        let mods_json_view = mods_json_view.as_ref().ok_or("ModsJsonView not initialized")?;

        // Step 1: Save current ModsJsonView state (preserves manual reordering)
        mods_json_view.save_mods_json()?;

        // Step 2: Load the saved state
        let mut existing_entries = load_mods_json(mods_json_path)?;

        // Step 3: Get list of currently enabled mod folders with .dfmod files
        let mut enabled_mod_names = std::collections::HashSet::new();
        for i in 0..model.n_items() {
            if let Some(obj) = model.item(i) {
                if let Ok(mod_entry) = obj.downcast::<ModEntry>() {
                    if mod_entry.enabled() {
                        // Check if this mod has .dfmod files
                        if let Ok(dfmod_infos) = crate::mod_entry::parse_dfmod_basic(&mod_entry.path()) {
                            for dfmod_info in dfmod_infos {
                                enabled_mod_names.insert(dfmod_info.file_name.clone());
                            }
                        }
                    }
                }
            }
        }

        // Step 4: Remove entries for mods that are no longer enabled
        existing_entries.retain(|entry| enabled_mod_names.contains(&entry.file_name));

        // Step 5: Add new mods that aren't in Mods.json yet
        let existing_file_names: std::collections::HashSet<_> =
            existing_entries.iter().map(|e| e.file_name.clone()).collect();

        let mut next_priority = existing_entries.iter()
            .map(|e| e.load_priority)
            .max()
            .unwrap_or(0)
            .saturating_add(1);

        for i in 0..model.n_items() {
            if let Some(obj) = model.item(i) {
                if let Ok(mod_entry) = obj.downcast::<ModEntry>() {
                    if mod_entry.enabled() {
                        if let Ok(dfmod_infos) = crate::mod_entry::parse_dfmod_basic(&mod_entry.path()) {
                            for dfmod_info in dfmod_infos {
                                if !existing_file_names.contains(&dfmod_info.file_name) {
                                    existing_entries.push(ModsJsonEntry {
                                        file_name: dfmod_info.file_name,
                                        title: dfmod_info.title,
                                        enabled: true,
                                        load_priority: next_priority,
                                    });
                                    next_priority += 1;
                                }
                            }
                        }
                    }
                }
            }
        }

        // Step 6: Sort by load_priority
        existing_entries.sort_by_key(|e| e.load_priority);

        // Step 7: Renumber priorities sequentially (fill gaps)
        for (i, entry) in existing_entries.iter_mut().enumerate() {
            entry.load_priority = i as u32;
        }

        // Step 8: Save to disk
        save_mods_json(mods_json_path, &existing_entries)?;

        // Step 9: Reload ModsJsonView (sorted by priority)
        mods_json_view.load_mods_json(mods_json_path);

        Ok(())
    }
}

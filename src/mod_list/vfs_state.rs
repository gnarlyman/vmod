//! VFS (Virtual File System) state management functions.

use gtk4::prelude::*;
use gtk4::gio;
use std::cell::RefCell;
use std::rc::Rc;

use crate::mod_entry::{ModEntry, ModState, VirtualFileSystem};

/// Rebuild all symlinks based on enabled mods and their order
/// Static version for use in closures
pub fn rebuild_vfs_static(model: &RefCell<Option<gio::ListStore>>, vfs: &RefCell<Option<VirtualFileSystem>>) {
    let model_borrow = model.borrow();
    let vfs_borrow = vfs.borrow();

    if let (Some(model), Some(vfs)) = (model_borrow.as_ref(), vfs_borrow.as_ref()) {
        // Clear all existing symlinks
        if let Err(e) = vfs.clear_all_symlinks() {
            eprintln!("Failed to clear symlinks: {}", e);
            return;
        }

        // Get all mods sorted by order
        let n_items = model.n_items();
        let mut mods: Vec<ModEntry> = Vec::new();

        for i in 0..n_items {
            if let Some(item) = model.item(i) {
                if let Ok(mod_entry) = item.downcast::<ModEntry>() {
                    mods.push(mod_entry);
                }
            }
        }

        // Sort by order (lowest to highest)
        mods.sort_by_key(|m| m.order());

        // Apply enabled mods in order
        for mod_entry in mods {
            if mod_entry.enabled() {
                let mod_path = mod_entry.path();
                if let Err(e) = vfs.enable_mod(&mod_path) {
                    eprintln!("Failed to enable mod {}: {}", mod_entry.name(), e);
                }
            }
        }
    }
}

/// Save mod enabled state to disk
/// Static version for use in closures
pub fn save_mod_state_static(model: &RefCell<Option<gio::ListStore>>, profile_name: &Rc<RefCell<Option<String>>>) {
    let model_borrow = model.borrow();
    let profile_name_borrow = profile_name.borrow();

    if let (Some(model), Some(profile_name)) = (model_borrow.as_ref(), profile_name_borrow.as_ref()) {
        let mut mod_state = ModState::new();

        // Collect enabled state from all mods
        let n_items = model.n_items();

        for i in 0..n_items {
            if let Some(item) = model.item(i) {
                if let Ok(mod_entry) = item.downcast::<ModEntry>() {
                    let mod_folder_name = mod_entry.path()
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("")
                        .to_string();

                    let enabled = mod_entry.enabled();
                    let order = mod_entry.order();

                    mod_state.set_enabled(mod_folder_name.clone(), enabled);
                    mod_state.set_order(mod_folder_name, order);
                }
            }
        }

        // Save to disk
        if let Err(e) = mod_state.save(profile_name) {
            eprintln!("Failed to save mod state: {}", e);
        }
    }
}

//! Model utility functions for finding positions and managing ordering.

use gtk4::prelude::*;
use gtk4::gio;
use gtk4::glib;
use gtk4::SingleSelection;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use crate::mod_entry::{ModEntry, SectionHeader, SectionsConfig};

/// Find the position of a mod in the model
pub fn find_mod_position(model: &gio::ListStore, target: &ModEntry) -> u32 {
    let target_path = target.path();
    for i in 0..model.n_items() {
        if let Some(item) = model.item(i) {
            if let Ok(entry) = item.downcast::<ModEntry>() {
                if entry.path() == target_path {
                    return i;
                }
            }
        }
    }
    0
}

/// Find the position of any item (ModEntry or SectionHeader) in the underlying model
pub fn find_item_position_in_model(model: &gio::ListStore, target: &glib::Object) -> Option<u32> {
    // Check if target is a ModEntry
    if let Some(mod_entry) = target.downcast_ref::<ModEntry>() {
        let target_path = mod_entry.path();
        for i in 0..model.n_items() {
            if let Some(item) = model.item(i) {
                if let Some(entry) = item.downcast_ref::<ModEntry>() {
                    if entry.path() == target_path {
                        return Some(i);
                    }
                }
            }
        }
    }
    // Check if target is a SectionHeader
    else if let Some(section) = target.downcast_ref::<SectionHeader>() {
        let target_id = section.section_id();
        for i in 0..model.n_items() {
            if let Some(item) = model.item(i) {
                if let Some(sec) = item.downcast_ref::<SectionHeader>() {
                    if sec.section_id() == target_id {
                        return Some(i);
                    }
                }
            }
        }
    }
    None
}

/// Find the position of an item in the selection model (which wraps the filtered model)
pub fn find_item_position_in_selection(selection: &SingleSelection, target: &glib::Object) -> Option<u32> {
    let n_items = selection.n_items();

    // Check if target is a ModEntry
    if let Some(mod_entry) = target.downcast_ref::<ModEntry>() {
        let target_path = mod_entry.path();
        for i in 0..n_items {
            if let Some(item) = selection.item(i) {
                if let Some(entry) = item.downcast_ref::<ModEntry>() {
                    if entry.path() == target_path {
                        return Some(i);
                    }
                }
            }
        }
    }
    // Check if target is a SectionHeader
    else if let Some(section) = target.downcast_ref::<SectionHeader>() {
        let target_id = section.section_id();
        for i in 0..n_items {
            if let Some(item) = selection.item(i) {
                if let Some(sec) = item.downcast_ref::<SectionHeader>() {
                    if sec.section_id() == target_id {
                        return Some(i);
                    }
                }
            }
        }
    }
    None
}

/// Helper to get order value from any item (ModEntry or SectionHeader)
pub fn get_item_order(item: &glib::Object) -> Option<u32> {
    if let Some(mod_entry) = item.downcast_ref::<ModEntry>() {
        Some(mod_entry.order())
    } else if let Some(section) = item.downcast_ref::<SectionHeader>() {
        Some(section.order())
    } else {
        None
    }
}

/// Helper to set order value on any item (ModEntry or SectionHeader)
pub fn set_item_order(item: &glib::Object, order: u32) {
    if let Some(mod_entry) = item.downcast_ref::<ModEntry>() {
        mod_entry.set_order(order);
    } else if let Some(section) = item.downcast_ref::<SectionHeader>() {
        section.set_order(order);
    }
}

/// Rebuild model sorted by order (handles both ModEntry and SectionHeader)
pub fn rebuild_model_sorted(model_store: &gio::ListStore) {
    let n_items = model_store.n_items();
    let mut items: Vec<(u32, u8, glib::Object)> = Vec::new();

    for i in 0..n_items {
        if let Some(item) = model_store.item(i) {
            if let Some(mod_entry) = item.downcast_ref::<ModEntry>() {
                items.push((mod_entry.order(), 1, item)); // priority 1 for mods
            } else if let Some(section) = item.downcast_ref::<SectionHeader>() {
                items.push((section.order(), 0, item)); // priority 0 for sections
            }
        }
    }

    // Sort by order, then by priority (sections first at same position)
    items.sort_by_key(|(order, priority, _)| (*order, *priority));

    model_store.remove_all();
    for (_, _, obj) in items {
        model_store.append(&obj);
    }

    // Update section assignments based on position
    update_section_assignments(model_store);
}

/// Scan the list and assign each mod to the section header above it
pub fn update_section_assignments(model_store: &gio::ListStore) {
    let n_items = model_store.n_items();
    let mut current_section_id: Option<String> = None;

    for i in 0..n_items {
        if let Some(item) = model_store.item(i) {
            if let Some(section) = item.downcast_ref::<SectionHeader>() {
                // Update current section
                current_section_id = Some(section.section_id());
            } else if let Some(mod_entry) = item.downcast_ref::<ModEntry>() {
                // Assign mod to current section (or None if before any section)
                // Use the property system directly for Option<String>
                mod_entry.set_property("section-id", &current_section_id);
            }
        }
    }
}

/// Sync section data from model to config and save to disk
pub fn sync_sections_to_config(
    model_store: &gio::ListStore,
    sections_config: &Rc<RefCell<SectionsConfig>>,
    profile_path: &Rc<RefCell<Option<PathBuf>>>,
) {
    let mut config = sections_config.borrow_mut();

    // Update all sections in config with current data from model
    for i in 0..model_store.n_items() {
        if let Some(item) = model_store.item(i) {
            if let Some(section) = item.downcast_ref::<SectionHeader>() {
                // Update or add section in config
                config.add_section(section.to_data());
            }
        }
    }

    // Save to disk
    drop(config);
    if let Some(path) = profile_path.borrow().as_ref() {
        let _ = sections_config.borrow().save(path);
    }
}

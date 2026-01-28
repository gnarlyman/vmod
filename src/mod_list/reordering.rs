//! Mod reordering operations (move up/down/top/bottom, enable/disable all, add section).

use gtk4::prelude::*;
use gtk4::{gio, ColumnView, CustomFilter, FilterChange, SingleSelection};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use crate::mod_entry::{ModEntry, SectionHeader, SectionsConfig, VirtualFileSystem};
use super::imp::ModListView;
use super::model_utils::{
    find_item_position_in_selection, find_mod_position, get_item_order, rebuild_model_sorted,
    set_item_order, sync_sections_to_config, update_section_assignments,
};
use super::vfs_state::save_mod_state_static;

impl ModListView {
    pub fn move_mod_up_static(
        model: &RefCell<Option<gio::ListStore>>,
        position: u32,
        _vfs: &RefCell<Option<VirtualFileSystem>>,
        profile_name: &Rc<RefCell<Option<String>>>,
        selection: &SingleSelection,
        sections_config: &Rc<RefCell<SectionsConfig>>,
        profile_path: &Rc<RefCell<Option<PathBuf>>>,
    ) {
        let model_borrow = model.borrow();
        if let Some(model_store) = model_borrow.as_ref() {
            if position == 0 || position >= model_store.n_items() {
                return; // Already at top or invalid
            }

            // Get current and previous items
            let current_item = match model_store.item(position) {
                Some(item) => item,
                None => return,
            };
            let prev_item = match model_store.item(position - 1) {
                Some(item) => item,
                None => return,
            };

            // Swap order values
            let current_order = get_item_order(&current_item).unwrap_or(position);
            let prev_order = get_item_order(&prev_item).unwrap_or(position - 1);

            set_item_order(&current_item, prev_order);
            set_item_order(&prev_item, current_order);

            // Atomic swap using splice - emits only ONE items-changed signal
            // Remove 2 items at position-1 and insert them in swapped order
            model_store.splice(position - 1, 2, &[current_item.clone(), prev_item.clone()]);

            // Update section assignments after the swap
            update_section_assignments(model_store);

            // Restore selection - find the moved item's position in the filtered selection model
            if let Some(sel_pos) = find_item_position_in_selection(selection, &current_item) {
                selection.set_selected(sel_pos);
            }

            // Sync section orders and save
            sync_sections_to_config(model_store, sections_config, profile_path);

            drop(model_borrow);
            save_mod_state_static(model, profile_name);
        }
    }

    /// Move a mod down in the list (static version for closures)
    pub fn move_mod_down_static(
        model: &RefCell<Option<gio::ListStore>>,
        position: u32,
        _vfs: &RefCell<Option<VirtualFileSystem>>,
        profile_name: &Rc<RefCell<Option<String>>>,
        selection: &SingleSelection,
        sections_config: &Rc<RefCell<SectionsConfig>>,
        profile_path: &Rc<RefCell<Option<PathBuf>>>,
    ) {
        let model_borrow = model.borrow();
        if let Some(model_store) = model_borrow.as_ref() {
            if position >= model_store.n_items() - 1 {
                return; // Already at bottom
            }

            // Get current and next items
            let current_item = match model_store.item(position) {
                Some(item) => item,
                None => return,
            };
            let next_item = match model_store.item(position + 1) {
                Some(item) => item,
                None => return,
            };

            // Swap order values
            let current_order = get_item_order(&current_item).unwrap_or(position);
            let next_order = get_item_order(&next_item).unwrap_or(position + 1);

            set_item_order(&current_item, next_order);
            set_item_order(&next_item, current_order);

            // Atomic swap using splice - emits only ONE items-changed signal
            // Remove 2 items at position and insert them in swapped order
            model_store.splice(position, 2, &[next_item.clone(), current_item.clone()]);

            // Update section assignments after the swap
            update_section_assignments(model_store);

            // Restore selection - find the moved item's position in the filtered selection model
            if let Some(sel_pos) = find_item_position_in_selection(selection, &current_item) {
                selection.set_selected(sel_pos);
            }

            // Sync section orders and save
            sync_sections_to_config(model_store, sections_config, profile_path);

            drop(model_borrow);
            save_mod_state_static(model, profile_name);
        }
    }

    /// Move a mod to top of the list (static version for closures)
    pub fn move_mod_to_top_static(
        model: &RefCell<Option<gio::ListStore>>,
        position: u32,
        _vfs: &RefCell<Option<VirtualFileSystem>>,
        profile_name: &Rc<RefCell<Option<String>>>,
        selection: &SingleSelection,
        sections_config: &Rc<RefCell<SectionsConfig>>,
        profile_path: &Rc<RefCell<Option<PathBuf>>>,
    ) {
        let model_borrow = model.borrow();
        if let Some(model_store) = model_borrow.as_ref() {
            if position == 0 || position >= model_store.n_items() {
                return; // Already at top or invalid
            }

            let current_item = match model_store.item(position) {
                Some(item) => item,
                None => return,
            };

            let current_order = get_item_order(&current_item).unwrap_or(position);

            // Set this item's order to 0
            set_item_order(&current_item, 0);

            // Shift all items with order < current_order up by 1
            for i in 0..model_store.n_items() {
                if i == position {
                    continue;
                }
                if let Some(item) = model_store.item(i) {
                    if let Some(order) = get_item_order(&item) {
                        if order < current_order {
                            set_item_order(&item, order + 1);
                        }
                    }
                }
            }

            // Rebuild model sorted
            rebuild_model_sorted(model_store);

            // Restore selection - find the moved item's position in the filtered selection model
            if let Some(new_pos) = find_item_position_in_selection(selection, &current_item) {
                selection.set_selected(new_pos);
            }

            // Sync section orders and save
            sync_sections_to_config(model_store, sections_config, profile_path);

            drop(model_borrow);
            save_mod_state_static(model, profile_name);
        }
    }

    /// Move a mod to bottom of the list (static version for closures)
    pub fn move_mod_to_bottom_static(
        model: &RefCell<Option<gio::ListStore>>,
        position: u32,
        _vfs: &RefCell<Option<VirtualFileSystem>>,
        profile_name: &Rc<RefCell<Option<String>>>,
        selection: &SingleSelection,
        sections_config: &Rc<RefCell<SectionsConfig>>,
        profile_path: &Rc<RefCell<Option<PathBuf>>>,
    ) {
        let model_borrow = model.borrow();
        if let Some(model_store) = model_borrow.as_ref() {
            let n_items = model_store.n_items();
            if n_items == 0 || position >= n_items - 1 {
                return; // Already at bottom or invalid
            }

            let last_position = n_items - 1;

            let current_item = match model_store.item(position) {
                Some(item) => item,
                None => return,
            };

            let current_order = get_item_order(&current_item).unwrap_or(position);

            // Set this item's order to last
            set_item_order(&current_item, last_position);

            // Shift all items with order > current_order down by 1
            for i in 0..n_items {
                if i == position {
                    continue;
                }
                if let Some(item) = model_store.item(i) {
                    if let Some(order) = get_item_order(&item) {
                        if order > current_order {
                            set_item_order(&item, order - 1);
                        }
                    }
                }
            }

            // Rebuild model sorted
            rebuild_model_sorted(model_store);

            // Restore selection - find the moved item's position in the filtered selection model
            if let Some(new_pos) = find_item_position_in_selection(selection, &current_item) {
                selection.set_selected(new_pos);
            }

            // Sync section orders and save
            sync_sections_to_config(model_store, sections_config, profile_path);

            drop(model_borrow);
            save_mod_state_static(model, profile_name);
        }
    }

    /// Enable all mods (static version for closures)
    pub fn enable_all_mods_static(
        model: &RefCell<Option<gio::ListStore>>,
        profile_name: &Rc<RefCell<Option<String>>>
    ) {
        let model_borrow = model.borrow();
        if let Some(model_store) = model_borrow.as_ref() {
            // Enable all mods
            for i in 0..model_store.n_items() {
                if let Some(item) = model_store.item(i) {
                    if let Ok(mod_entry) = item.downcast::<ModEntry>() {
                        mod_entry.set_enabled(true);
                    }
                }
            }

            drop(model_borrow);
            save_mod_state_static(model, profile_name);
        }
    }

    /// Disable all mods (static version for closures)
    pub fn disable_all_mods_static(
        model: &RefCell<Option<gio::ListStore>>,
        profile_name: &Rc<RefCell<Option<String>>>
    ) {
        let model_borrow = model.borrow();
        if let Some(model_store) = model_borrow.as_ref() {
            // Disable all mods
            for i in 0..model_store.n_items() {
                if let Some(item) = model_store.item(i) {
                    if let Ok(mod_entry) = item.downcast::<ModEntry>() {
                        mod_entry.set_enabled(false);
                    }
                }
            }

            drop(model_borrow);
            save_mod_state_static(model, profile_name);
        }
    }

    /// Add a new section at the top of the list
    pub fn add_section_at_selection(
        model: &RefCell<Option<gio::ListStore>>,
        _selection: &SingleSelection,
        sections_config: &Rc<RefCell<SectionsConfig>>,
        profile_path: &Rc<RefCell<Option<PathBuf>>>,
        filter: &RefCell<Option<CustomFilter>>,
        column_view: &ColumnView,
    ) {
        let model_borrow = model.borrow();
        if let Some(model_store) = model_borrow.as_ref() {
            // Always add sections at the top
            let position = 0u32;

            // Create new section with default name
            let section = SectionHeader::new("New Section", position);

            // Insert into model at position
            model_store.insert(position, &section);

            // Update order of subsequent items (position + 1 is safe now since we bounded position)
            for i in (position + 1)..model_store.n_items() {
                if let Some(item) = model_store.item(i) {
                    if let Some(mod_entry) = item.downcast_ref::<ModEntry>() {
                        mod_entry.set_order(i);
                    } else if let Some(sec) = item.downcast_ref::<SectionHeader>() {
                        sec.set_order(i);
                    }
                }
            }

            // Update section assignments based on new positions
            update_section_assignments(model_store);

            // Save section to config
            let section_data = section.to_data();
            sections_config.borrow_mut().add_section(section_data);

            if let Some(path) = profile_path.borrow().as_ref() {
                let _ = sections_config.borrow().save(path);
            }

            // Update filter to reflect changes
            if let Some(filter) = filter.borrow().as_ref() {
                filter.changed(FilterChange::Different);
            }

            // Scroll to top to show the new section
            use gtk4::prelude::ScrollableExt;
            if let Some(vadj) = column_view.vadjustment() {
                vadj.set_value(0.0);
            }
        }
    }

    /// Public API: Move a mod up
    pub fn move_mod_up(&self, mod_entry: &ModEntry) {
        if let Some(selection) = self.selection_model.borrow().as_ref() {
            if let Some(model) = self.model.borrow().as_ref() {
                let position = find_mod_position(model, mod_entry);
                if position < model.n_items() {
                    Self::move_mod_up_static(&self.model, position, &self.vfs, &self.profile_name, selection, &self.sections_config, &self.profile_path);
                }
            }
        }
    }

    /// Public API: Move a mod down
    pub fn move_mod_down(&self, mod_entry: &ModEntry) {
        if let Some(selection) = self.selection_model.borrow().as_ref() {
            if let Some(model) = self.model.borrow().as_ref() {
                let position = find_mod_position(model, mod_entry);
                if position < model.n_items() {
                    Self::move_mod_down_static(&self.model, position, &self.vfs, &self.profile_name, selection, &self.sections_config, &self.profile_path);
                }
            }
        }
    }
}

/// Remove a section from the model and config
pub fn remove_section_static(
    model: &RefCell<Option<gio::ListStore>>,
    section_id: &str,
    sections_config: &Rc<RefCell<SectionsConfig>>,
    profile_path: &Rc<RefCell<Option<PathBuf>>>,
    filter: &RefCell<Option<CustomFilter>>,
) {
    let model_borrow = model.borrow();
    if let Some(model_store) = model_borrow.as_ref() {
        // Find section position in model by section_id
        let mut position: Option<u32> = None;
        for i in 0..model_store.n_items() {
            if let Some(item) = model_store.item(i) {
                if let Some(section) = item.downcast_ref::<SectionHeader>() {
                    if section.section_id() == section_id {
                        position = Some(i);
                        break;
                    }
                }
            }
        }

        if let Some(pos) = position {
            // Remove from model
            model_store.remove(pos);

            // Update order of remaining items
            for i in pos..model_store.n_items() {
                if let Some(item) = model_store.item(i) {
                    if let Some(mod_entry) = item.downcast_ref::<ModEntry>() {
                        mod_entry.set_order(i);
                    } else if let Some(sec) = item.downcast_ref::<SectionHeader>() {
                        sec.set_order(i);
                    }
                }
            }

            // Update section assignments
            update_section_assignments(model_store);

            // Remove from config
            sections_config.borrow_mut().remove_section(section_id);

            // Save config to disk
            if let Some(path) = profile_path.borrow().as_ref() {
                let _ = sections_config.borrow().save(path);
            }

            // Trigger filter update
            if let Some(filter) = filter.borrow().as_ref() {
                filter.changed(FilterChange::Different);
            }
        }
    }
}

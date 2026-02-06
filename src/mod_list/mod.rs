mod imp;
mod model_utils;
mod vfs_state;
mod data_loading;
mod reordering;
mod mods_json_sync;
mod conflict_scan;
mod backup_ui;
mod columns;
mod ui_builder;
mod metadata_fetch;

use gtk4::subclass::prelude::*;
use gtk4::{gio, glib, Box};
use crate::mod_entry::ModEntry;

glib::wrapper! {
    pub struct ModListView(ObjectSubclass<imp::ModListView>)
        @extends Box, gtk4::Widget,
        @implements gio::ActionGroup, gio::ActionMap, gtk4::Accessible, gtk4::Buildable,
                    gtk4::ConstraintTarget, gtk4::Orientable;
}

impl ModListView {
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    /// Loads mods from the specified folder and populates the list
    pub fn load_mods(&self, mods_folder: &std::path::Path, game_mods_folder: &std::path::Path, profile_name: &str, mods_json_path: &std::path::Path) {
        self.imp().load_mods(mods_folder, game_mods_folder, profile_name, mods_json_path);
    }

    /// Refreshes the mod list display
    pub fn refresh(&self) {
        self.imp().refresh_list();
    }

    /// Rebuilds VFS symlinks for all enabled mods in order
    pub fn rebuild_vfs(&self) {
        self.imp().rebuild_vfs();
    }

    /// Moves a mod up in the load order
    pub fn move_mod_up(&self, mod_entry: &ModEntry) {
        self.imp().move_mod_up(mod_entry);
    }

    /// Moves a mod down in the load order
    pub fn move_mod_down(&self, mod_entry: &ModEntry) {
        self.imp().move_mod_down(mod_entry);
    }

    /// Reload mods using stored paths (used after backup restore)
    pub fn reload(&self) {
        self.imp().reload();
    }

    /// Trigger batch metadata fetch (mod names only) from Nexus Mods API
    pub fn fetch_metadata(&self) {
        let imp = self.imp();
        let nexus_config = crate::nexus_api::NexusConfig::load();
        if let Some(api_key) = nexus_config.api_key {
            // Reuse the existing progress UI widgets
            if let (Some(progress_box), Some(progress_bar), Some(progress_label)) = (
                imp.progress_box.borrow().as_ref().cloned(),
                imp.progress_bar.borrow().as_ref().cloned(),
                imp.progress_label.borrow().as_ref().cloned(),
            ) {
                // Create a dummy button since this is triggered from the menu
                let button = gtk4::Button::new();
                imp::ModListView::start_metadata_fetch(
                    &imp.model,
                    &imp.is_metadata_fetching,
                    &progress_box,
                    &progress_bar,
                    &progress_label,
                    &button,
                    api_key,
                    nexus_config.game_domain,
                );
            }
        } else {
            log::warn!("No Nexus API key configured, cannot fetch metadata");
        }
    }

    /// Trigger version check for all mods with Nexus ID
    pub fn check_all_versions(&self) {
        let imp = self.imp();
        let nexus_config = crate::nexus_api::NexusConfig::load();
        if let Some(api_key) = nexus_config.api_key {
            // Reuse the existing progress UI widgets
            if let (Some(progress_box), Some(progress_bar), Some(progress_label)) = (
                imp.progress_box.borrow().as_ref().cloned(),
                imp.progress_bar.borrow().as_ref().cloned(),
                imp.progress_label.borrow().as_ref().cloned(),
            ) {
                // Create a dummy button since this is triggered from the menu
                let button = gtk4::Button::new();
                imp::ModListView::start_version_check(
                    &imp.model,
                    &imp.is_metadata_fetching,
                    &progress_box,
                    &progress_bar,
                    &progress_label,
                    &button,
                    api_key,
                    nexus_config.game_domain,
                );
            }
        } else {
            log::warn!("No Nexus API key configured, cannot check versions");
        }
    }

    /// Check version for a single mod
    pub fn check_single_version(&self, mod_entry: &ModEntry) {
        let imp = self.imp();
        let nexus_config = crate::nexus_api::NexusConfig::load();
        if let Some(api_key) = nexus_config.api_key {
            if let Some(nexus_id) = mod_entry.nexus_id() {
                imp::ModListView::check_single_mod_version(
                    &imp.model,
                    mod_entry.name(),
                    mod_entry.path(),
                    nexus_id,
                    mod_entry.version(),
                    api_key,
                    nexus_config.game_domain,
                );
            }
        } else {
            log::warn!("No Nexus API key configured, cannot check version");
        }
    }

    /// Refresh the downloads list
    pub fn refresh_downloads(&self) {
        imp::ModListView::refresh_downloads_static(&self.imp().downloads_model);
    }
}

impl Default for ModListView {
    fn default() -> Self {
        Self::new()
    }
}

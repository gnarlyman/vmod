mod imp;

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
}

impl Default for ModListView {
    fn default() -> Self {
        Self::new()
    }
}

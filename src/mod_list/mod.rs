mod imp;

use gtk4::subclass::prelude::*;
use gtk4::{gio, glib, Box};

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
    pub fn load_mods(&self, mods_folder: &std::path::Path, game_mods_folder: &std::path::Path) {
        self.imp().load_mods(mods_folder, game_mods_folder);
    }

    /// Refreshes the mod list display
    pub fn refresh(&self) {
        self.imp().refresh_list();
    }

    /// Rebuilds VFS symlinks for all enabled mods in order
    pub fn rebuild_vfs(&self) {
        self.imp().rebuild_vfs();
    }
}

impl Default for ModListView {
    fn default() -> Self {
        Self::new()
    }
}

mod imp;

use gtk4::{gio, glib};
use gtk4::subclass::prelude::*;
use std::path::PathBuf;

glib::wrapper! {
    pub struct ConflictPanel(ObjectSubclass<imp::ConflictPanel>)
        @extends gtk4::Box, gtk4::Widget,
        @implements gio::ActionGroup, gio::ActionMap, gtk4::Accessible, gtk4::Buildable,
                    gtk4::ConstraintTarget, gtk4::Orientable;
}

impl ConflictPanel {
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    /// Update the panel to show conflicts and files for the selected mod
    pub fn update_for_mod(
        &self,
        mod_name: &str,
        mod_path: &PathBuf,
        enabled_mods: &[(String, PathBuf)],
    ) {
        self.imp().update_for_mod(mod_name, mod_path, enabled_mods);
    }

    /// Clear the panel (when no mod is selected)
    pub fn clear(&self) {
        self.imp().clear();
    }
}

impl Default for ConflictPanel {
    fn default() -> Self {
        Self::new()
    }
}

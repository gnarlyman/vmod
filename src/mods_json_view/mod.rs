mod imp;

use gtk4::{gio, glib};
use gtk4::subclass::prelude::*;
use std::path::Path;

glib::wrapper! {
    pub struct ModsJsonView(ObjectSubclass<imp::ModsJsonView>)
        @extends gtk4::Box, gtk4::Widget,
        @implements gio::ActionGroup, gio::ActionMap, gtk4::Accessible, gtk4::Buildable,
                    gtk4::ConstraintTarget, gtk4::Orientable;
}

impl ModsJsonView {
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    pub fn load_mods_json(&self, mods_json_path: &Path) {
        self.imp().load_mods_json_static(mods_json_path);
    }

    pub fn save_mods_json(&self) -> Result<(), String> {
        self.imp().save_mods_json_static()
    }
}

impl Default for ModsJsonView {
    fn default() -> Self {
        Self::new()
    }
}

mod imp;

use gtk4::{gio, glib};
use gtk4::subclass::prelude::*;
use std::path::PathBuf;

use crate::mod_entry::ModConflictSummary;

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

    /// Update the panel using cached conflict data from a scan
    pub fn update_with_cached_conflicts(
        &self,
        mod_path: &PathBuf,
        conflict_summary: Option<&ModConflictSummary>,
    ) {
        self.imp().update_with_cached_conflicts(mod_path, conflict_summary);
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

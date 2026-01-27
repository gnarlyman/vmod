mod download_item;
mod imp;

pub use download_item::DownloadItem;

use gtk4::{gio, glib};
use gtk4::subclass::prelude::*;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::mod_entry::{ModConflictSummary, DfmodCacheKey};

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

    /// Set the shared dfmod cache reference from ModListView
    pub fn set_dfmod_cache(&self, cache: Arc<Mutex<HashMap<DfmodCacheKey, Vec<String>>>>) {
        self.imp().shared_dfmod_cache.replace(Some(cache));
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

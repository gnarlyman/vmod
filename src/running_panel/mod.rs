mod imp;

use gtk4::{gio, glib};
use gtk4::subclass::prelude::*;
use std::path::PathBuf;

glib::wrapper! {
    pub struct RunningPanel(ObjectSubclass<imp::RunningPanel>)
        @extends gtk4::Box, gtk4::Widget,
        @implements gio::ActionGroup, gio::ActionMap, gtk4::Accessible, gtk4::Buildable,
                    gtk4::ConstraintTarget, gtk4::Orientable;
}

impl RunningPanel {
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    /// Set the launcher executable path
    pub fn set_launcher(&self, path: PathBuf) {
        self.imp().set_launcher(path);
    }

    /// Get the current launcher path
    pub fn launcher_path(&self) -> Option<PathBuf> {
        self.imp().launcher_path.borrow().clone()
    }

    /// Check if a process is currently running
    pub fn is_running(&self) -> bool {
        self.imp().child_process.borrow().is_some()
    }
}

impl Default for RunningPanel {
    fn default() -> Self {
        Self::new()
    }
}

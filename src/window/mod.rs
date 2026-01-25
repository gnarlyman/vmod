mod imp;

use gtk4::prelude::*;
use gtk4::subclass::prelude::*;
use gtk4::{gio, glib, Application, Box};

glib::wrapper! {
    pub struct VmodWindow(ObjectSubclass<imp::VmodWindow>)
        @extends gtk4::ApplicationWindow, gtk4::Window, gtk4::Widget,
        @implements gio::ActionGroup, gio::ActionMap, gtk4::Accessible, gtk4::Buildable,
                    gtk4::ConstraintTarget, gtk4::Native, gtk4::Root, gtk4::ShortcutManager;
}

impl VmodWindow {
    pub fn new(app: &Application) -> Self {
        glib::Object::builder().property("application", app).build()
    }

    pub fn content_box(&self) -> Box {
        self.imp().content_box.clone()
    }

    pub fn load_window_state(&self) {
        let settings = self.imp().settings.borrow();
        let settings = settings.as_ref().expect("Settings not initialized");

        let width = settings.int("window-width");
        let height = settings.int("window-height");
        let is_maximized = settings.boolean("window-maximized");

        self.set_default_size(width, height);

        if is_maximized {
            self.maximize();
        }
    }

    pub fn save_window_state(&self) {
        let settings = self.imp().settings.borrow();
        let settings = settings.as_ref().expect("Settings not initialized");

        let size = self.default_size();
        settings.set_int("window-width", size.0).ok();
        settings.set_int("window-height", size.1).ok();
        settings.set_boolean("window-maximized", self.is_maximized()).ok();
    }
}

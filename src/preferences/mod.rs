mod imp;

use gtk4::prelude::*;
use gtk4::glib;

glib::wrapper! {
    pub struct PreferencesDialog(ObjectSubclass<imp::PreferencesDialog>)
        @extends gtk4::Window, gtk4::Widget,
        @implements gtk4::Accessible, gtk4::Buildable, gtk4::ConstraintTarget,
                    gtk4::Native, gtk4::Root, gtk4::ShortcutManager;
}

impl PreferencesDialog {
    pub fn new(parent: &gtk4::Window) -> Self {
        let dialog: Self = glib::Object::builder().build();
        dialog.set_transient_for(Some(parent));
        dialog
    }
}

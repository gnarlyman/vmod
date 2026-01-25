use gtk4::prelude::*;
use gtk4::subclass::prelude::*;
use gtk4::{glib, gio, CompositeTemplate, PopoverMenuBar, Box, DropDown};
use std::cell::RefCell;

use crate::mod_list::ModListView;

#[derive(Debug, Default, CompositeTemplate)]
#[template(resource = "/org/vmod/VMOD/window.ui")]
pub struct VmodWindow {
    #[template_child]
    pub menu_bar: TemplateChild<PopoverMenuBar>,
    #[template_child]
    pub content_box: TemplateChild<Box>,

    pub settings: RefCell<Option<gio::Settings>>,
    pub profile_dropdown: RefCell<Option<DropDown>>,
    pub mod_list_view: RefCell<Option<ModListView>>,
}

#[glib::object_subclass]
impl ObjectSubclass for VmodWindow {
    const NAME: &'static str = "VmodWindow";
    type Type = super::VmodWindow;
    type ParentType = gtk4::ApplicationWindow;

    fn class_init(klass: &mut Self::Class) {
        klass.bind_template();
    }

    fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
        obj.init_template();
    }
}

impl ObjectImpl for VmodWindow {
    fn constructed(&self) {
        self.parent_constructed();

        let obj = self.obj();

        // Initialize settings
        let settings = gio::Settings::new(crate::config::APP_ID);
        self.settings.replace(Some(settings));

        // Load window state
        obj.load_window_state();

        // Save window state on close
        obj.connect_close_request(|window| {
            window.save_window_state();
            glib::Propagation::Proceed
        });
    }
}

impl WidgetImpl for VmodWindow {}
impl WindowImpl for VmodWindow {}
impl ApplicationWindowImpl for VmodWindow {}

impl VmodWindow {
    pub fn content_box(&self) -> &Box {
        &self.content_box
    }
}

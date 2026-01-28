use glib::Object;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::subclass::prelude::*;
use std::cell::RefCell;

mod imp {
    use super::*;
    use glib::Properties;
    use std::cell::Cell;

    #[derive(Properties, Default)]
    #[properties(wrapper_type = super::DfmodEntry)]
    pub struct DfmodEntry {
        #[property(get, set)]
        pub file_name: RefCell<String>,
        #[property(get, set)]
        pub title: RefCell<String>,
        #[property(get, set)]
        pub enabled: Cell<bool>,
        #[property(get, set)]
        pub load_priority: Cell<u32>,
        #[property(get, set)]
        pub highlighted: Cell<bool>,
        /// Sorting status: 0=Neutral (no rules), 1=Correct (green), 2=Wrong (red)
        #[property(get, set)]
        pub sorting_status: Cell<u8>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for DfmodEntry {
        const NAME: &'static str = "DfmodEntry";
        type Type = super::DfmodEntry;
    }

    #[glib::derived_properties]
    impl ObjectImpl for DfmodEntry {}
}

glib::wrapper! {
    pub struct DfmodEntry(ObjectSubclass<imp::DfmodEntry>);
}

impl DfmodEntry {
    pub fn new(file_name: String, title: String, enabled: bool, load_priority: u32) -> Self {
        Object::builder()
            .property("file-name", &file_name)
            .property("title", &title)
            .property("enabled", enabled)
            .property("load-priority", load_priority)
            .build()
    }
}

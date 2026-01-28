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
    #[properties(wrapper_type = super::TreeItem)]
    pub struct TreeItem {
        #[property(get, set)]
        pub display_name: RefCell<String>,
        #[property(get, set)]
        pub full_path: RefCell<String>,
        #[property(get, set)]
        pub is_expandable: Cell<bool>,
        #[property(get, set)]
        pub item_type: Cell<u32>, // 0=mod_root, 1=folder, 2=file, 3=dfmod
        #[property(get, set)]
        pub conflict_count: Cell<u32>,
        /// Whether this item directly matches the current filter
        #[property(get, set)]
        pub matches_filter: Cell<bool>,
        /// Whether this item is visible in filter results (self or descendant matches)
        #[property(get, set)]
        pub visible_in_filter: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for TreeItem {
        const NAME: &'static str = "TreeItem";
        type Type = super::TreeItem;
    }

    #[glib::derived_properties]
    impl ObjectImpl for TreeItem {}
}

glib::wrapper! {
    pub struct TreeItem(ObjectSubclass<imp::TreeItem>);
}

impl TreeItem {
    /// Create a new tree item
    pub fn new(display_name: &str, full_path: &str, is_expandable: bool, item_type: u32) -> Self {
        Object::builder()
            .property("display-name", display_name)
            .property("full-path", full_path)
            .property("is-expandable", is_expandable)
            .property("item-type", item_type)
            .property("conflict-count", 0u32)
            .property("matches-filter", false)
            .property("visible-in-filter", true)
            .build()
    }

    /// Create a mod root item (expandable, shows mod name)
    pub fn new_mod_root(mod_name: &str, mod_path: &str, conflict_count: u32) -> Self {
        Object::builder()
            .property("display-name", mod_name)
            .property("full-path", mod_path)
            .property("is-expandable", true)
            .property("item-type", 0u32)
            .property("conflict-count", conflict_count)
            .property("matches-filter", false)
            .property("visible-in-filter", true)
            .build()
    }

    /// Create a folder item (expandable)
    pub fn new_folder(name: &str, path: &str) -> Self {
        Object::builder()
            .property("display-name", name)
            .property("full-path", path)
            .property("is-expandable", true)
            .property("item-type", 1u32)
            .property("conflict-count", 0u32)
            .property("matches-filter", false)
            .property("visible-in-filter", true)
            .build()
    }

    /// Create a file item (not expandable)
    pub fn new_file(name: &str, path: &str) -> Self {
        Object::builder()
            .property("display-name", name)
            .property("full-path", path)
            .property("is-expandable", false)
            .property("item-type", 2u32)
            .property("conflict-count", 0u32)
            .property("matches-filter", false)
            .property("visible-in-filter", true)
            .build()
    }

    /// Create a dfmod archive item (expandable, shows contained assets)
    /// item_type 3 = dfmod archive
    pub fn new_dfmod(name: &str, path: &str, asset_count: u32) -> Self {
        Object::builder()
            .property("display-name", name)
            .property("full-path", path)
            .property("is-expandable", true)
            .property("item-type", 3u32)
            .property("conflict-count", asset_count)
            .property("matches-filter", false)
            .property("visible-in-filter", true)
            .build()
    }
}

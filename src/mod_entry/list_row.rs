use glib::Object;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::subclass::prelude::*;
use std::cell::{Cell, RefCell};

use super::{ModEntry, SectionHeader};

mod imp {
    use super::*;
    use glib::Properties;

    #[derive(Properties, Default)]
    #[properties(wrapper_type = super::ListRow)]
    pub struct ListRow {
        /// True if this row is a section header, false if it's a mod entry
        #[property(get, set)]
        pub is_section: Cell<bool>,
        /// Display name (mod name or section name)
        #[property(get, set)]
        pub display_name: RefCell<String>,
        /// Order position in the list
        #[property(get, set)]
        pub order: Cell<u32>,
        /// For sections: whether expanded
        #[property(get, set)]
        pub expanded: Cell<bool>,
        /// For sections: unique section ID
        #[property(get, set)]
        pub section_id: RefCell<Option<String>>,
        /// For mods: enabled state
        #[property(get, set)]
        pub enabled: Cell<bool>,
        /// For mods: version string
        #[property(get, set)]
        pub version: RefCell<String>,
        /// For mods: nexus ID
        #[property(get, set)]
        pub nexus_id: RefCell<Option<String>>,
        /// For mods: conflict count
        #[property(get, set)]
        pub conflict_count: Cell<u32>,
        /// For mods: which section this belongs to (None = ungrouped)
        #[property(get, set)]
        pub parent_section_id: RefCell<Option<String>>,

        // Internal references
        pub mod_entry: RefCell<Option<ModEntry>>,
        pub section_header: RefCell<Option<SectionHeader>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ListRow {
        const NAME: &'static str = "ListRow";
        type Type = super::ListRow;
    }

    #[glib::derived_properties]
    impl ObjectImpl for ListRow {}
}

glib::wrapper! {
    pub struct ListRow(ObjectSubclass<imp::ListRow>);
}

impl ListRow {
    /// Create a ListRow from a ModEntry
    pub fn from_mod(mod_entry: &ModEntry) -> Self {
        let obj: Self = Object::builder()
            .property("is-section", false)
            .property("display-name", mod_entry.display_name())
            .property("order", mod_entry.order())
            .property("expanded", true)
            .property("enabled", mod_entry.enabled())
            .property("version", mod_entry.version())
            .property("nexus-id", mod_entry.nexus_id())
            .property("conflict-count", mod_entry.conflict_count())
            .property("section-id", None::<String>)
            .property("parent-section-id", mod_entry.section_id())
            .build();

        obj.imp().mod_entry.replace(Some(mod_entry.clone()));

        // Bind properties bidirectionally for live updates
        mod_entry.bind_property("display-name", &obj, "display-name")
            .bidirectional()
            .sync_create()
            .build();
        mod_entry.bind_property("enabled", &obj, "enabled")
            .bidirectional()
            .sync_create()
            .build();
        mod_entry.bind_property("order", &obj, "order")
            .bidirectional()
            .sync_create()
            .build();
        mod_entry.bind_property("conflict-count", &obj, "conflict-count")
            .bidirectional()
            .sync_create()
            .build();
        mod_entry.bind_property("section-id", &obj, "parent-section-id")
            .bidirectional()
            .sync_create()
            .build();

        obj
    }

    /// Create a ListRow from a SectionHeader
    pub fn from_section(section: &SectionHeader) -> Self {
        let obj: Self = Object::builder()
            .property("is-section", true)
            .property("display-name", section.name())
            .property("order", section.order())
            .property("expanded", section.expanded())
            .property("section-id", Some(section.section_id()))
            .property("enabled", false)
            .property("version", "")
            .property("nexus-id", None::<String>)
            .property("conflict-count", 0u32)
            .property("parent-section-id", None::<String>)
            .build();

        obj.imp().section_header.replace(Some(section.clone()));

        // Bind properties bidirectionally
        section.bind_property("name", &obj, "display-name")
            .bidirectional()
            .sync_create()
            .build();
        section.bind_property("expanded", &obj, "expanded")
            .bidirectional()
            .sync_create()
            .build();
        section.bind_property("order", &obj, "order")
            .bidirectional()
            .sync_create()
            .build();

        obj
    }

    /// Get the underlying ModEntry if this is a mod row
    pub fn mod_entry(&self) -> Option<ModEntry> {
        self.imp().mod_entry.borrow().clone()
    }

    /// Get the underlying SectionHeader if this is a section row
    pub fn section_header(&self) -> Option<SectionHeader> {
        self.imp().section_header.borrow().clone()
    }
}

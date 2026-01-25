use gtk4::prelude::*;
use gtk4::subclass::prelude::*;
use gtk4::{
    glib, gio, Box, Button, ColumnView, ColumnViewColumn, Label, Orientation, ScrolledWindow,
    SignalListItemFactory, SingleSelection, CheckButton,
};
use std::cell::RefCell;
use std::path::PathBuf;

use crate::mod_entry::{DfmodEntry, load_mods_json, save_mods_json};

pub struct ModsJsonView {
    pub column_view: RefCell<Option<ColumnView>>,
    pub model: RefCell<Option<gio::ListStore>>,
    pub mods_json_path: RefCell<Option<PathBuf>>,
}

impl Default for ModsJsonView {
    fn default() -> Self {
        Self {
            column_view: RefCell::new(None),
            model: RefCell::new(None),
            mods_json_path: RefCell::new(None),
        }
    }
}

#[glib::object_subclass]
impl ObjectSubclass for ModsJsonView {
    const NAME: &'static str = "ModsJsonView";
    type Type = super::ModsJsonView;
    type ParentType = Box;
}

impl ObjectImpl for ModsJsonView {
    fn constructed(&self) {
        self.parent_constructed();

        let obj = self.obj();
        obj.set_orientation(Orientation::Vertical);
        obj.set_spacing(6);
        obj.set_margin_top(12);
        obj.set_margin_bottom(12);
        obj.set_margin_start(6);
        obj.set_margin_end(12);

        // Header label
        let header_label = Label::new(Some("Mods.json Entries"));
        header_label.add_css_class("heading");
        header_label.set_xalign(0.0);
        obj.append(&header_label);

        // Create the ListStore to hold DfmodEntry objects
        let model = gio::ListStore::new::<DfmodEntry>();
        self.model.replace(Some(model.clone()));

        // Create selection model
        let selection_model = SingleSelection::new(Some(model.clone()));
        selection_model.set_autoselect(false);
        selection_model.set_can_unselect(true);

        // Create ColumnView
        let column_view = ColumnView::new(Some(selection_model));
        column_view.set_show_row_separators(true);
        column_view.set_show_column_separators(true);

        // Add columns
        self.add_checkbox_column(&column_view);
        self.add_title_column(&column_view);
        self.add_filename_column(&column_view);
        self.add_priority_column(&column_view);
        self.add_actions_column(&column_view);

        // Wrap in scrolled window
        let scrolled_window = ScrolledWindow::new();
        scrolled_window.set_vexpand(true);
        scrolled_window.set_hexpand(true);
        scrolled_window.set_child(Some(&column_view));

        obj.append(&scrolled_window);

        self.column_view.replace(Some(column_view));
    }
}

impl WidgetImpl for ModsJsonView {}
impl BoxImpl for ModsJsonView {}

impl ModsJsonView {
    fn add_checkbox_column(&self, column_view: &ColumnView) {
        let factory = SignalListItemFactory::new();

        factory.connect_setup(move |_factory, item| {
            let list_item = item.downcast_ref::<gtk4::ListItem>()
                .expect("Item must be ListItem");
            let check_button = CheckButton::new();
            list_item.set_child(Some(&check_button));
        });

        factory.connect_bind(move |_factory, item| {
            let list_item = item.downcast_ref::<gtk4::ListItem>()
                .expect("Item must be ListItem");

            let dfmod_entry = list_item
                .item()
                .and_downcast::<DfmodEntry>()
                .expect("Item must be DfmodEntry");

            let check_button = list_item
                .child()
                .and_downcast::<CheckButton>()
                .expect("Child must be CheckButton");

            dfmod_entry
                .bind_property("enabled", &check_button, "active")
                .bidirectional()
                .sync_create()
                .build();
        });

        let column = ColumnViewColumn::new(Some("Enabled"), Some(factory));
        column.set_fixed_width(80);
        column_view.append_column(&column);
    }

    fn add_title_column(&self, column_view: &ColumnView) {
        let factory = SignalListItemFactory::new();

        factory.connect_setup(move |_factory, item| {
            let list_item = item.downcast_ref::<gtk4::ListItem>()
                .expect("Item must be ListItem");
            let label = Label::new(None);
            label.set_xalign(0.0);
            list_item.set_child(Some(&label));
        });

        factory.connect_bind(move |_factory, item| {
            let list_item = item.downcast_ref::<gtk4::ListItem>()
                .expect("Item must be ListItem");

            let dfmod_entry = list_item
                .item()
                .and_downcast::<DfmodEntry>()
                .expect("Item must be DfmodEntry");

            let label = list_item
                .child()
                .and_downcast::<Label>()
                .expect("Child must be Label");

            dfmod_entry
                .bind_property("title", &label, "label")
                .sync_create()
                .build();
        });

        let column = ColumnViewColumn::new(Some("Title"), Some(factory));
        column.set_expand(true);
        column_view.append_column(&column);
    }

    fn add_filename_column(&self, column_view: &ColumnView) {
        let factory = SignalListItemFactory::new();

        factory.connect_setup(move |_factory, item| {
            let list_item = item.downcast_ref::<gtk4::ListItem>()
                .expect("Item must be ListItem");
            let label = Label::new(None);
            label.set_xalign(0.0);
            list_item.set_child(Some(&label));
        });

        factory.connect_bind(move |_factory, item| {
            let list_item = item.downcast_ref::<gtk4::ListItem>()
                .expect("Item must be ListItem");

            let dfmod_entry = list_item
                .item()
                .and_downcast::<DfmodEntry>()
                .expect("Item must be DfmodEntry");

            let label = list_item
                .child()
                .and_downcast::<Label>()
                .expect("Child must be Label");

            dfmod_entry
                .bind_property("file-name", &label, "label")
                .sync_create()
                .build();
        });

        let column = ColumnViewColumn::new(Some("FileName"), Some(factory));
        column.set_fixed_width(200);
        column_view.append_column(&column);
    }

    fn add_priority_column(&self, column_view: &ColumnView) {
        let factory = SignalListItemFactory::new();

        factory.connect_setup(move |_factory, item| {
            let list_item = item.downcast_ref::<gtk4::ListItem>()
                .expect("Item must be ListItem");
            let label = Label::new(None);
            label.set_xalign(0.5);
            list_item.set_child(Some(&label));
        });

        factory.connect_bind(move |_factory, item| {
            let list_item = item.downcast_ref::<gtk4::ListItem>()
                .expect("Item must be ListItem");

            let dfmod_entry = list_item
                .item()
                .and_downcast::<DfmodEntry>()
                .expect("Item must be DfmodEntry");

            let label = list_item
                .child()
                .and_downcast::<Label>()
                .expect("Child must be Label");

            label.set_text(&dfmod_entry.load_priority().to_string());

            let label_clone = label.clone();
            dfmod_entry.connect_notify_local(
                Some("load-priority"),
                move |entry, _| {
                    label_clone.set_text(&entry.load_priority().to_string());
                },
            );
        });

        let column = ColumnViewColumn::new(Some("Priority"), Some(factory));
        column.set_fixed_width(80);
        column_view.append_column(&column);
    }

    fn add_actions_column(&self, column_view: &ColumnView) {
        let factory = SignalListItemFactory::new();

        factory.connect_setup(move |_factory, item| {
            let list_item = item.downcast_ref::<gtk4::ListItem>()
                .expect("Item must be ListItem");
            let button_box = Box::new(Orientation::Horizontal, 2);

            let up_button = Button::with_label("↑");
            let down_button = Button::with_label("↓");

            button_box.append(&up_button);
            button_box.append(&down_button);
            list_item.set_child(Some(&button_box));
        });

        let model_ref = self.model.clone();
        factory.connect_bind(move |_factory, item| {
            let list_item = item.downcast_ref::<gtk4::ListItem>()
                .expect("Item must be ListItem");

            let dfmod_entry = list_item
                .item()
                .and_downcast::<DfmodEntry>()
                .expect("Item must be DfmodEntry");

            let button_box = list_item
                .child()
                .and_downcast::<Box>()
                .expect("Child must be Box");

            let up_button = button_box
                .first_child()
                .and_downcast::<Button>()
                .expect("First child must be Button");

            let down_button = button_box
                .last_child()
                .and_downcast::<Button>()
                .expect("Last child must be Button");

            let model_clone = model_ref.clone();
            let entry_clone = dfmod_entry.clone();
            up_button.connect_clicked(move |_| {
                Self::move_entry_up_static(&model_clone, &entry_clone);
            });

            let model_clone = model_ref.clone();
            let entry_clone = dfmod_entry.clone();
            down_button.connect_clicked(move |_| {
                Self::move_entry_down_static(&model_clone, &entry_clone);
            });
        });

        let column = ColumnViewColumn::new(Some("Actions"), Some(factory));
        column.set_fixed_width(100);
        column_view.append_column(&column);
    }

    pub fn load_mods_json_static(&self, mods_json_path: &std::path::Path) {
        self.mods_json_path.replace(Some(mods_json_path.to_path_buf()));

        let model = self.model.borrow();
        if let Some(model) = model.as_ref() {
            model.remove_all();

            match load_mods_json(mods_json_path) {
                Ok(mut entries) => {
                    // Sort by load_priority so it always displays 0, 1, 2, 3...
                    entries.sort_by_key(|e| e.load_priority);

                    for entry in entries {
                        let dfmod_entry = DfmodEntry::new(
                            entry.file_name,
                            entry.title,
                            entry.enabled,
                            entry.load_priority,
                        );
                        model.append(&dfmod_entry);
                    }
                }
                Err(e) => {
                    eprintln!("Failed to load Mods.json: {}", e);
                }
            }
        }
    }

    pub fn save_mods_json_static(&self) -> Result<(), String> {
        let model = self.model.borrow();
        let mods_json_path = self.mods_json_path.borrow();

        let model = model.as_ref().ok_or("Model not initialized")?;
        let mods_json_path = mods_json_path.as_ref().ok_or("Mods.json path not set")?;

        let mut entries = Vec::new();
        for i in 0..model.n_items() {
            if let Some(obj) = model.item(i) {
                if let Ok(dfmod_entry) = obj.downcast::<DfmodEntry>() {
                    entries.push(crate::mod_entry::ModsJsonEntry {
                        file_name: dfmod_entry.file_name(),
                        title: dfmod_entry.title(),
                        enabled: dfmod_entry.enabled(),
                        load_priority: dfmod_entry.load_priority(),
                    });
                }
            }
        }

        save_mods_json(mods_json_path, &entries)
    }

    fn move_entry_up_static(model: &RefCell<Option<gio::ListStore>>, entry: &DfmodEntry) {
        let model_borrow = model.borrow();
        if let Some(model_store) = model_borrow.as_ref() {
            let current_priority = entry.load_priority();
            if current_priority == 0 {
                return;
            }

            // Swap priorities
            for i in 0..model_store.n_items() {
                if let Some(obj) = model_store.item(i) {
                    if let Ok(other_entry) = obj.downcast::<DfmodEntry>() {
                        if other_entry.load_priority() == current_priority - 1 {
                            entry.set_load_priority(current_priority - 1);
                            other_entry.set_load_priority(current_priority);
                            break;
                        }
                    }
                }
            }

            // Re-sort the list by priority
            let mut entries: Vec<DfmodEntry> = Vec::new();
            for i in 0..model_store.n_items() {
                if let Some(obj) = model_store.item(i) {
                    if let Ok(dfmod_entry) = obj.downcast::<DfmodEntry>() {
                        entries.push(dfmod_entry);
                    }
                }
            }
            entries.sort_by_key(|e| e.load_priority());

            // Clear and re-populate in sorted order
            model_store.remove_all();
            for dfmod_entry in entries {
                model_store.append(&dfmod_entry);
            }
        }
    }

    fn move_entry_down_static(model: &RefCell<Option<gio::ListStore>>, entry: &DfmodEntry) {
        let model_borrow = model.borrow();
        if let Some(model_store) = model_borrow.as_ref() {
            let current_priority = entry.load_priority();
            let max_priority = model_store.n_items() - 1;

            if current_priority >= max_priority {
                return;
            }

            // Swap priorities
            for i in 0..model_store.n_items() {
                if let Some(obj) = model_store.item(i) {
                    if let Ok(other_entry) = obj.downcast::<DfmodEntry>() {
                        if other_entry.load_priority() == current_priority + 1 {
                            entry.set_load_priority(current_priority + 1);
                            other_entry.set_load_priority(current_priority);
                            break;
                        }
                    }
                }
            }

            // Re-sort the list by priority
            let mut entries: Vec<DfmodEntry> = Vec::new();
            for i in 0..model_store.n_items() {
                if let Some(obj) = model_store.item(i) {
                    if let Ok(dfmod_entry) = obj.downcast::<DfmodEntry>() {
                        entries.push(dfmod_entry);
                    }
                }
            }
            entries.sort_by_key(|e| e.load_priority());

            // Clear and re-populate in sorted order
            model_store.remove_all();
            for dfmod_entry in entries {
                model_store.append(&dfmod_entry);
            }
        }
    }
}

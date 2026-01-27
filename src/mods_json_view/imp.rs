use gtk4::prelude::*;
use gtk4::subclass::prelude::*;
use gtk4::{
    gio, glib, Box, Button, CheckButton, ColumnView, ColumnViewColumn, Label, Orientation,
    PolicyType, ScrolledWindow, SignalListItemFactory, SingleSelection,
};
use std::cell::RefCell;
use std::path::PathBuf;

use crate::mod_entry::{load_mods_json, save_mods_json, DfmodEntry, SortingRules};

pub struct ModsJsonView {
    pub column_view: RefCell<Option<ColumnView>>,
    pub model: RefCell<Option<gio::ListStore>>,
    pub selection_model: RefCell<Option<SingleSelection>>,
    pub mods_json_path: RefCell<Option<PathBuf>>,
    pub settings: RefCell<Option<gio::Settings>>,
}

impl Default for ModsJsonView {
    fn default() -> Self {
        Self {
            column_view: RefCell::new(None),
            model: RefCell::new(None),
            selection_model: RefCell::new(None),
            mods_json_path: RefCell::new(None),
            settings: RefCell::new(None),
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

        // Initialize settings
        let settings = gio::Settings::new(crate::config::APP_ID);
        self.settings.replace(Some(settings.clone()));

        // Header label
        let header_label = Label::new(Some("Mods.json Entries"));
        header_label.add_css_class("heading");
        header_label.set_xalign(0.0);
        obj.append(&header_label);

        // Sort button row
        let sort_row = Box::new(Orientation::Horizontal, 6);
        let sort_button = Button::with_label("Sort Now");
        sort_button.set_tooltip_text(Some("Apply sorting rules from sorting_rules.json"));
        sort_row.append(&sort_button);
        obj.append(&sort_row);

        // Create the ListStore to hold DfmodEntry objects
        let model = gio::ListStore::new::<DfmodEntry>();
        self.model.replace(Some(model.clone()));

        // Create selection model
        let selection_model = SingleSelection::new(Some(model.clone()));
        selection_model.set_autoselect(false);
        selection_model.set_can_unselect(true);
        self.selection_model.replace(Some(selection_model.clone()));

        // Create ColumnView
        let column_view = ColumnView::new(Some(selection_model.clone()));
        column_view.set_show_row_separators(true);
        column_view.set_show_column_separators(true);

        // Add columns
        self.add_checkbox_column(&column_view, &settings);
        self.add_title_column(&column_view, &settings);
        self.add_filename_column(&column_view, &settings);
        self.add_priority_column(&column_view, &settings);

        // Wrap in scrolled window
        let scrolled_window = ScrolledWindow::new();
        scrolled_window.set_vexpand(true);
        scrolled_window.set_hexpand(true);
        scrolled_window.set_policy(PolicyType::Never, PolicyType::Automatic); // No horizontal scroll
        scrolled_window.set_child(Some(&column_view));

        obj.append(&scrolled_window);

        // Add reorder buttons below the list
        let button_box = Box::new(Orientation::Horizontal, 6);
        button_box.set_halign(gtk4::Align::Center);
        button_box.set_margin_top(6);
        button_box.set_margin_bottom(6);

        let top_button = Button::with_label("⇈ Top");
        let up_button = Button::with_label("↑ Up");
        let down_button = Button::with_label("↓ Down");
        let bottom_button = Button::with_label("⇊ Bottom");

        // Add separator
        let separator = gtk4::Separator::new(Orientation::Vertical);
        separator.set_margin_start(6);
        separator.set_margin_end(6);

        let enable_all_button = Button::with_label("Enable All");
        let disable_all_button = Button::with_label("Disable All");

        button_box.append(&top_button);
        button_box.append(&up_button);
        button_box.append(&down_button);
        button_box.append(&bottom_button);
        button_box.append(&separator);
        button_box.append(&enable_all_button);
        button_box.append(&disable_all_button);

        obj.append(&button_box);

        // Store model reference for button callbacks
        let model_ref = self.model.clone();

        // Connect buttons to move selected entry
        let selection_model_clone = column_view.model().unwrap();
        let selection = selection_model_clone.downcast::<SingleSelection>().unwrap();

        let selection_clone = selection.clone();
        let model_clone = model_ref.clone();
        top_button.connect_clicked(move |_| {
            if let Some(item) = selection_clone.selected_item() {
                if let Ok(dfmod_entry) = item.downcast::<DfmodEntry>() {
                    Self::move_entry_to_top_static(&model_clone, &dfmod_entry, &selection_clone);
                }
            }
        });

        let selection_clone = selection.clone();
        let model_clone = model_ref.clone();
        up_button.connect_clicked(move |_| {
            if let Some(item) = selection_clone.selected_item() {
                if let Ok(dfmod_entry) = item.downcast::<DfmodEntry>() {
                    Self::move_entry_up_static(&model_clone, &dfmod_entry, &selection_clone);
                }
            }
        });

        let selection_clone = selection.clone();
        let model_clone = model_ref.clone();
        down_button.connect_clicked(move |_| {
            if let Some(item) = selection_clone.selected_item() {
                if let Ok(dfmod_entry) = item.downcast::<DfmodEntry>() {
                    Self::move_entry_down_static(&model_clone, &dfmod_entry, &selection_clone);
                }
            }
        });

        let selection_clone = selection.clone();
        let model_clone = model_ref.clone();
        bottom_button.connect_clicked(move |_| {
            if let Some(item) = selection_clone.selected_item() {
                if let Ok(dfmod_entry) = item.downcast::<DfmodEntry>() {
                    Self::move_entry_to_bottom_static(&model_clone, &dfmod_entry, &selection_clone);
                }
            }
        });

        // Connect enable/disable all buttons
        let model_clone = model_ref.clone();
        enable_all_button.connect_clicked(move |_| {
            Self::enable_all_entries_static(&model_clone);
        });

        let model_clone = model_ref.clone();
        disable_all_button.connect_clicked(move |_| {
            Self::disable_all_entries_static(&model_clone);
        });

        // Connect sort button
        let obj_for_sort = obj.clone();
        sort_button.connect_clicked(move |_| {
            obj_for_sort.imp().apply_sorting_rules();
        });

        self.column_view.replace(Some(column_view));
    }
}

impl WidgetImpl for ModsJsonView {}
impl BoxImpl for ModsJsonView {}

impl ModsJsonView {
    fn add_checkbox_column(&self, column_view: &ColumnView, settings: &gio::Settings) {
        let factory = SignalListItemFactory::new();

        factory.connect_setup(move |_factory, item| {
            let list_item = item
                .downcast_ref::<gtk4::ListItem>()
                .expect("Item must be ListItem");
            let check_button = CheckButton::new();
            list_item.set_child(Some(&check_button));
        });

        factory.connect_bind(move |_factory, item| {
            let list_item = item
                .downcast_ref::<gtk4::ListItem>()
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

        let column = ColumnViewColumn::new(Some("On"), Some(factory));
        column.set_resizable(false);
        column.set_fixed_width(35);
        column_view.append_column(&column);
    }

    fn add_title_column(&self, column_view: &ColumnView, _settings: &gio::Settings) {
        let factory = SignalListItemFactory::new();

        factory.connect_setup(move |_factory, item| {
            let list_item = item
                .downcast_ref::<gtk4::ListItem>()
                .expect("Item must be ListItem");
            let label = Label::new(None);
            label.set_xalign(0.0);
            list_item.set_child(Some(&label));
        });

        factory.connect_bind(move |_factory, item| {
            let list_item = item
                .downcast_ref::<gtk4::ListItem>()
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

            // Apply CSS to row widget when highlighted changes
            let label_clone = label.clone();
            dfmod_entry.connect_notify_local(Some("highlighted"), move |entry, _| {
                let mut widget = label_clone.clone().upcast::<gtk4::Widget>();
                while let Some(parent) = widget.parent() {
                    if parent.css_name() == "row" {
                        if entry.highlighted() {
                            parent.add_css_class("highlighted");
                        } else {
                            parent.remove_css_class("highlighted");
                        }
                        break;
                    }
                    widget = parent;
                }
            });
        });

        let column = ColumnViewColumn::new(Some("Title"), Some(factory));
        column.set_resizable(false);
        column.set_fixed_width(200);
        column_view.append_column(&column);
    }

    fn add_filename_column(&self, column_view: &ColumnView, _settings: &gio::Settings) {
        let factory = SignalListItemFactory::new();

        factory.connect_setup(move |_factory, item| {
            let list_item = item
                .downcast_ref::<gtk4::ListItem>()
                .expect("Item must be ListItem");
            let label = Label::new(None);
            label.set_xalign(0.0);
            list_item.set_child(Some(&label));
        });

        factory.connect_bind(move |_factory, item| {
            let list_item = item
                .downcast_ref::<gtk4::ListItem>()
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

        let column = ColumnViewColumn::new(Some("File"), Some(factory));
        column.set_resizable(false);
        column.set_expand(true);
        column_view.append_column(&column);
    }

    fn add_priority_column(&self, column_view: &ColumnView, _settings: &gio::Settings) {
        let factory = SignalListItemFactory::new();

        factory.connect_setup(move |_factory, item| {
            let list_item = item
                .downcast_ref::<gtk4::ListItem>()
                .expect("Item must be ListItem");
            let label = Label::new(None);
            label.set_xalign(0.5);
            list_item.set_child(Some(&label));
        });

        factory.connect_bind(move |_factory, item| {
            let list_item = item
                .downcast_ref::<gtk4::ListItem>()
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
            dfmod_entry.connect_notify_local(Some("load-priority"), move |entry, _| {
                label_clone.set_text(&entry.load_priority().to_string());
            });
        });

        let column = ColumnViewColumn::new(Some("#"), Some(factory));
        column.set_resizable(false);
        column.set_fixed_width(35);
        column_view.append_column(&column);
    }

    pub fn load_mods_json_static(&self, mods_json_path: &std::path::Path) {
        self.mods_json_path
            .replace(Some(mods_json_path.to_path_buf()));

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

    fn move_entry_up_static(
        model: &RefCell<Option<gio::ListStore>>,
        entry: &DfmodEntry,
        selection: &SingleSelection,
    ) {
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

            // Restore selection at new position (moved up by 1)
            if current_priority > 0 {
                selection.set_selected(current_priority - 1);
            }
        }
    }

    fn move_entry_down_static(
        model: &RefCell<Option<gio::ListStore>>,
        entry: &DfmodEntry,
        selection: &SingleSelection,
    ) {
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

            // Restore selection at new position (moved down by 1)
            if current_priority < max_priority {
                selection.set_selected(current_priority + 1);
            }
        }
    }

    fn move_entry_to_top_static(
        model: &RefCell<Option<gio::ListStore>>,
        entry: &DfmodEntry,
        selection: &SingleSelection,
    ) {
        let model_borrow = model.borrow();
        if let Some(model_store) = model_borrow.as_ref() {
            let current_priority = entry.load_priority();

            if current_priority == 0 {
                return; // Already at top
            }

            // Set this entry's priority to 0
            entry.set_load_priority(0);

            // Shift all entries with priority < current_priority down by 1
            for i in 0..model_store.n_items() {
                if let Some(obj) = model_store.item(i) {
                    if let Ok(other_entry) = obj.downcast::<DfmodEntry>() {
                        if other_entry.load_priority() < current_priority
                            && other_entry.file_name() != entry.file_name()
                        {
                            other_entry.set_load_priority(other_entry.load_priority() + 1);
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

            // Restore selection at position 0 (top)
            selection.set_selected(0);
        }
    }

    fn move_entry_to_bottom_static(
        model: &RefCell<Option<gio::ListStore>>,
        entry: &DfmodEntry,
        selection: &SingleSelection,
    ) {
        let model_borrow = model.borrow();
        if let Some(model_store) = model_borrow.as_ref() {
            let current_priority = entry.load_priority();
            let last_priority = (model_store.n_items() - 1) as u32;

            if current_priority >= last_priority {
                return; // Already at bottom
            }

            // Set this entry's priority to last
            entry.set_load_priority(last_priority);

            // Shift all entries with priority > current_priority up by 1
            for i in 0..model_store.n_items() {
                if let Some(obj) = model_store.item(i) {
                    if let Ok(other_entry) = obj.downcast::<DfmodEntry>() {
                        if other_entry.load_priority() > current_priority
                            && other_entry.file_name() != entry.file_name()
                        {
                            other_entry.set_load_priority(other_entry.load_priority() - 1);
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

            // Restore selection at bottom position
            selection.set_selected(last_priority);
        }
    }

    /// Enable all entries (static version for closures)
    fn enable_all_entries_static(model: &RefCell<Option<gio::ListStore>>) {
        let model_borrow = model.borrow();
        if let Some(model_store) = model_borrow.as_ref() {
            // Enable all entries
            for i in 0..model_store.n_items() {
                if let Some(item) = model_store.item(i) {
                    if let Ok(dfmod_entry) = item.downcast::<DfmodEntry>() {
                        dfmod_entry.set_enabled(true);
                    }
                }
            }
        }
    }

    /// Disable all entries (static version for closures)
    fn disable_all_entries_static(model: &RefCell<Option<gio::ListStore>>) {
        let model_borrow = model.borrow();
        if let Some(model_store) = model_borrow.as_ref() {
            // Disable all entries
            for i in 0..model_store.n_items() {
                if let Some(item) = model_store.item(i) {
                    if let Ok(dfmod_entry) = item.downcast::<DfmodEntry>() {
                        dfmod_entry.set_enabled(false);
                    }
                }
            }
        }
    }

    /// Apply sorting rules from sorting_rules.json
    fn apply_sorting_rules(&self) {
        // Get Mods.json path
        let path = match self.mods_json_path.borrow().as_ref() {
            Some(p) => p.clone(),
            None => {
                eprintln!("Mods.json path not set");
                return;
            }
        };

        // Load sorting rules from ~/.config/vmod/sorting_rules.json
        let config_dir = match dirs::config_dir() {
            Some(dir) => dir.join("vmod"),
            None => {
                eprintln!("Could not find config directory");
                return;
            }
        };
        let rules_path = config_dir.join("sorting_rules.json");

        let rules = match SortingRules::load(&rules_path) {
            Ok(rules) => rules,
            Err(e) => {
                eprintln!("Failed to load sorting rules: {}", e);
                return;
            }
        };

        if rules.rules.is_empty() {
            eprintln!("No sorting rules found in {:?}", rules_path);
            return;
        }

        // Load current Mods.json
        let entries = match load_mods_json(&path) {
            Ok(entries) => entries,
            Err(e) => {
                eprintln!("Failed to load Mods.json: {}", e);
                return;
            }
        };

        // Apply sorting
        let sorted_entries = match rules.apply_sort(&entries) {
            Ok(sorted) => sorted,
            Err(e) => {
                eprintln!("Failed to sort mods: {}", e);
                return;
            }
        };

        // Save sorted Mods.json
        if let Err(e) = save_mods_json(&path, &sorted_entries) {
            eprintln!("Failed to save Mods.json: {}", e);
            return;
        }

        // Reload the model with sorted entries
        if let Some(model_store) = self.model.borrow().as_ref() {
            model_store.remove_all();
            for entry in &sorted_entries {
                let dfmod_entry = DfmodEntry::new(
                    entry.file_name.clone(),
                    entry.title.clone(),
                    entry.enabled,
                    entry.load_priority,
                );
                model_store.append(&dfmod_entry);
            }
        }

        println!(
            "Applied sorting rules: {} mods reordered",
            sorted_entries.len()
        );
    }

    pub fn highlight_entries(&self, file_names: &[String]) {
        if let Some(model) = self.model.borrow().as_ref() {
            for i in 0..model.n_items() {
                if let Some(item) = model.item(i).and_then(|i| i.downcast::<DfmodEntry>().ok()) {
                    item.set_highlighted(file_names.contains(&item.file_name()));
                }
            }
        }
    }

    pub fn clear_highlights(&self) {
        if let Some(model) = self.model.borrow().as_ref() {
            for i in 0..model.n_items() {
                if let Some(item) = model.item(i).and_then(|i| i.downcast::<DfmodEntry>().ok()) {
                    item.set_highlighted(false);
                }
            }
        }
    }
}

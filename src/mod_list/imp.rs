use gtk4::prelude::*;
use gtk4::subclass::prelude::*;
use gtk4::{
    glib, gio, Box, Button, ColumnView, ColumnViewColumn, Label, Orientation, ScrolledWindow,
    SignalListItemFactory, SingleSelection, CheckButton, SearchEntry,
};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use crate::mod_entry::{ModEntry, ModList, ModState, VirtualFileSystem};

pub struct ModListView {
    pub column_view: RefCell<Option<ColumnView>>,
    pub model: RefCell<Option<gio::ListStore>>,
    pub vfs: RefCell<Option<VirtualFileSystem>>,
    pub search_entry: RefCell<Option<SearchEntry>>,
    pub profile_name: Rc<RefCell<Option<String>>>,
}

impl Default for ModListView {
    fn default() -> Self {
        Self {
            column_view: RefCell::new(None),
            model: RefCell::new(None),
            vfs: RefCell::new(None),
            search_entry: RefCell::new(None),
            profile_name: Rc::new(RefCell::new(None)),
        }
    }
}

#[glib::object_subclass]
impl ObjectSubclass for ModListView {
    const NAME: &'static str = "ModListView";
    type Type = super::ModListView;
    type ParentType = Box;
}

impl ObjectImpl for ModListView {
    fn constructed(&self) {
        self.parent_constructed();

        let obj = self.obj();
        obj.set_orientation(Orientation::Vertical);
        obj.set_spacing(6);
        obj.set_margin_top(12);
        obj.set_margin_bottom(12);
        obj.set_margin_start(12);
        obj.set_margin_end(12);

        // Create search entry
        let search_entry = SearchEntry::new();
        search_entry.set_placeholder_text(Some("Filter mods..."));
        obj.append(&search_entry);
        self.search_entry.replace(Some(search_entry.clone()));

        // Create the ListStore to hold ModEntry objects
        let model = gio::ListStore::new::<ModEntry>();
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
        self.add_name_column(&column_view);
        self.add_version_column(&column_view);
        self.add_order_column(&column_view);
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

impl WidgetImpl for ModListView {}
impl BoxImpl for ModListView {}

impl ModListView {
    fn add_checkbox_column(&self, column_view: &ColumnView) {
        let factory = SignalListItemFactory::new();

        // Setup: Create the CheckButton widget
        factory.connect_setup(move |_factory, item| {
            let list_item = item.downcast_ref::<gtk4::ListItem>()
                .expect("Item must be ListItem");
            let check_button = CheckButton::new();
            list_item.set_child(Some(&check_button));
        });

        // Bind: Connect the CheckButton to the ModEntry's enabled property
        let model_ref = self.model.clone();
        let vfs_ref = self.vfs.clone();
        let profile_name_ref = self.profile_name.clone();
        factory.connect_bind(move |_factory, item| {
            let list_item = item.downcast_ref::<gtk4::ListItem>()
                .expect("Item must be ListItem");

            let mod_entry = list_item
                .item()
                .and_downcast::<ModEntry>()
                .expect("Item must be ModEntry");

            let check_button = list_item
                .child()
                .and_downcast::<CheckButton>()
                .expect("Child must be CheckButton");

            // Bind the enabled property
            mod_entry
                .bind_property("enabled", &check_button, "active")
                .bidirectional()
                .sync_create()
                .build();

            // Connect to toggled signal to rebuild VFS and save state
            let model_clone = model_ref.clone();
            let vfs_clone = vfs_ref.clone();
            let profile_name_clone = profile_name_ref.clone();
            check_button.connect_toggled(move |_btn| {
                // Rebuild all symlinks when any mod's enabled state changes
                Self::rebuild_vfs_static(&model_clone, &vfs_clone);

                // Save mod state
                Self::save_mod_state_static(&model_clone, &profile_name_clone);
            });
        });

        let column = ColumnViewColumn::new(Some("Enabled"), Some(factory));
        column.set_fixed_width(80);
        column_view.append_column(&column);
    }

    fn add_name_column(&self, column_view: &ColumnView) {
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

            let mod_entry = list_item
                .item()
                .and_downcast::<ModEntry>()
                .expect("Item must be ModEntry");

            let label = list_item
                .child()
                .and_downcast::<Label>()
                .expect("Child must be Label");

            mod_entry
                .bind_property("name", &label, "label")
                .sync_create()
                .build();
        });

        let column = ColumnViewColumn::new(Some("Mod Name"), Some(factory));
        column.set_expand(true);
        column_view.append_column(&column);
    }

    fn add_version_column(&self, column_view: &ColumnView) {
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

            let mod_entry = list_item
                .item()
                .and_downcast::<ModEntry>()
                .expect("Item must be ModEntry");

            let label = list_item
                .child()
                .and_downcast::<Label>()
                .expect("Child must be Label");

            mod_entry
                .bind_property("version", &label, "label")
                .sync_create()
                .build();
        });

        let column = ColumnViewColumn::new(Some("Version"), Some(factory));
        column.set_fixed_width(100);
        column_view.append_column(&column);
    }

    fn add_order_column(&self, column_view: &ColumnView) {
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

            let mod_entry = list_item
                .item()
                .and_downcast::<ModEntry>()
                .expect("Item must be ModEntry");

            let label = list_item
                .child()
                .and_downcast::<Label>()
                .expect("Child must be Label");

            // Set initial value
            label.set_text(&mod_entry.order().to_string());

            // Update when order property changes
            let label_clone = label.clone();
            mod_entry.connect_notify_local(
                Some("order"),
                move |entry, _| {
                    label_clone.set_text(&entry.order().to_string());
                },
            );
        });

        let column = ColumnViewColumn::new(Some("Order"), Some(factory));
        column.set_fixed_width(80);
        column_view.append_column(&column);
    }

    fn add_actions_column(&self, column_view: &ColumnView) {
        let factory = SignalListItemFactory::new();

        // Setup: Create the button box
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

        // Bind: Connect the buttons to the ModEntry
        let model_ref = self.model.clone();
        let vfs_ref = self.vfs.clone();
        let profile_name_ref = self.profile_name.clone();

        factory.connect_bind(move |_factory, item| {
            let list_item = item.downcast_ref::<gtk4::ListItem>()
                .expect("Item must be ListItem");

            let mod_entry = list_item
                .item()
                .and_downcast::<ModEntry>()
                .expect("Item must be ModEntry");

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

            // Connect up button
            let model_clone = model_ref.clone();
            let vfs_clone = vfs_ref.clone();
            let profile_name_clone = profile_name_ref.clone();
            let mod_entry_clone = mod_entry.clone();

            up_button.connect_clicked(move |_| {
                Self::move_mod_up_static(&model_clone, &mod_entry_clone, &vfs_clone, &profile_name_clone);
            });

            // Connect down button
            let model_clone = model_ref.clone();
            let vfs_clone = vfs_ref.clone();
            let profile_name_clone = profile_name_ref.clone();
            let mod_entry_clone = mod_entry.clone();

            down_button.connect_clicked(move |_| {
                Self::move_mod_down_static(&model_clone, &mod_entry_clone, &vfs_clone, &profile_name_clone);
            });
        });

        let column = ColumnViewColumn::new(Some("Actions"), Some(factory));
        column.set_fixed_width(100);
        column_view.append_column(&column);
    }

    pub fn load_mods(&self, mods_folder: &std::path::Path, game_mods_folder: &std::path::Path, profile_name: &str) {
        // Store profile name
        self.profile_name.replace(Some(profile_name.to_string()));

        // Create VFS manager
        let vfs = VirtualFileSystem::new(game_mods_folder.to_path_buf());
        self.vfs.replace(Some(vfs));

        // Load saved mod state
        let mod_state = match ModState::load(profile_name) {
            Ok(state) => state,
            Err(_) => ModState::default(),
        };

        // Scan mods folder
        let mut mods = ModList::scan_mods_folder(mods_folder);

        // Restore enabled state and order for all mods
        for mod_entry in &mods {
            // Get mod folder name for state lookup
            let mod_folder_name = mod_entry.path()
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();

            // Restore enabled state from saved state
            let is_enabled = mod_state.is_enabled(&mod_folder_name);
            if is_enabled {
                mod_entry.set_enabled(true);
            }

            // Restore order from saved state if available
            if let Some(saved_order) = mod_state.get_order(&mod_folder_name) {
                mod_entry.set_order(saved_order);
            }
        }

        // Sort mods by order before adding to model
        mods.sort_by_key(|m| m.order());

        // Populate the model with sorted mods
        let model = self.model.borrow();
        if let Some(model) = model.as_ref() {
            model.remove_all();
            for mod_entry in mods {
                model.append(&mod_entry);
            }
        }

        // Rebuild VFS with loaded state
        self.rebuild_vfs();
    }

    pub fn refresh_list(&self) {
        // Trigger a re-render by signaling model changed
        if let Some(model) = self.model.borrow().as_ref() {
            model.items_changed(0, model.n_items(), model.n_items());
        }
    }

    pub fn get_vfs(&self) -> Option<VirtualFileSystem> {
        self.vfs.borrow().as_ref().map(|vfs| {
            VirtualFileSystem::new(PathBuf::from(&vfs.game_mods_folder))
        })
    }

    /// Rebuild all symlinks based on enabled mods and their order
    /// Static version for use in closures
    fn rebuild_vfs_static(model: &RefCell<Option<gio::ListStore>>, vfs: &RefCell<Option<VirtualFileSystem>>) {
        let model_borrow = model.borrow();
        let vfs_borrow = vfs.borrow();

        if let (Some(model), Some(vfs)) = (model_borrow.as_ref(), vfs_borrow.as_ref()) {
            // Clear all existing symlinks
            if let Err(e) = vfs.clear_all_symlinks() {
                eprintln!("Failed to clear symlinks: {}", e);
                return;
            }

            // Get all mods sorted by order
            let n_items = model.n_items();
            let mut mods: Vec<ModEntry> = Vec::new();

            for i in 0..n_items {
                if let Some(item) = model.item(i) {
                    if let Ok(mod_entry) = item.downcast::<ModEntry>() {
                        mods.push(mod_entry);
                    }
                }
            }

            // Sort by order (lowest to highest)
            mods.sort_by_key(|m| m.order());

            // Apply enabled mods in order
            for mod_entry in mods {
                if mod_entry.enabled() {
                    let mod_path = mod_entry.path();
                    if let Err(e) = vfs.enable_mod(&mod_path) {
                        eprintln!("Failed to enable mod {}: {}", mod_entry.name(), e);
                    }
                }
            }
        }
    }

    /// Rebuild all symlinks based on enabled mods and their order
    pub fn rebuild_vfs(&self) {
        Self::rebuild_vfs_static(&self.model, &self.vfs);
    }

    /// Save mod enabled state to disk
    /// Static version for use in closures
    fn save_mod_state_static(model: &RefCell<Option<gio::ListStore>>, profile_name: &Rc<RefCell<Option<String>>>) {
        let model_borrow = model.borrow();
        let profile_name_borrow = profile_name.borrow();

        if let (Some(model), Some(profile_name)) = (model_borrow.as_ref(), profile_name_borrow.as_ref()) {
            let mut mod_state = ModState::new();

            // Collect enabled state from all mods
            let n_items = model.n_items();

            for i in 0..n_items {
                if let Some(item) = model.item(i) {
                    if let Ok(mod_entry) = item.downcast::<ModEntry>() {
                        let mod_folder_name = mod_entry.path()
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("")
                            .to_string();

                        let enabled = mod_entry.enabled();
                        let order = mod_entry.order();

                        mod_state.set_enabled(mod_folder_name.clone(), enabled);
                        mod_state.set_order(mod_folder_name, order);
                    }
                }
            }

            // Save to disk
            if let Err(e) = mod_state.save(profile_name) {
                eprintln!("Failed to save mod state: {}", e);
            }
        }
    }

    /// Save mod state
    pub fn save_mod_state(&self) {
        Self::save_mod_state_static(&self.model, &self.profile_name);
    }

    /// Find the position of a mod in the model
    fn find_mod_position(model: &gio::ListStore, target: &ModEntry) -> u32 {
        let target_path = target.path();
        for i in 0..model.n_items() {
            if let Some(item) = model.item(i) {
                if let Ok(entry) = item.downcast::<ModEntry>() {
                    if entry.path() == target_path {
                        return i;
                    }
                }
            }
        }
        0
    }

    /// Move a mod up in the list (static version for closures)
    fn move_mod_up_static(
        model: &RefCell<Option<gio::ListStore>>,
        mod_entry: &ModEntry,
        vfs: &RefCell<Option<VirtualFileSystem>>,
        profile_name: &Rc<RefCell<Option<String>>>
    ) {
        let model_borrow = model.borrow();
        if let Some(model_store) = model_borrow.as_ref() {
            // Find position of this mod in model
            let position = Self::find_mod_position(model_store, mod_entry);

            if position == 0 {
                return; // Already at top
            }

            // Get previous mod
            let prev_mod = model_store.item(position - 1)
                .and_downcast::<ModEntry>()
                .expect("Previous item must be ModEntry");

            // Swap order values
            let temp_order = mod_entry.order();
            mod_entry.set_order(prev_mod.order());
            prev_mod.set_order(temp_order);

            // Collect all items and sort by order
            let n_items = model_store.n_items();
            let mut mods: Vec<ModEntry> = Vec::new();
            for i in 0..n_items {
                if let Some(item) = model_store.item(i) {
                    if let Ok(entry) = item.downcast::<ModEntry>() {
                        mods.push(entry);
                    }
                }
            }
            mods.sort_by_key(|m| m.order());

            // Clear and re-populate model in sorted order
            model_store.remove_all();
            for mod_entry in mods {
                model_store.append(&mod_entry);
            }

            // Drop the borrow before calling static methods
            drop(model_borrow);

            // Rebuild VFS and save
            Self::rebuild_vfs_static(model, vfs);
            Self::save_mod_state_static(model, profile_name);
        }
    }

    /// Move a mod down in the list (static version for closures)
    fn move_mod_down_static(
        model: &RefCell<Option<gio::ListStore>>,
        mod_entry: &ModEntry,
        vfs: &RefCell<Option<VirtualFileSystem>>,
        profile_name: &Rc<RefCell<Option<String>>>
    ) {
        let model_borrow = model.borrow();
        if let Some(model_store) = model_borrow.as_ref() {
            // Find position of this mod in model
            let position = Self::find_mod_position(model_store, mod_entry);

            if position >= model_store.n_items() - 1 {
                return; // Already at bottom
            }

            // Get next mod
            let next_mod = model_store.item(position + 1)
                .and_downcast::<ModEntry>()
                .expect("Next item must be ModEntry");

            // Swap order values
            let temp_order = mod_entry.order();
            mod_entry.set_order(next_mod.order());
            next_mod.set_order(temp_order);

            // Collect all items and sort by order
            let n_items = model_store.n_items();
            let mut mods: Vec<ModEntry> = Vec::new();
            for i in 0..n_items {
                if let Some(item) = model_store.item(i) {
                    if let Ok(entry) = item.downcast::<ModEntry>() {
                        mods.push(entry);
                    }
                }
            }
            mods.sort_by_key(|m| m.order());

            // Clear and re-populate model in sorted order
            model_store.remove_all();
            for mod_entry in mods {
                model_store.append(&mod_entry);
            }

            // Drop the borrow before calling static methods
            drop(model_borrow);

            // Rebuild VFS and save
            Self::rebuild_vfs_static(model, vfs);
            Self::save_mod_state_static(model, profile_name);
        }
    }

    /// Public API: Move a mod up
    pub fn move_mod_up(&self, mod_entry: &ModEntry) {
        Self::move_mod_up_static(&self.model, mod_entry, &self.vfs, &self.profile_name);
    }

    /// Public API: Move a mod down
    pub fn move_mod_down(&self, mod_entry: &ModEntry) {
        Self::move_mod_down_static(&self.model, mod_entry, &self.vfs, &self.profile_name);
    }
}

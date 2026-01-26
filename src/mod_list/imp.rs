use gtk4::prelude::*;
use gtk4::subclass::prelude::*;
use gtk4::{
    glib, gio, Box, Button, ColumnView, ColumnViewColumn, Label, Orientation, ScrolledWindow,
    SignalListItemFactory, SingleSelection, CheckButton, SearchEntry, Paned, UriLauncher,
    CustomFilter, FilterListModel, FilterChange, ProgressBar,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use crate::mod_entry::{ModEntry, ModList, ModState, VirtualFileSystem, load_mods_json, DfmodCacheKey, ModConflictSummary, detect_all_conflicts};
use crate::mods_json_view::ModsJsonView;
use crate::conflict_panel::ConflictPanel;

pub struct ModListView {
    pub column_view: RefCell<Option<ColumnView>>,
    pub model: RefCell<Option<gio::ListStore>>,
    pub filter_model: RefCell<Option<FilterListModel>>,
    pub filter: RefCell<Option<CustomFilter>>,
    pub selection_model: RefCell<Option<SingleSelection>>,
    pub vfs: RefCell<Option<VirtualFileSystem>>,
    pub search_entry: RefCell<Option<SearchEntry>>,
    pub profile_name: Rc<RefCell<Option<String>>>,
    pub mods_json_view: RefCell<Option<ModsJsonView>>,
    pub mods_json_path: RefCell<Option<PathBuf>>,
    pub paned: RefCell<Option<Paned>>,
    pub settings: RefCell<Option<gio::Settings>>,
    pub conflict_panel: RefCell<Option<ConflictPanel>>,
    // Conflict scanning state
    pub scan_button: RefCell<Option<Button>>,
    pub progress_box: RefCell<Option<Box>>,
    pub progress_bar: RefCell<Option<ProgressBar>>,
    pub progress_label: RefCell<Option<Label>>,
    pub conflict_results: Rc<RefCell<HashMap<PathBuf, ModConflictSummary>>>,
    pub dfmod_cache: Arc<Mutex<HashMap<DfmodCacheKey, Vec<String>>>>,
    pub is_scanning: Rc<RefCell<bool>>,
}

impl Default for ModListView {
    fn default() -> Self {
        Self {
            column_view: RefCell::new(None),
            model: RefCell::new(None),
            filter_model: RefCell::new(None),
            filter: RefCell::new(None),
            selection_model: RefCell::new(None),
            vfs: RefCell::new(None),
            search_entry: RefCell::new(None),
            profile_name: Rc::new(RefCell::new(None)),
            mods_json_view: RefCell::new(None),
            mods_json_path: RefCell::new(None),
            paned: RefCell::new(None),
            settings: RefCell::new(None),
            conflict_panel: RefCell::new(None),
            scan_button: RefCell::new(None),
            progress_box: RefCell::new(None),
            progress_bar: RefCell::new(None),
            progress_label: RefCell::new(None),
            conflict_results: Rc::new(RefCell::new(HashMap::new())),
            dfmod_cache: Arc::new(Mutex::new(HashMap::new())),
            is_scanning: Rc::new(RefCell::new(false)),
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

        // Initialize settings
        let settings = gio::Settings::new(crate::config::APP_ID);
        self.settings.replace(Some(settings.clone()));

        // Create horizontal Paned (resizable split)
        let paned = Paned::new(Orientation::Horizontal);

        // Left side: Mod folders list
        let left_box = Box::new(Orientation::Vertical, 6);
        left_box.set_margin_top(12);
        left_box.set_margin_bottom(12);
        left_box.set_margin_start(12);
        left_box.set_margin_end(6);

        // Header label
        let mod_folders_label = Label::new(Some("Mod Folders"));
        mod_folders_label.add_css_class("heading");
        mod_folders_label.set_xalign(0.0);
        left_box.append(&mod_folders_label);

        // Create search entry
        let search_entry = SearchEntry::new();
        search_entry.set_placeholder_text(Some("Filter mods..."));
        left_box.append(&search_entry);
        self.search_entry.replace(Some(search_entry.clone()));

        // Create the ListStore to hold ModEntry objects
        let model = gio::ListStore::new::<ModEntry>();
        self.model.replace(Some(model.clone()));

        // Create filter for searching by name
        let search_text: Rc<RefCell<String>> = Rc::new(RefCell::new(String::new()));
        let search_text_clone = search_text.clone();
        let filter = CustomFilter::new(move |obj| {
            let search = search_text_clone.borrow();
            if search.is_empty() {
                return true;
            }
            if let Some(mod_entry) = obj.downcast_ref::<ModEntry>() {
                mod_entry.name().to_lowercase().contains(&search.to_lowercase())
            } else {
                true
            }
        });
        self.filter.replace(Some(filter.clone()));

        // Create filter model wrapping the ListStore
        let filter_model = FilterListModel::new(Some(model.clone()), Some(filter.clone()));
        self.filter_model.replace(Some(filter_model.clone()));

        // Create selection model using filtered model
        let selection_model = SingleSelection::new(Some(filter_model.clone()));
        selection_model.set_autoselect(false);
        selection_model.set_can_unselect(true);
        self.selection_model.replace(Some(selection_model.clone()));

        // Connect search entry to filter
        let filter_clone = filter.clone();
        let search_text_for_signal = search_text.clone();
        search_entry.connect_search_changed(move |entry| {
            *search_text_for_signal.borrow_mut() = entry.text().to_string();
            filter_clone.changed(FilterChange::Different);
        });

        // Create ColumnView
        let column_view = ColumnView::new(Some(selection_model.clone()));
        column_view.set_show_row_separators(true);
        column_view.set_show_column_separators(true);

        // Add columns
        self.add_checkbox_column(&column_view);
        self.add_name_column(&column_view);
        self.add_version_column(&column_view);
        self.add_order_column(&column_view);
        self.add_conflicts_column(&column_view);
        self.add_nexus_column(&column_view);

        // Wrap in scrolled window
        let scrolled_window = ScrolledWindow::new();
        scrolled_window.set_vexpand(true);
        scrolled_window.set_hexpand(true);
        scrolled_window.set_child(Some(&column_view));

        left_box.append(&scrolled_window);

        // Add reorder buttons below the list
        let button_box = Box::new(Orientation::Horizontal, 6);
        button_box.set_halign(gtk4::Align::Center);
        button_box.set_margin_top(6);

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

        // Second separator
        let separator2 = gtk4::Separator::new(Orientation::Vertical);
        separator2.set_margin_start(6);
        separator2.set_margin_end(6);

        let scan_button = Button::with_label("Scan Conflicts");
        self.scan_button.replace(Some(scan_button.clone()));

        button_box.append(&top_button);
        button_box.append(&up_button);
        button_box.append(&down_button);
        button_box.append(&bottom_button);
        button_box.append(&separator);
        button_box.append(&enable_all_button);
        button_box.append(&disable_all_button);
        button_box.append(&separator2);
        button_box.append(&scan_button);

        left_box.append(&button_box);

        // Progress bar (hidden by default)
        let progress_box = Box::new(Orientation::Horizontal, 6);
        progress_box.set_visible(false);
        progress_box.set_margin_top(6);

        let progress_bar = ProgressBar::new();
        progress_bar.set_hexpand(true);
        progress_bar.set_show_text(false);

        let progress_label = Label::new(Some(""));
        progress_label.set_xalign(0.0);

        progress_box.append(&progress_bar);
        progress_box.append(&progress_label);

        left_box.append(&progress_box);

        self.progress_box.replace(Some(progress_box.clone()));
        self.progress_bar.replace(Some(progress_bar.clone()));
        self.progress_label.replace(Some(progress_label.clone()));

        // Store references needed for button callbacks
        let model_ref = self.model.clone();
        let vfs_ref = self.vfs.clone();
        let profile_name_ref = self.profile_name.clone();

        // Connect buttons to move selected mod
        let selection_model_clone = column_view.model().unwrap();
        let selection = selection_model_clone.downcast::<SingleSelection>().unwrap();

        let selection_clone = selection.clone();
        let model_clone = model_ref.clone();
        let vfs_clone = vfs_ref.clone();
        let profile_clone = profile_name_ref.clone();
        top_button.connect_clicked(move |_| {
            if let Some(item) = selection_clone.selected_item() {
                if let Ok(mod_entry) = item.downcast::<ModEntry>() {
                    Self::move_mod_to_top_static(&model_clone, &mod_entry, &vfs_clone, &profile_clone, &selection_clone);
                }
            }
        });

        let selection_clone = selection.clone();
        let model_clone = model_ref.clone();
        let vfs_clone = vfs_ref.clone();
        let profile_clone = profile_name_ref.clone();
        up_button.connect_clicked(move |_| {
            if let Some(item) = selection_clone.selected_item() {
                if let Ok(mod_entry) = item.downcast::<ModEntry>() {
                    Self::move_mod_up_static(&model_clone, &mod_entry, &vfs_clone, &profile_clone, &selection_clone);
                }
            }
        });

        let selection_clone = selection.clone();
        let model_clone = model_ref.clone();
        let vfs_clone = vfs_ref.clone();
        let profile_clone = profile_name_ref.clone();
        down_button.connect_clicked(move |_| {
            if let Some(item) = selection_clone.selected_item() {
                if let Ok(mod_entry) = item.downcast::<ModEntry>() {
                    Self::move_mod_down_static(&model_clone, &mod_entry, &vfs_clone, &profile_clone, &selection_clone);
                }
            }
        });

        let selection_clone = selection.clone();
        let model_clone = model_ref.clone();
        let vfs_clone = vfs_ref.clone();
        let profile_clone = profile_name_ref.clone();
        bottom_button.connect_clicked(move |_| {
            if let Some(item) = selection_clone.selected_item() {
                if let Ok(mod_entry) = item.downcast::<ModEntry>() {
                    Self::move_mod_to_bottom_static(&model_clone, &mod_entry, &vfs_clone, &profile_clone, &selection_clone);
                }
            }
        });

        // Connect enable/disable all buttons
        let model_clone = model_ref.clone();
        let profile_clone = profile_name_ref.clone();
        enable_all_button.connect_clicked(move |_| {
            Self::enable_all_mods_static(&model_clone, &profile_clone);
        });

        let model_clone = model_ref.clone();
        let profile_clone = profile_name_ref.clone();
        disable_all_button.connect_clicked(move |_| {
            Self::disable_all_mods_static(&model_clone, &profile_clone);
        });

        // Connect scan button
        let model_clone = model_ref.clone();
        let is_scanning_clone = self.is_scanning.clone();
        let conflict_results_clone = self.conflict_results.clone();
        let dfmod_cache_clone = self.dfmod_cache.clone();
        let progress_box_clone = progress_box.clone();
        let progress_bar_clone = progress_bar.clone();
        let progress_label_clone = progress_label.clone();
        let scan_button_clone = scan_button.clone();
        scan_button.connect_clicked(move |_| {
            Self::start_conflict_scan(
                &model_clone,
                &is_scanning_clone,
                &conflict_results_clone,
                &dfmod_cache_clone,
                &progress_box_clone,
                &progress_bar_clone,
                &progress_label_clone,
                &scan_button_clone,
            );
        });

        paned.set_start_child(Some(&left_box));

        // Right side: Mods.json view
        let mods_json_view = ModsJsonView::new();
        self.mods_json_view.replace(Some(mods_json_view.clone()));
        paned.set_end_child(Some(&mods_json_view));

        // Create conflict panel
        let conflict_panel = ConflictPanel::new();
        conflict_panel.set_margin_start(12);
        conflict_panel.set_margin_end(12);
        conflict_panel.set_margin_top(6);
        self.conflict_panel.replace(Some(conflict_panel.clone()));

        // Update conflict panel when mod is selected (uses cached conflict data)
        let conflict_panel_clone = conflict_panel.clone();
        let conflict_results_for_selection = self.conflict_results.clone();
        selection_model.connect_selected_item_notify(move |sel| {
            if let Some(mod_entry) = sel.selected_item().and_then(|i| i.downcast::<ModEntry>().ok()) {
                // Look up cached conflict data for this mod
                let mod_path = mod_entry.path();
                let results = conflict_results_for_selection.borrow();
                let summary = results.get(&mod_path);

                conflict_panel_clone.update_with_cached_conflicts(&mod_path, summary);
            } else {
                conflict_panel_clone.clear();
            }
        });

        // Load saved paned position
        let saved_position = settings.int("paned-position");
        paned.set_position(saved_position);

        // Save paned position when it changes
        let settings_clone = settings.clone();
        paned.connect_position_notify(move |paned| {
            let position = paned.position();
            settings_clone.set_int("paned-position", position).ok();
        });

        // Store paned reference
        self.paned.replace(Some(paned.clone()));

        obj.append(&paned);

        // Add conflict panel between paned and Apply button
        obj.append(&conflict_panel);

        // Add Apply button at the bottom (outside paned, spans full width)
        let button_box = Box::new(Orientation::Horizontal, 6);
        button_box.set_halign(gtk4::Align::End);
        button_box.set_margin_top(6);
        button_box.set_margin_bottom(12);
        button_box.set_margin_start(12);
        button_box.set_margin_end(12);

        let apply_button = Button::with_label("Apply Changes");
        apply_button.add_css_class("suggested-action");
        button_box.append(&apply_button);

        obj.append(&button_box);

        // Connect Apply button to apply all changes
        let widget = obj.clone();
        apply_button.connect_clicked(move |_| {
            widget.imp().apply_changes();
        });

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

            // Bind the enabled property and store the binding
            let binding = mod_entry
                .bind_property("enabled", &check_button, "active")
                .bidirectional()
                .sync_create()
                .build();

            // Connect to toggled signal to save state
            let model_clone = model_ref.clone();
            let profile_name_clone = profile_name_ref.clone();
            let handler_id = check_button.connect_toggled(move |_btn| {
                // Save mod state (VFS rebuild happens on Apply button)
                Self::save_mod_state_static(&model_clone, &profile_name_clone);
            });

            // Store binding and handler for cleanup in unbind
            unsafe {
                list_item.set_data("binding", binding);
                list_item.set_data("handler-id", handler_id);
            }
        });

        // Unbind: Clean up bindings and signal handlers
        factory.connect_unbind(move |_factory, item| {
            let list_item = item.downcast_ref::<gtk4::ListItem>()
                .expect("Item must be ListItem");

            // Unbind the property binding
            unsafe {
                if let Some(binding) = list_item.steal_data::<glib::Binding>("binding") {
                    binding.unbind();
                }
            }

            // Disconnect the signal handler
            if let Some(check_button) = list_item.child().and_downcast::<CheckButton>() {
                unsafe {
                    if let Some(handler_id) = list_item.steal_data::<glib::SignalHandlerId>("handler-id") {
                        check_button.disconnect(handler_id);
                    }
                }
            }
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

            let binding = mod_entry
                .bind_property("name", &label, "label")
                .sync_create()
                .build();

            unsafe {
                list_item.set_data("name-binding", binding);
            }
        });

        factory.connect_unbind(move |_factory, item| {
            let list_item = item.downcast_ref::<gtk4::ListItem>()
                .expect("Item must be ListItem");

            unsafe {
                if let Some(binding) = list_item.steal_data::<glib::Binding>("name-binding") {
                    binding.unbind();
                }
            }
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

            let binding = mod_entry
                .bind_property("version", &label, "label")
                .sync_create()
                .build();

            unsafe {
                list_item.set_data("version-binding", binding);
            }
        });

        factory.connect_unbind(move |_factory, item| {
            let list_item = item.downcast_ref::<gtk4::ListItem>()
                .expect("Item must be ListItem");

            unsafe {
                if let Some(binding) = list_item.steal_data::<glib::Binding>("version-binding") {
                    binding.unbind();
                }
            }
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
            let handler_id = mod_entry.connect_notify_local(
                Some("order"),
                move |entry, _| {
                    label_clone.set_text(&entry.order().to_string());
                },
            );

            // Store handler for cleanup
            unsafe {
                list_item.set_data("order-handler-id", handler_id);
            }
        });

        factory.connect_unbind(move |_factory, item| {
            let list_item = item.downcast_ref::<gtk4::ListItem>()
                .expect("Item must be ListItem");

            if let Some(mod_entry) = list_item.item().and_downcast::<ModEntry>() {
                unsafe {
                    if let Some(handler_id) = list_item.steal_data::<glib::SignalHandlerId>("order-handler-id") {
                        mod_entry.disconnect(handler_id);
                    }
                }
            }
        });

        let column = ColumnViewColumn::new(Some("Order"), Some(factory));
        column.set_fixed_width(80);
        column_view.append_column(&column);
    }

    fn add_conflicts_column(&self, column_view: &ColumnView) {
        let factory = SignalListItemFactory::new();

        factory.connect_setup(move |_factory, item| {
            let list_item = item.downcast_ref::<gtk4::ListItem>()
                .expect("Item must be ListItem");
            let label = Label::new(Some("-"));
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

            // Initial value
            let count = mod_entry.conflict_count();
            if count > 0 {
                label.set_text(&count.to_string());
                label.add_css_class("warning");
            } else {
                label.set_text("-");
                label.remove_css_class("warning");
            }

            // Update when conflict_count property changes
            let label_clone = label.clone();
            let handler_id = mod_entry.connect_notify_local(
                Some("conflict-count"),
                move |entry, _| {
                    let count = entry.conflict_count();
                    if count > 0 {
                        label_clone.set_text(&count.to_string());
                        label_clone.add_css_class("warning");
                    } else {
                        label_clone.set_text("-");
                        label_clone.remove_css_class("warning");
                    }
                },
            );

            // Store handler for cleanup
            unsafe {
                list_item.set_data("conflict-handler-id", handler_id);
            }
        });

        factory.connect_unbind(move |_factory, item| {
            let list_item = item.downcast_ref::<gtk4::ListItem>()
                .expect("Item must be ListItem");

            if let Some(mod_entry) = list_item.item().and_downcast::<ModEntry>() {
                unsafe {
                    if let Some(handler_id) = list_item.steal_data::<glib::SignalHandlerId>("conflict-handler-id") {
                        mod_entry.disconnect(handler_id);
                    }
                }
            }
        });

        let column = ColumnViewColumn::new(Some("⚠"), Some(factory));
        column.set_fixed_width(50);
        column_view.append_column(&column);
    }

    fn add_nexus_column(&self, column_view: &ColumnView) {
        let factory = SignalListItemFactory::new();

        // Setup phase: Create box with two icon buttons
        factory.connect_setup(move |_factory, item| {
            let list_item = item.downcast_ref::<gtk4::ListItem>()
                .expect("Item must be ListItem");

            let button_box = Box::new(Orientation::Horizontal, 4);
            button_box.set_margin_end(16);

            // Nexus button with external link icon
            let nexus_button = Button::from_icon_name("go-jump-symbolic");
            nexus_button.add_css_class("flat");

            // Folder button with folder icon
            let folder_button = Button::from_icon_name("folder-open-symbolic");
            folder_button.add_css_class("flat");

            button_box.append(&nexus_button);
            button_box.append(&folder_button);
            list_item.set_child(Some(&button_box));
        });

        // Bind phase: Connect button clicks
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

            // Get the buttons from the box
            let nexus_button = button_box
                .first_child()
                .and_downcast::<Button>()
                .expect("First child must be Button");

            let folder_button = nexus_button
                .next_sibling()
                .and_downcast::<Button>()
                .expect("Second child must be Button");

            // Get nexus_id and configure nexus button state
            let nexus_id = mod_entry.nexus_id();

            if let Some(id) = nexus_id {
                nexus_button.set_sensitive(true);
                nexus_button.set_tooltip_text(Some(&format!("Open mod {} on Nexus Mods", id)));

                // Connect click handler to open Nexus URL
                let id_clone = id.clone();
                let handler_id = nexus_button.connect_clicked(move |btn| {
                    let url = format!("https://www.nexusmods.com/daggerfallunity/mods/{}", id_clone);

                    let root = btn.root();
                    if let Some(window) = root.and_downcast::<gtk4::Window>() {
                        let launcher = UriLauncher::new(&url);
                        launcher.launch(Some(&window), gio::Cancellable::NONE, |result| {
                            if let Err(e) = result {
                                eprintln!("Failed to open URL: {}", e);
                            }
                        });
                    }
                });
                unsafe { nexus_button.set_data("handler-id", handler_id); }
            } else {
                nexus_button.set_sensitive(false);
                nexus_button.set_tooltip_text(Some("This mod is not from Nexus Mods"));
            }

            // Configure folder button - always enabled
            let mod_path = mod_entry.path();
            folder_button.set_sensitive(true);
            folder_button.set_tooltip_text(Some(&format!("Open folder: {}", mod_path.display())));

            let folder_handler_id = folder_button.connect_clicked(move |_btn| {
                // Use the `open` crate for cross-platform folder opening
                if let Err(e) = open::that(&mod_path) {
                    eprintln!("Failed to open folder: {}", e);
                }
            });
            unsafe { folder_button.set_data("folder-handler-id", folder_handler_id); }
        });

        // Unbind phase: Disconnect signal handlers
        factory.connect_unbind(move |_factory, item| {
            let list_item = item.downcast_ref::<gtk4::ListItem>()
                .expect("Item must be ListItem");

            let button_box = list_item
                .child()
                .and_downcast::<Box>()
                .expect("Child must be Box");

            let nexus_button = button_box
                .first_child()
                .and_downcast::<Button>()
                .expect("First child must be Button");

            let folder_button = nexus_button
                .next_sibling()
                .and_downcast::<Button>()
                .expect("Second child must be Button");

            // Disconnect nexus button handler if exists
            unsafe {
                if let Some(handler_id) = nexus_button.steal_data::<glib::SignalHandlerId>("handler-id") {
                    nexus_button.disconnect(handler_id);
                }
            }

            // Disconnect folder button handler
            unsafe {
                if let Some(handler_id) = folder_button.steal_data::<glib::SignalHandlerId>("folder-handler-id") {
                    folder_button.disconnect(handler_id);
                }
            }
        });

        let column = ColumnViewColumn::new(Some("Nexus"), Some(factory));
        column.set_fixed_width(100);
        column_view.append_column(&column);
    }

    pub fn load_mods(&self, mods_folder: &std::path::Path, game_mods_folder: &std::path::Path, profile_name: &str, mods_json_path: &std::path::Path) {
        // Store profile name
        self.profile_name.replace(Some(profile_name.to_string()));

        // Store mods_json_path
        self.mods_json_path.replace(Some(mods_json_path.to_path_buf()));

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

        // Load Mods.json into ModsJsonView
        if let Some(mods_json_view) = self.mods_json_view.borrow().as_ref() {
            mods_json_view.load_mods_json(mods_json_path);
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
        _vfs: &RefCell<Option<VirtualFileSystem>>,
        profile_name: &Rc<RefCell<Option<String>>>,
        selection: &SingleSelection
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

            // Restore selection at new position (moved up by 1)
            if position > 0 {
                selection.set_selected(position - 1);
            }

            // Drop the borrow before calling static methods
            drop(model_borrow);

            // Save state (VFS rebuild happens on Apply button)
            Self::save_mod_state_static(model, profile_name);
        }
    }

    /// Move a mod down in the list (static version for closures)
    fn move_mod_down_static(
        model: &RefCell<Option<gio::ListStore>>,
        mod_entry: &ModEntry,
        _vfs: &RefCell<Option<VirtualFileSystem>>,
        profile_name: &Rc<RefCell<Option<String>>>,
        selection: &SingleSelection
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

            // Restore selection at new position (moved down by 1)
            if position < n_items - 1 {
                selection.set_selected(position + 1);
            }

            // Drop the borrow before calling static methods
            drop(model_borrow);

            // Save state (VFS rebuild happens on Apply button)
            Self::save_mod_state_static(model, profile_name);
        }
    }

    /// Move a mod to top of the list (static version for closures)
    fn move_mod_to_top_static(
        model: &RefCell<Option<gio::ListStore>>,
        mod_entry: &ModEntry,
        _vfs: &RefCell<Option<VirtualFileSystem>>,
        profile_name: &Rc<RefCell<Option<String>>>,
        selection: &SingleSelection
    ) {
        let model_borrow = model.borrow();
        if let Some(model_store) = model_borrow.as_ref() {
            let position = Self::find_mod_position(model_store, mod_entry);

            if position == 0 {
                return; // Already at top
            }

            // Set this mod's order to 0
            let current_order = mod_entry.order();
            mod_entry.set_order(0);

            // Shift all mods with order < current_order down by 1
            for i in 0..model_store.n_items() {
                if let Some(item) = model_store.item(i) {
                    if let Ok(entry) = item.downcast::<ModEntry>() {
                        if entry.order() < current_order && entry.name() != mod_entry.name() {
                            entry.set_order(entry.order() + 1);
                        }
                    }
                }
            }

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

            // Restore selection at position 0 (top)
            selection.set_selected(0);

            drop(model_borrow);
            Self::save_mod_state_static(model, profile_name);
        }
    }

    /// Move a mod to bottom of the list (static version for closures)
    fn move_mod_to_bottom_static(
        model: &RefCell<Option<gio::ListStore>>,
        mod_entry: &ModEntry,
        _vfs: &RefCell<Option<VirtualFileSystem>>,
        profile_name: &Rc<RefCell<Option<String>>>,
        selection: &SingleSelection
    ) {
        let model_borrow = model.borrow();
        if let Some(model_store) = model_borrow.as_ref() {
            let position = Self::find_mod_position(model_store, mod_entry);
            let last_position = model_store.n_items() - 1;

            if position >= last_position {
                return; // Already at bottom
            }

            // Set this mod's order to last
            let current_order = mod_entry.order();
            mod_entry.set_order(last_position);

            // Shift all mods with order > current_order up by 1
            for i in 0..model_store.n_items() {
                if let Some(item) = model_store.item(i) {
                    if let Ok(entry) = item.downcast::<ModEntry>() {
                        if entry.order() > current_order && entry.name() != mod_entry.name() {
                            entry.set_order(entry.order() - 1);
                        }
                    }
                }
            }

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

            // Restore selection at bottom position
            selection.set_selected(last_position);

            drop(model_borrow);
            Self::save_mod_state_static(model, profile_name);
        }
    }

    /// Enable all mods (static version for closures)
    fn enable_all_mods_static(
        model: &RefCell<Option<gio::ListStore>>,
        profile_name: &Rc<RefCell<Option<String>>>
    ) {
        let model_borrow = model.borrow();
        if let Some(model_store) = model_borrow.as_ref() {
            // Enable all mods
            for i in 0..model_store.n_items() {
                if let Some(item) = model_store.item(i) {
                    if let Ok(mod_entry) = item.downcast::<ModEntry>() {
                        mod_entry.set_enabled(true);
                    }
                }
            }

            drop(model_borrow);
            Self::save_mod_state_static(model, profile_name);
        }
    }

    /// Disable all mods (static version for closures)
    fn disable_all_mods_static(
        model: &RefCell<Option<gio::ListStore>>,
        profile_name: &Rc<RefCell<Option<String>>>
    ) {
        let model_borrow = model.borrow();
        if let Some(model_store) = model_borrow.as_ref() {
            // Disable all mods
            for i in 0..model_store.n_items() {
                if let Some(item) = model_store.item(i) {
                    if let Ok(mod_entry) = item.downcast::<ModEntry>() {
                        mod_entry.set_enabled(false);
                    }
                }
            }

            drop(model_borrow);
            Self::save_mod_state_static(model, profile_name);
        }
    }

    /// Public API: Move a mod up
    pub fn move_mod_up(&self, mod_entry: &ModEntry) {
        if let Some(selection) = self.selection_model.borrow().as_ref() {
            Self::move_mod_up_static(&self.model, mod_entry, &self.vfs, &self.profile_name, selection);
        }
    }

    /// Public API: Move a mod down
    pub fn move_mod_down(&self, mod_entry: &ModEntry) {
        if let Some(selection) = self.selection_model.borrow().as_ref() {
            Self::move_mod_down_static(&self.model, mod_entry, &self.vfs, &self.profile_name, selection);
        }
    }

    /// Apply all changes: rebuild VFS and update Mods.json
    pub fn apply_changes(&self) {
        // Step 1: Rebuild VFS
        self.rebuild_vfs();

        // Step 2: Save current ModsJsonView state first (preserves manual reordering)
        // Then sync with enabled mods
        if let Err(e) = self.sync_mods_json() {
            eprintln!("Failed to sync Mods.json: {}", e);
        }
    }

    /// Sync Mods.json with enabled mods, preserving manual reordering
    fn sync_mods_json(&self) -> Result<(), String> {
        let model = self.model.borrow();
        let mods_json_path = self.mods_json_path.borrow();
        let mods_json_view = self.mods_json_view.borrow();

        let model = model.as_ref().ok_or("Model not initialized")?;
        let mods_json_path = mods_json_path.as_ref().ok_or("Mods.json path not set")?;
        let mods_json_view = mods_json_view.as_ref().ok_or("ModsJsonView not initialized")?;

        // Step 1: Save current ModsJsonView state (preserves manual reordering)
        mods_json_view.save_mods_json()?;

        // Step 2: Load the saved state
        let mut existing_entries = load_mods_json(mods_json_path)?;

        // Step 3: Get list of currently enabled mod folders with .dfmod files
        let mut enabled_mod_names = std::collections::HashSet::new();
        for i in 0..model.n_items() {
            if let Some(obj) = model.item(i) {
                if let Ok(mod_entry) = obj.downcast::<ModEntry>() {
                    if mod_entry.enabled() {
                        // Check if this mod has .dfmod files
                        if let Ok(dfmod_infos) = crate::mod_entry::parse_dfmod(&mod_entry.path()) {
                            for dfmod_info in dfmod_infos {
                                enabled_mod_names.insert(dfmod_info.file_name.clone());
                            }
                        }
                    }
                }
            }
        }

        // Step 4: Remove entries for mods that are no longer enabled
        existing_entries.retain(|entry| enabled_mod_names.contains(&entry.file_name));

        // Step 5: Add new mods that aren't in Mods.json yet
        let existing_file_names: std::collections::HashSet<_> =
            existing_entries.iter().map(|e| e.file_name.clone()).collect();

        let mut next_priority = existing_entries.iter()
            .map(|e| e.load_priority)
            .max()
            .unwrap_or(0)
            .saturating_add(1);

        for i in 0..model.n_items() {
            if let Some(obj) = model.item(i) {
                if let Ok(mod_entry) = obj.downcast::<ModEntry>() {
                    if mod_entry.enabled() {
                        if let Ok(dfmod_infos) = crate::mod_entry::parse_dfmod(&mod_entry.path()) {
                            for dfmod_info in dfmod_infos {
                                if !existing_file_names.contains(&dfmod_info.file_name) {
                                    existing_entries.push(crate::mod_entry::ModsJsonEntry {
                                        file_name: dfmod_info.file_name,
                                        title: dfmod_info.title,
                                        enabled: true,
                                        load_priority: next_priority,
                                    });
                                    next_priority += 1;
                                }
                            }
                        }
                    }
                }
            }
        }

        // Step 6: Sort by load_priority
        existing_entries.sort_by_key(|e| e.load_priority);

        // Step 7: Renumber priorities sequentially (fill gaps)
        for (i, entry) in existing_entries.iter_mut().enumerate() {
            entry.load_priority = i as u32;
        }

        // Step 8: Save to disk
        crate::mod_entry::save_mods_json(mods_json_path, &existing_entries)?;

        // Step 9: Reload ModsJsonView (sorted by priority)
        mods_json_view.load_mods_json(mods_json_path);

        Ok(())
    }

    /// Start async conflict scanning on a background thread
    fn start_conflict_scan(
        model: &RefCell<Option<gio::ListStore>>,
        is_scanning: &Rc<RefCell<bool>>,
        conflict_results: &Rc<RefCell<HashMap<PathBuf, ModConflictSummary>>>,
        dfmod_cache: &Arc<Mutex<HashMap<DfmodCacheKey, Vec<String>>>>,
        progress_box: &Box,
        progress_bar: &ProgressBar,
        progress_label: &Label,
        scan_button: &Button,
    ) {
        // Check if already scanning
        if *is_scanning.borrow() {
            return;
        }
        *is_scanning.borrow_mut() = true;

        // Collect enabled mods
        let mut enabled_mods: Vec<(String, PathBuf)> = Vec::new();
        if let Some(model_store) = model.borrow().as_ref() {
            for i in 0..model_store.n_items() {
                if let Some(item) = model_store.item(i) {
                    if let Ok(entry) = item.downcast::<ModEntry>() {
                        if entry.enabled() {
                            enabled_mods.push((entry.name(), entry.path()));
                        }
                    }
                }
            }
        }

        if enabled_mods.is_empty() {
            *is_scanning.borrow_mut() = false;
            return;
        }

        // Show progress UI and disable scan button
        progress_box.set_visible(true);
        scan_button.set_sensitive(false);
        progress_bar.set_fraction(0.0);
        progress_label.set_text("Starting scan...");

        // Clone cache for thread
        let cache_clone = dfmod_cache.clone();

        // Shared state for progress
        let progress_state = Arc::new(Mutex::new(ScanProgressState {
            current: 0,
            total: enabled_mods.len(),
            current_mod: String::new(),
            completed: false,
            results: None,
        }));

        let progress_state_thread = progress_state.clone();

        // Spawn background thread
        std::thread::spawn(move || {
            // Get a mutable copy of the cache
            let mut local_cache = {
                let guard = cache_clone.lock().unwrap();
                guard.clone()
            };

            let results = detect_all_conflicts(
                &enabled_mods,
                &mut local_cache,
                |mod_name, current, total| {
                    let mut state = progress_state_thread.lock().unwrap();
                    state.current = current;
                    state.total = total;
                    state.current_mod = mod_name;
                },
            );

            // Update the shared cache with any new entries
            {
                let mut guard = cache_clone.lock().unwrap();
                for (key, value) in local_cache {
                    guard.entry(key).or_insert(value);
                }
            }

            // Mark as completed
            let mut state = progress_state_thread.lock().unwrap();
            state.completed = true;
            state.results = Some(results);
        });

        // Poll progress from main thread
        let is_scanning_clone = is_scanning.clone();
        let conflict_results_clone = conflict_results.clone();
        let model_clone = model.borrow().clone();
        let progress_box_clone = progress_box.clone();
        let progress_bar_clone = progress_bar.clone();
        let progress_label_clone = progress_label.clone();
        let scan_button_clone = scan_button.clone();

        glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
            let state = progress_state.lock().unwrap();

            if state.completed {
                // Get results
                if let Some(ref results) = state.results {
                    // Update conflict counts on ModEntry objects
                    if let Some(ref model_store) = model_clone {
                        for i in 0..model_store.n_items() {
                            if let Some(item) = model_store.item(i) {
                                if let Ok(entry) = item.downcast::<ModEntry>() {
                                    let path = entry.path();
                                    let count = results
                                        .get(&path)
                                        .map(|s| s.total_conflict_count as u32)
                                        .unwrap_or(0);
                                    entry.set_conflict_count(count);
                                }
                            }
                        }
                    }

                    // Store results
                    *conflict_results_clone.borrow_mut() = results.clone();
                }

                // Hide progress UI and re-enable scan button
                progress_box_clone.set_visible(false);
                scan_button_clone.set_sensitive(true);
                *is_scanning_clone.borrow_mut() = false;

                return glib::ControlFlow::Break;
            }

            // Update progress UI
            let fraction = if state.total > 0 {
                state.current as f64 / state.total as f64
            } else {
                0.0
            };
            progress_bar_clone.set_fraction(fraction);
            progress_label_clone.set_text(&format!("{}/{} {}", state.current, state.total, state.current_mod));

            glib::ControlFlow::Continue
        });
    }
}

/// Progress state shared between background thread and main thread
struct ScanProgressState {
    current: usize,
    total: usize,
    current_mod: String,
    completed: bool,
    results: Option<HashMap<PathBuf, ModConflictSummary>>,
}

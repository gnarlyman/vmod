use gtk4::prelude::*;
use gtk4::subclass::prelude::*;
use gtk4::{
    glib, gio, Box, Button, ColumnView, ColumnViewColumn, Label, Orientation, ScrolledWindow,
    SignalListItemFactory, SingleSelection, CheckButton, SearchEntry, Paned, UriLauncher,
    CustomFilter, FilterListModel, FilterChange, ProgressBar, Entry,
};
use std::collections::HashSet;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use crate::mod_entry::{ModEntry, ModList, ModState, VirtualFileSystem, load_mods_json, save_mods_json, DfmodCacheKey, ModConflictSummary, detect_all_conflicts, SectionHeader, SectionsConfig, BackupManager};
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
    // Section management
    pub sections_config: Rc<RefCell<SectionsConfig>>,
    pub collapsed_sections: Rc<RefCell<HashSet<String>>>,
    pub profile_path: Rc<RefCell<Option<PathBuf>>>,
    // Stored paths for reload
    pub mods_folder: RefCell<Option<PathBuf>>,
    pub game_mods_folder: RefCell<Option<PathBuf>>,
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
            sections_config: Rc::new(RefCell::new(SectionsConfig::default())),
            collapsed_sections: Rc::new(RefCell::new(HashSet::new())),
            profile_path: Rc::new(RefCell::new(None)),
            mods_folder: RefCell::new(None),
            game_mods_folder: RefCell::new(None),
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

        // Create filter row with search entry and action buttons
        let filter_row = Box::new(Orientation::Horizontal, 6);

        // Create search entry
        let search_entry = SearchEntry::new();
        search_entry.set_placeholder_text(Some("Filter mods..."));
        search_entry.set_hexpand(true);
        filter_row.append(&search_entry);
        self.search_entry.replace(Some(search_entry.clone()));

        // Add Section button
        let add_section_button = Button::with_label("+ Section");
        add_section_button.set_tooltip_text(Some("Add a new collapsible section"));
        filter_row.append(&add_section_button);

        // Scan Conflicts button
        let scan_button = Button::with_label("Scan Conflicts");
        self.scan_button.replace(Some(scan_button.clone()));
        filter_row.append(&scan_button);

        // Backup button
        let backup_button = Button::with_label("Backup");
        backup_button.set_tooltip_text(Some("Create or restore mod list backups"));
        filter_row.append(&backup_button);

        // Refresh button
        let refresh_button = Button::with_label("Refresh");
        refresh_button.set_tooltip_text(Some("Rescan mod folders"));
        filter_row.append(&refresh_button);

        left_box.append(&filter_row);

        // Create the ListStore to hold ModEntry and SectionHeader objects
        let model = gio::ListStore::new::<glib::Object>();
        self.model.replace(Some(model.clone()));

        // Create filter for searching by name and handling collapsed sections
        let search_text: Rc<RefCell<String>> = Rc::new(RefCell::new(String::new()));
        let search_text_clone = search_text.clone();
        let collapsed_sections = self.collapsed_sections.clone();
        let filter = CustomFilter::new(move |obj| {
            // Section headers are always visible
            if let Some(section) = obj.downcast_ref::<SectionHeader>() {
                let search = search_text_clone.borrow();
                if search.is_empty() {
                    return true;
                }
                // Show section if its name matches search
                return section.name().to_lowercase().contains(&search.to_lowercase());
            }

            // For mod entries
            if let Some(mod_entry) = obj.downcast_ref::<ModEntry>() {
                // Check if mod's section is collapsed
                if let Some(section_id) = mod_entry.section_id() {
                    if collapsed_sections.borrow().contains(&section_id) {
                        return false; // Hide mods in collapsed sections
                    }
                }

                let search = search_text_clone.borrow();
                if search.is_empty() {
                    return true;
                }
                return mod_entry.name().to_lowercase().contains(&search.to_lowercase());
            }

            true
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

        // Add separator for action buttons
        let separator2 = gtk4::Separator::new(Orientation::Vertical);
        separator2.set_margin_start(6);
        separator2.set_margin_end(6);

        // Context-sensitive action buttons (react to selection)
        let folder_button = Button::from_icon_name("folder-open-symbolic");
        folder_button.set_tooltip_text(Some("Open mod folder"));
        folder_button.set_sensitive(false);  // Disabled until a mod is selected

        let nexus_button = Button::from_icon_name("go-jump-symbolic");
        nexus_button.set_tooltip_text(Some("Open on Nexus Mods"));
        nexus_button.set_sensitive(false);  // Disabled until a mod with nexus_id is selected

        button_box.append(&top_button);
        button_box.append(&up_button);
        button_box.append(&down_button);
        button_box.append(&bottom_button);
        button_box.append(&separator);
        button_box.append(&enable_all_button);
        button_box.append(&disable_all_button);
        button_box.append(&separator2);
        button_box.append(&folder_button);
        button_box.append(&nexus_button);

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
        let sections_config_ref = self.sections_config.clone();
        let profile_path_ref = self.profile_path.clone();

        // Connect buttons to move selected mod
        let selection_model_clone = column_view.model().unwrap();
        let selection = selection_model_clone.downcast::<SingleSelection>().unwrap();

        let selection_clone = selection.clone();
        let model_clone = model_ref.clone();
        let vfs_clone = vfs_ref.clone();
        let profile_clone = profile_name_ref.clone();
        let sections_config_clone = sections_config_ref.clone();
        let profile_path_clone = profile_path_ref.clone();
        top_button.connect_clicked(move |_| {
            // Get selected item and find its position in the underlying model (not filtered)
            if let Some(item) = selection_clone.selected_item() {
                let position = {
                    let model_borrow = model_clone.borrow();
                    model_borrow.as_ref().and_then(|m| Self::find_item_position_in_model(m, &item))
                };
                if let Some(pos) = position {
                    Self::move_mod_to_top_static(&model_clone, pos, &vfs_clone, &profile_clone, &selection_clone, &sections_config_clone, &profile_path_clone);
                }
            }
        });

        let selection_clone = selection.clone();
        let model_clone = model_ref.clone();
        let vfs_clone = vfs_ref.clone();
        let profile_clone = profile_name_ref.clone();
        let sections_config_clone = sections_config_ref.clone();
        let profile_path_clone = profile_path_ref.clone();
        up_button.connect_clicked(move |_| {
            if let Some(item) = selection_clone.selected_item() {
                let position = {
                    let model_borrow = model_clone.borrow();
                    model_borrow.as_ref().and_then(|m| Self::find_item_position_in_model(m, &item))
                };
                if let Some(pos) = position {
                    Self::move_mod_up_static(&model_clone, pos, &vfs_clone, &profile_clone, &selection_clone, &sections_config_clone, &profile_path_clone);
                }
            }
        });

        let selection_clone = selection.clone();
        let model_clone = model_ref.clone();
        let vfs_clone = vfs_ref.clone();
        let profile_clone = profile_name_ref.clone();
        let sections_config_clone = sections_config_ref.clone();
        let profile_path_clone = profile_path_ref.clone();
        down_button.connect_clicked(move |_| {
            if let Some(item) = selection_clone.selected_item() {
                let position = {
                    let model_borrow = model_clone.borrow();
                    model_borrow.as_ref().and_then(|m| Self::find_item_position_in_model(m, &item))
                };
                if let Some(pos) = position {
                    Self::move_mod_down_static(&model_clone, pos, &vfs_clone, &profile_clone, &selection_clone, &sections_config_clone, &profile_path_clone);
                }
            }
        });

        let selection_clone = selection.clone();
        let model_clone = model_ref.clone();
        let vfs_clone = vfs_ref.clone();
        let profile_clone = profile_name_ref.clone();
        let sections_config_clone = sections_config_ref.clone();
        let profile_path_clone = profile_path_ref.clone();
        bottom_button.connect_clicked(move |_| {
            // Get selected item and find its position in the underlying model (not filtered)
            if let Some(item) = selection_clone.selected_item() {
                let position = {
                    let model_borrow = model_clone.borrow();
                    model_borrow.as_ref().and_then(|m| Self::find_item_position_in_model(m, &item))
                };
                if let Some(pos) = position {
                    Self::move_mod_to_bottom_static(&model_clone, pos, &vfs_clone, &profile_clone, &selection_clone, &sections_config_clone, &profile_path_clone);
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

        // Connect folder button - opens the selected mod's folder
        let selection_clone = selection.clone();
        folder_button.connect_clicked(move |_| {
            if let Some(mod_entry) = selection_clone.selected_item().and_then(|i| i.downcast::<ModEntry>().ok()) {
                if let Err(e) = open::that(&mod_entry.path()) {
                    eprintln!("Failed to open folder: {}", e);
                }
            }
        });

        // Connect nexus button - opens the selected mod on Nexus Mods
        let selection_clone = selection.clone();
        nexus_button.connect_clicked(move |btn| {
            if let Some(mod_entry) = selection_clone.selected_item().and_then(|i| i.downcast::<ModEntry>().ok()) {
                if let Some(nexus_id) = mod_entry.nexus_id() {
                    let url = format!("https://www.nexusmods.com/daggerfallunity/mods/{}", nexus_id);
                    let root = btn.root();
                    if let Some(window) = root.and_downcast::<gtk4::Window>() {
                        let launcher = UriLauncher::new(&url);
                        launcher.launch(Some(&window), gio::Cancellable::NONE, |result| {
                            if let Err(e) = result {
                                eprintln!("Failed to open URL: {}", e);
                            }
                        });
                    }
                }
            }
        });

        // Update folder/nexus button sensitivity when selection changes
        let folder_button_clone = folder_button.clone();
        let nexus_button_clone = nexus_button.clone();
        selection.connect_selected_item_notify(move |sel| {
            if let Some(mod_entry) = sel.selected_item().and_then(|i| i.downcast::<ModEntry>().ok()) {
                // A mod is selected - enable folder button, conditionally enable nexus button
                folder_button_clone.set_sensitive(true);
                folder_button_clone.set_tooltip_text(Some(&format!("Open folder: {}", mod_entry.path().display())));

                if let Some(nexus_id) = mod_entry.nexus_id() {
                    nexus_button_clone.set_sensitive(true);
                    nexus_button_clone.set_tooltip_text(Some(&format!("Open mod {} on Nexus Mods", nexus_id)));
                } else {
                    nexus_button_clone.set_sensitive(false);
                    nexus_button_clone.set_tooltip_text(Some("This mod is not from Nexus Mods"));
                }
            } else {
                // No mod selected (or a section is selected) - disable both buttons
                folder_button_clone.set_sensitive(false);
                folder_button_clone.set_tooltip_text(Some("Open mod folder"));
                nexus_button_clone.set_sensitive(false);
                nexus_button_clone.set_tooltip_text(Some("Open on Nexus Mods"));
            }
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

        // Connect add section button
        let model_clone = model_ref.clone();
        let selection_model_clone = selection_model.clone();
        let sections_config_clone = self.sections_config.clone();
        let profile_path_clone = self.profile_path.clone();
        let filter_clone = self.filter.clone();
        let column_view_clone = column_view.clone();
        add_section_button.connect_clicked(move |_| {
            Self::add_section_at_selection(
                &model_clone,
                &selection_model_clone,
                &sections_config_clone,
                &profile_path_clone,
                &filter_clone,
                &column_view_clone,
            );
        });

        // Connect backup button
        let profile_name_for_backup = self.profile_name.clone();
        let profile_path_for_backup = self.profile_path.clone();
        let widget_for_backup = obj.clone();
        backup_button.connect_clicked(move |btn| {
            Self::show_backup_popover(
                btn,
                &profile_name_for_backup,
                &profile_path_for_backup,
                &widget_for_backup,
            );
        });

        // Connect refresh button
        let obj_for_refresh = obj.clone();
        refresh_button.connect_clicked(move |_| {
            obj_for_refresh.imp().reload();
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
        let mods_json_view_clone = mods_json_view.clone();
        selection_model.connect_selected_item_notify(move |sel| {
            if let Some(mod_entry) = sel.selected_item().and_then(|i| i.downcast::<ModEntry>().ok()) {
                // Highlight related dfmods in Mods.json panel
                if let Ok(dfmods) = crate::mod_entry::parse_dfmod_basic(&mod_entry.path()) {
                    mods_json_view_clone.highlight_entries(
                        &dfmods.iter().map(|d| d.file_name.clone()).collect::<Vec<_>>()
                    );
                }

                // Look up cached conflict data for this mod
                let mod_path = mod_entry.path();
                let results = conflict_results_for_selection.borrow();
                let summary = results.get(&mod_path);

                conflict_panel_clone.update_with_cached_conflicts(&mod_path, summary);
            } else {
                mods_json_view_clone.clear_highlights();
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

        // Setup: Create a Box that can hold either CheckButton or expander Button
        factory.connect_setup(move |_factory, item| {
            let list_item = item.downcast_ref::<gtk4::ListItem>()
                .expect("Item must be ListItem");
            let container = Box::new(Orientation::Horizontal, 0);
            list_item.set_child(Some(&container));
        });

        // Bind: Connect the CheckButton to the ModEntry's enabled property
        let model_ref = self.model.clone();
        let profile_name_ref = self.profile_name.clone();
        let collapsed_sections = self.collapsed_sections.clone();
        let filter_ref = self.filter.clone();
        let sections_config_for_checkbox = self.sections_config.clone();
        let profile_path_for_checkbox = self.profile_path.clone();
        factory.connect_bind(move |_factory, item| {
            let list_item = item.downcast_ref::<gtk4::ListItem>()
                .expect("Item must be ListItem");

            let container = list_item
                .child()
                .and_downcast::<Box>()
                .expect("Child must be Box");

            // Clear any previous children
            while let Some(child) = container.first_child() {
                container.remove(&child);
            }

            // Check if this is a section header or mod entry
            if let Some(section) = list_item.item().and_downcast::<SectionHeader>() {
                // Create expand/collapse button for section
                let is_expanded = !collapsed_sections.borrow().contains(&section.section_id());
                let icon_name = if is_expanded { "pan-down-symbolic" } else { "pan-end-symbolic" };
                let button = Button::from_icon_name(icon_name);
                button.add_css_class("flat");

                let collapsed_clone = collapsed_sections.clone();
                let section_id = section.section_id();
                let filter_clone = filter_ref.clone();
                let sections_config_clone = sections_config_for_checkbox.clone();
                let profile_path_clone = profile_path_for_checkbox.clone();
                let handler_id = button.connect_clicked(move |btn| {
                    let mut collapsed = collapsed_clone.borrow_mut();
                    let is_expanding = collapsed.contains(&section_id);
                    if is_expanding {
                        collapsed.remove(&section_id);
                        btn.set_icon_name("pan-down-symbolic");
                    } else {
                        collapsed.insert(section_id.clone());
                        btn.set_icon_name("pan-end-symbolic");
                    }
                    drop(collapsed);

                    // Persist expanded state
                    sections_config_clone.borrow_mut().update_section_expanded(&section_id, is_expanding);
                    if let Some(path) = profile_path_clone.borrow().as_ref() {
                        let _ = sections_config_clone.borrow().save(path);
                    }

                    // Trigger filter update
                    if let Some(filter) = filter_clone.borrow().as_ref() {
                        filter.changed(FilterChange::Different);
                    }
                });

                container.append(&button);

                unsafe {
                    list_item.set_data("is-section", true);
                    list_item.set_data("section-handler-id", handler_id);
                }
            } else if let Some(mod_entry) = list_item.item().and_downcast::<ModEntry>() {
                // Create checkbox for mod entry
                let check_button = CheckButton::new();

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

                container.append(&check_button);

                unsafe {
                    list_item.set_data("is-section", false);
                    list_item.set_data("binding", binding);
                    list_item.set_data("handler-id", handler_id);
                }
            }
        });

        // Unbind: Clean up bindings and signal handlers
        factory.connect_unbind(move |_factory, item| {
            let list_item = item.downcast_ref::<gtk4::ListItem>()
                .expect("Item must be ListItem");

            let is_section: bool = unsafe {
                list_item.steal_data::<bool>("is-section").unwrap_or(false)
            };

            if is_section {
                // Clean up section button handler
                if let Some(container) = list_item.child().and_downcast::<Box>() {
                    if let Some(button) = container.first_child().and_downcast::<Button>() {
                        unsafe {
                            if let Some(handler_id) = list_item.steal_data::<glib::SignalHandlerId>("section-handler-id") {
                                button.disconnect(handler_id);
                            }
                        }
                    }
                }
            } else {
                // Unbind the property binding
                unsafe {
                    if let Some(binding) = list_item.steal_data::<glib::Binding>("binding") {
                        binding.unbind();
                    }
                }

                // Disconnect the checkbox handler
                if let Some(container) = list_item.child().and_downcast::<Box>() {
                    if let Some(check_button) = container.first_child().and_downcast::<CheckButton>() {
                        unsafe {
                            if let Some(handler_id) = list_item.steal_data::<glib::SignalHandlerId>("handler-id") {
                                check_button.disconnect(handler_id);
                            }
                        }
                    }
                }
            }
        });

        let column = ColumnViewColumn::new(Some("On"), Some(factory));
        column.set_fixed_width(35);
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

            let label = list_item
                .child()
                .and_downcast::<Label>()
                .expect("Child must be Label");

            // Check if this is a section header or mod entry
            if let Some(section) = list_item.item().and_downcast::<SectionHeader>() {
                // Section header: show name with bold styling
                label.add_css_class("heading");
                label.set_markup(&format!("<b>{}</b>", glib::markup_escape_text(&section.name())));

                let binding = section
                    .bind_property("name", &label, "label")
                    .transform_to(|_, name: String| {
                        Some(format!("<b>{}</b>", glib::markup_escape_text(&name)))
                    })
                    .sync_create()
                    .build();

                label.set_use_markup(true);

                unsafe {
                    list_item.set_data("name-binding", binding);
                    list_item.set_data("is-section-name", true);
                }
            } else if let Some(mod_entry) = list_item.item().and_downcast::<ModEntry>() {
                // Regular mod entry
                label.remove_css_class("heading");
                label.set_use_markup(false);

                let binding = mod_entry
                    .bind_property("name", &label, "label")
                    .sync_create()
                    .build();

                unsafe {
                    list_item.set_data("name-binding", binding);
                    list_item.set_data("is-section-name", false);
                }
            }
        });

        factory.connect_unbind(move |_factory, item| {
            let list_item = item.downcast_ref::<gtk4::ListItem>()
                .expect("Item must be ListItem");

            // Clean up CSS class if it was a section
            if let Some(label) = list_item.child().and_downcast::<Label>() {
                label.remove_css_class("heading");
                label.set_use_markup(false);
            }

            unsafe {
                if let Some(binding) = list_item.steal_data::<glib::Binding>("name-binding") {
                    binding.unbind();
                }
                let _ = list_item.steal_data::<bool>("is-section-name");
            }
        });

        let column = ColumnViewColumn::new(Some("Name"), Some(factory));
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

            let label = list_item
                .child()
                .and_downcast::<Label>()
                .expect("Child must be Label");

            // Section headers show nothing in version column
            if list_item.item().and_downcast::<SectionHeader>().is_some() {
                label.set_text("");
                unsafe {
                    list_item.set_data("is-section-version", true);
                }
            } else if let Some(mod_entry) = list_item.item().and_downcast::<ModEntry>() {
                let binding = mod_entry
                    .bind_property("version", &label, "label")
                    .sync_create()
                    .build();

                unsafe {
                    list_item.set_data("version-binding", binding);
                    list_item.set_data("is-section-version", false);
                }
            }
        });

        factory.connect_unbind(move |_factory, item| {
            let list_item = item.downcast_ref::<gtk4::ListItem>()
                .expect("Item must be ListItem");

            unsafe {
                if let Some(binding) = list_item.steal_data::<glib::Binding>("version-binding") {
                    binding.unbind();
                }
                let _ = list_item.steal_data::<bool>("is-section-version");
            }
        });

        let column = ColumnViewColumn::new(Some("Ver"), Some(factory));
        column.set_fixed_width(60);
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

            let label = list_item
                .child()
                .and_downcast::<Label>()
                .expect("Child must be Label");

            // Section headers show nothing in order column
            if list_item.item().and_downcast::<SectionHeader>().is_some() {
                label.set_text("");
                unsafe {
                    list_item.set_data("is-section-order", true);
                }
            } else if let Some(mod_entry) = list_item.item().and_downcast::<ModEntry>() {
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
                    list_item.set_data("is-section-order", false);
                }
            }
        });

        factory.connect_unbind(move |_factory, item| {
            let list_item = item.downcast_ref::<gtk4::ListItem>()
                .expect("Item must be ListItem");

            unsafe {
                let is_section = list_item.steal_data::<bool>("is-section-order").unwrap_or(false);
                if !is_section {
                    if let Some(mod_entry) = list_item.item().and_downcast::<ModEntry>() {
                        if let Some(handler_id) = list_item.steal_data::<glib::SignalHandlerId>("order-handler-id") {
                            mod_entry.disconnect(handler_id);
                        }
                    }
                }
            }
        });

        let column = ColumnViewColumn::new(Some("#"), Some(factory));
        column.set_fixed_width(35);
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

            let label = list_item
                .child()
                .and_downcast::<Label>()
                .expect("Child must be Label");

            // Section headers show nothing in conflicts column
            if list_item.item().and_downcast::<SectionHeader>().is_some() {
                label.set_text("");
                label.remove_css_class("warning");
                unsafe {
                    list_item.set_data("is-section-conflict", true);
                }
            } else if let Some(mod_entry) = list_item.item().and_downcast::<ModEntry>() {
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
                    list_item.set_data("is-section-conflict", false);
                }
            }
        });

        factory.connect_unbind(move |_factory, item| {
            let list_item = item.downcast_ref::<gtk4::ListItem>()
                .expect("Item must be ListItem");

            unsafe {
                let is_section = list_item.steal_data::<bool>("is-section-conflict").unwrap_or(false);
                if !is_section {
                    if let Some(mod_entry) = list_item.item().and_downcast::<ModEntry>() {
                        if let Some(handler_id) = list_item.steal_data::<glib::SignalHandlerId>("conflict-handler-id") {
                            mod_entry.disconnect(handler_id);
                        }
                    }
                }
            }

            // Clear warning class
            if let Some(label) = list_item.child().and_downcast::<Label>() {
                label.remove_css_class("warning");
            }
        });

        let column = ColumnViewColumn::new(Some("⚠"), Some(factory));
        column.set_fixed_width(35);
        column_view.append_column(&column);
    }

    pub fn load_mods(&self, mods_folder: &std::path::Path, game_mods_folder: &std::path::Path, profile_name: &str, mods_json_path: &std::path::Path) {
        // Store profile name
        self.profile_name.replace(Some(profile_name.to_string()));

        // Store mods_json_path
        self.mods_json_path.replace(Some(mods_json_path.to_path_buf()));

        // Store profile path for sections config
        self.profile_path.replace(Some(mods_folder.to_path_buf()));

        // Store paths for reload
        self.mods_folder.replace(Some(mods_folder.to_path_buf()));
        self.game_mods_folder.replace(Some(game_mods_folder.to_path_buf()));

        // Create VFS manager
        let vfs = VirtualFileSystem::new(game_mods_folder.to_path_buf());
        self.vfs.replace(Some(vfs));

        // Load saved mod state
        let mod_state = match ModState::load(profile_name) {
            Ok(state) => state,
            Err(_) => ModState::default(),
        };

        // Load sections config
        let sections_config = SectionsConfig::load(mods_folder);
        self.sections_config.replace(sections_config.clone());

        // Scan mods folder
        let mut mods = ModList::scan_mods_folder(mods_folder);

        // Restore enabled state, order, and section_id for all mods
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

            // Restore section assignment
            if let Some(section_id) = sections_config.get_section_for_mod(&mod_folder_name) {
                mod_entry.set_section_id(section_id);
            }
        }

        // Sort mods by order before adding to model
        mods.sort_by_key(|m| m.order());

        // Create section headers from config
        let sections: Vec<SectionHeader> = sections_config.sections.iter()
            .map(|data| SectionHeader::from_data(data))
            .collect();

        // Populate collapsed_sections from config
        {
            let mut collapsed = self.collapsed_sections.borrow_mut();
            collapsed.clear();
            for section_data in &sections_config.sections {
                if !section_data.expanded {
                    collapsed.insert(section_data.section_id.clone());
                }
            }
        }

        // Build a combined list with sections and mods interleaved by order
        // Create sortable items (order, is_section_priority, object)
        // Sections come before mods at the same order position
        let model = self.model.borrow();
        if let Some(model) = model.as_ref() {
            model.remove_all();

            // Create a combined vec of (order, priority, object) where priority 0=section, 1=mod
            let mut items: Vec<(u32, u8, glib::Object)> = Vec::new();

            for section in sections {
                items.push((section.order(), 0, section.upcast()));
            }
            for mod_entry in mods {
                items.push((mod_entry.order(), 1, mod_entry.upcast()));
            }

            // Sort by order, then by priority (sections first at same position)
            items.sort_by_key(|(order, priority, _)| (*order, *priority));

            // Add to model
            for (_, _, obj) in items {
                model.append(&obj);
            }

            // Assign mods to sections based on position
            Self::update_section_assignments(model);
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

    /// Reload mods using stored paths (used after backup restore)
    pub fn reload(&self) {
        let mods_folder = self.mods_folder.borrow().clone();
        let game_mods_folder = self.game_mods_folder.borrow().clone();
        let profile_name = self.profile_name.borrow().clone();
        let mods_json_path = self.mods_json_path.borrow().clone();

        if let (Some(mods_folder), Some(game_mods_folder), Some(profile_name), Some(mods_json_path)) =
            (mods_folder, game_mods_folder, profile_name, mods_json_path)
        {
            self.load_mods(&mods_folder, &game_mods_folder, &profile_name, &mods_json_path);
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

    /// Find the position of any item (ModEntry or SectionHeader) in the underlying model
    fn find_item_position_in_model(model: &gio::ListStore, target: &glib::Object) -> Option<u32> {
        // Check if target is a ModEntry
        if let Some(mod_entry) = target.downcast_ref::<ModEntry>() {
            let target_path = mod_entry.path();
            for i in 0..model.n_items() {
                if let Some(item) = model.item(i) {
                    if let Some(entry) = item.downcast_ref::<ModEntry>() {
                        if entry.path() == target_path {
                            return Some(i);
                        }
                    }
                }
            }
        }
        // Check if target is a SectionHeader
        else if let Some(section) = target.downcast_ref::<SectionHeader>() {
            let target_id = section.section_id();
            for i in 0..model.n_items() {
                if let Some(item) = model.item(i) {
                    if let Some(sec) = item.downcast_ref::<SectionHeader>() {
                        if sec.section_id() == target_id {
                            return Some(i);
                        }
                    }
                }
            }
        }
        None
    }

    /// Find the position of an item in the selection model (which wraps the filtered model)
    fn find_item_position_in_selection(selection: &SingleSelection, target: &glib::Object) -> Option<u32> {
        let n_items = selection.n_items();

        // Check if target is a ModEntry
        if let Some(mod_entry) = target.downcast_ref::<ModEntry>() {
            let target_path = mod_entry.path();
            for i in 0..n_items {
                if let Some(item) = selection.item(i) {
                    if let Some(entry) = item.downcast_ref::<ModEntry>() {
                        if entry.path() == target_path {
                            return Some(i);
                        }
                    }
                }
            }
        }
        // Check if target is a SectionHeader
        else if let Some(section) = target.downcast_ref::<SectionHeader>() {
            let target_id = section.section_id();
            for i in 0..n_items {
                if let Some(item) = selection.item(i) {
                    if let Some(sec) = item.downcast_ref::<SectionHeader>() {
                        if sec.section_id() == target_id {
                            return Some(i);
                        }
                    }
                }
            }
        }
        None
    }

    /// Helper to get order value from any item (ModEntry or SectionHeader)
    fn get_item_order(item: &glib::Object) -> Option<u32> {
        if let Some(mod_entry) = item.downcast_ref::<ModEntry>() {
            Some(mod_entry.order())
        } else if let Some(section) = item.downcast_ref::<SectionHeader>() {
            Some(section.order())
        } else {
            None
        }
    }

    /// Helper to set order value on any item (ModEntry or SectionHeader)
    fn set_item_order(item: &glib::Object, order: u32) {
        if let Some(mod_entry) = item.downcast_ref::<ModEntry>() {
            mod_entry.set_order(order);
        } else if let Some(section) = item.downcast_ref::<SectionHeader>() {
            section.set_order(order);
        }
    }

    /// Rebuild model sorted by order (handles both ModEntry and SectionHeader)
    fn rebuild_model_sorted(model_store: &gio::ListStore) {
        let n_items = model_store.n_items();
        let mut items: Vec<(u32, u8, glib::Object)> = Vec::new();

        for i in 0..n_items {
            if let Some(item) = model_store.item(i) {
                if let Some(mod_entry) = item.downcast_ref::<ModEntry>() {
                    items.push((mod_entry.order(), 1, item)); // priority 1 for mods
                } else if let Some(section) = item.downcast_ref::<SectionHeader>() {
                    items.push((section.order(), 0, item)); // priority 0 for sections
                }
            }
        }

        // Sort by order, then by priority (sections first at same position)
        items.sort_by_key(|(order, priority, _)| (*order, *priority));

        model_store.remove_all();
        for (_, _, obj) in items {
            model_store.append(&obj);
        }

        // Update section assignments based on position
        Self::update_section_assignments(model_store);
    }

    /// Scan the list and assign each mod to the section header above it
    fn update_section_assignments(model_store: &gio::ListStore) {
        let n_items = model_store.n_items();
        let mut current_section_id: Option<String> = None;

        for i in 0..n_items {
            if let Some(item) = model_store.item(i) {
                if let Some(section) = item.downcast_ref::<SectionHeader>() {
                    // Update current section
                    current_section_id = Some(section.section_id());
                } else if let Some(mod_entry) = item.downcast_ref::<ModEntry>() {
                    // Assign mod to current section (or None if before any section)
                    // Use the property system directly for Option<String>
                    mod_entry.set_property("section-id", &current_section_id);
                }
            }
        }
    }

    /// Sync section data from model to config and save to disk
    fn sync_sections_to_config(
        model_store: &gio::ListStore,
        sections_config: &Rc<RefCell<SectionsConfig>>,
        profile_path: &Rc<RefCell<Option<PathBuf>>>,
    ) {
        let mut config = sections_config.borrow_mut();

        // Update all sections in config with current data from model
        for i in 0..model_store.n_items() {
            if let Some(item) = model_store.item(i) {
                if let Some(section) = item.downcast_ref::<SectionHeader>() {
                    // Update or add section in config
                    config.add_section(section.to_data());
                }
            }
        }

        // Save to disk
        drop(config);
        if let Some(path) = profile_path.borrow().as_ref() {
            let _ = sections_config.borrow().save(path);
        }
    }

    fn move_mod_up_static(
        model: &RefCell<Option<gio::ListStore>>,
        position: u32,
        _vfs: &RefCell<Option<VirtualFileSystem>>,
        profile_name: &Rc<RefCell<Option<String>>>,
        selection: &SingleSelection,
        sections_config: &Rc<RefCell<SectionsConfig>>,
        profile_path: &Rc<RefCell<Option<PathBuf>>>,
    ) {
        let model_borrow = model.borrow();
        if let Some(model_store) = model_borrow.as_ref() {
            if position == 0 || position >= model_store.n_items() {
                return; // Already at top or invalid
            }

            // Get current and previous items
            let current_item = match model_store.item(position) {
                Some(item) => item,
                None => return,
            };
            let prev_item = match model_store.item(position - 1) {
                Some(item) => item,
                None => return,
            };

            // Swap order values
            let current_order = Self::get_item_order(&current_item).unwrap_or(position);
            let prev_order = Self::get_item_order(&prev_item).unwrap_or(position - 1);

            Self::set_item_order(&current_item, prev_order);
            Self::set_item_order(&prev_item, current_order);

            // Rebuild model sorted
            Self::rebuild_model_sorted(model_store);

            // Restore selection - find the moved item's position in the filtered selection model
            if let Some(new_pos) = Self::find_item_position_in_selection(selection, &current_item) {
                selection.set_selected(new_pos);
            }

            // Sync section orders and save
            Self::sync_sections_to_config(model_store, sections_config, profile_path);

            drop(model_borrow);
            Self::save_mod_state_static(model, profile_name);
        }
    }

    /// Move a mod down in the list (static version for closures)
    fn move_mod_down_static(
        model: &RefCell<Option<gio::ListStore>>,
        position: u32,
        _vfs: &RefCell<Option<VirtualFileSystem>>,
        profile_name: &Rc<RefCell<Option<String>>>,
        selection: &SingleSelection,
        sections_config: &Rc<RefCell<SectionsConfig>>,
        profile_path: &Rc<RefCell<Option<PathBuf>>>,
    ) {
        let model_borrow = model.borrow();
        if let Some(model_store) = model_borrow.as_ref() {
            if position >= model_store.n_items() - 1 {
                return; // Already at bottom
            }

            // Get current and next items
            let current_item = match model_store.item(position) {
                Some(item) => item,
                None => return,
            };
            let next_item = match model_store.item(position + 1) {
                Some(item) => item,
                None => return,
            };

            // Swap order values
            let current_order = Self::get_item_order(&current_item).unwrap_or(position);
            let next_order = Self::get_item_order(&next_item).unwrap_or(position + 1);

            Self::set_item_order(&current_item, next_order);
            Self::set_item_order(&next_item, current_order);

            // Rebuild model sorted
            Self::rebuild_model_sorted(model_store);

            // Restore selection - find the moved item's position in the filtered selection model
            if let Some(new_pos) = Self::find_item_position_in_selection(selection, &current_item) {
                selection.set_selected(new_pos);
            }

            // Sync section orders and save
            Self::sync_sections_to_config(model_store, sections_config, profile_path);

            drop(model_borrow);
            Self::save_mod_state_static(model, profile_name);
        }
    }

    /// Move a mod to top of the list (static version for closures)
    fn move_mod_to_top_static(
        model: &RefCell<Option<gio::ListStore>>,
        position: u32,
        _vfs: &RefCell<Option<VirtualFileSystem>>,
        profile_name: &Rc<RefCell<Option<String>>>,
        selection: &SingleSelection,
        sections_config: &Rc<RefCell<SectionsConfig>>,
        profile_path: &Rc<RefCell<Option<PathBuf>>>,
    ) {
        let model_borrow = model.borrow();
        if let Some(model_store) = model_borrow.as_ref() {
            if position == 0 || position >= model_store.n_items() {
                return; // Already at top or invalid
            }

            let current_item = match model_store.item(position) {
                Some(item) => item,
                None => return,
            };

            let current_order = Self::get_item_order(&current_item).unwrap_or(position);

            // Set this item's order to 0
            Self::set_item_order(&current_item, 0);

            // Shift all items with order < current_order up by 1
            for i in 0..model_store.n_items() {
                if i == position {
                    continue;
                }
                if let Some(item) = model_store.item(i) {
                    if let Some(order) = Self::get_item_order(&item) {
                        if order < current_order {
                            Self::set_item_order(&item, order + 1);
                        }
                    }
                }
            }

            // Rebuild model sorted
            Self::rebuild_model_sorted(model_store);

            // Restore selection - find the moved item's position in the filtered selection model
            if let Some(new_pos) = Self::find_item_position_in_selection(selection, &current_item) {
                selection.set_selected(new_pos);
            }

            // Sync section orders and save
            Self::sync_sections_to_config(model_store, sections_config, profile_path);

            drop(model_borrow);
            Self::save_mod_state_static(model, profile_name);
        }
    }

    /// Move a mod to bottom of the list (static version for closures)
    fn move_mod_to_bottom_static(
        model: &RefCell<Option<gio::ListStore>>,
        position: u32,
        _vfs: &RefCell<Option<VirtualFileSystem>>,
        profile_name: &Rc<RefCell<Option<String>>>,
        selection: &SingleSelection,
        sections_config: &Rc<RefCell<SectionsConfig>>,
        profile_path: &Rc<RefCell<Option<PathBuf>>>,
    ) {
        let model_borrow = model.borrow();
        if let Some(model_store) = model_borrow.as_ref() {
            let n_items = model_store.n_items();
            if n_items == 0 || position >= n_items - 1 {
                return; // Already at bottom or invalid
            }

            let last_position = n_items - 1;

            let current_item = match model_store.item(position) {
                Some(item) => item,
                None => return,
            };

            let current_order = Self::get_item_order(&current_item).unwrap_or(position);

            // Set this item's order to last
            Self::set_item_order(&current_item, last_position);

            // Shift all items with order > current_order down by 1
            for i in 0..n_items {
                if i == position {
                    continue;
                }
                if let Some(item) = model_store.item(i) {
                    if let Some(order) = Self::get_item_order(&item) {
                        if order > current_order {
                            Self::set_item_order(&item, order - 1);
                        }
                    }
                }
            }

            // Rebuild model sorted
            Self::rebuild_model_sorted(model_store);

            // Restore selection - find the moved item's position in the filtered selection model
            if let Some(new_pos) = Self::find_item_position_in_selection(selection, &current_item) {
                selection.set_selected(new_pos);
            }

            // Sync section orders and save
            Self::sync_sections_to_config(model_store, sections_config, profile_path);

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

    /// Add a new section at the top of the list
    fn add_section_at_selection(
        model: &RefCell<Option<gio::ListStore>>,
        _selection: &SingleSelection,
        sections_config: &Rc<RefCell<SectionsConfig>>,
        profile_path: &Rc<RefCell<Option<PathBuf>>>,
        filter: &RefCell<Option<CustomFilter>>,
        column_view: &ColumnView,
    ) {
        let model_borrow = model.borrow();
        if let Some(model_store) = model_borrow.as_ref() {
            // Always add sections at the top
            let position = 0u32;

            // Create new section with default name
            let section = SectionHeader::new("New Section", position);

            // Insert into model at position
            model_store.insert(position, &section);

            // Update order of subsequent items (position + 1 is safe now since we bounded position)
            for i in (position + 1)..model_store.n_items() {
                if let Some(item) = model_store.item(i) {
                    if let Some(mod_entry) = item.downcast_ref::<ModEntry>() {
                        mod_entry.set_order(i);
                    } else if let Some(sec) = item.downcast_ref::<SectionHeader>() {
                        sec.set_order(i);
                    }
                }
            }

            // Update section assignments based on new positions
            Self::update_section_assignments(model_store);

            // Save section to config
            let section_data = section.to_data();
            sections_config.borrow_mut().add_section(section_data);

            if let Some(path) = profile_path.borrow().as_ref() {
                let _ = sections_config.borrow().save(path);
            }

            // Update filter to reflect changes
            if let Some(filter) = filter.borrow().as_ref() {
                filter.changed(FilterChange::Different);
            }

            // Scroll to top to show the new section
            use gtk4::prelude::ScrollableExt;
            if let Some(vadj) = column_view.vadjustment() {
                vadj.set_value(0.0);
            }
        }
    }

    /// Public API: Move a mod up
    pub fn move_mod_up(&self, mod_entry: &ModEntry) {
        if let Some(selection) = self.selection_model.borrow().as_ref() {
            if let Some(model) = self.model.borrow().as_ref() {
                let position = Self::find_mod_position(model, mod_entry);
                if position < model.n_items() {
                    Self::move_mod_up_static(&self.model, position, &self.vfs, &self.profile_name, selection, &self.sections_config, &self.profile_path);
                }
            }
        }
    }

    /// Public API: Move a mod down
    pub fn move_mod_down(&self, mod_entry: &ModEntry) {
        if let Some(selection) = self.selection_model.borrow().as_ref() {
            if let Some(model) = self.model.borrow().as_ref() {
                let position = Self::find_mod_position(model, mod_entry);
                if position < model.n_items() {
                    Self::move_mod_down_static(&self.model, position, &self.vfs, &self.profile_name, selection, &self.sections_config, &self.profile_path);
                }
            }
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
                        if let Ok(dfmod_infos) = crate::mod_entry::parse_dfmod_basic(&mod_entry.path()) {
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
                        if let Ok(dfmod_infos) = crate::mod_entry::parse_dfmod_basic(&mod_entry.path()) {
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

    /// Show backup/restore popover
    fn show_backup_popover(
        btn: &Button,
        profile_name: &Rc<RefCell<Option<String>>>,
        profile_path: &Rc<RefCell<Option<PathBuf>>>,
        widget: &super::ModListView,
    ) {
        let profile_name_opt = profile_name.borrow().clone();
        let Some(profile_name_str) = profile_name_opt else {
            eprintln!("No profile selected");
            return;
        };

        let profile_path_opt = profile_path.borrow().clone();
        let Some(profile_path_buf) = profile_path_opt else {
            eprintln!("Profile path not set");
            return;
        };

        // Create the main popover
        let popover = gtk4::Popover::new();
        let main_box = Box::new(Orientation::Vertical, 6);
        main_box.set_margin_top(6);
        main_box.set_margin_bottom(6);
        main_box.set_margin_start(6);
        main_box.set_margin_end(6);

        // Create Backup button
        let create_btn = Button::with_label("Create Backup");
        create_btn.add_css_class("flat");

        // Restore Backup button
        let restore_btn = Button::with_label("Restore Backup");
        restore_btn.add_css_class("flat");

        main_box.append(&create_btn);
        main_box.append(&restore_btn);

        // Create Backup handler
        let popover_for_create = popover.clone();
        let profile_name_for_create = profile_name_str.clone();
        let profile_path_for_create = profile_path_buf.clone();
        create_btn.connect_clicked(move |btn| {
            Self::show_create_backup_view(btn, &popover_for_create, &profile_name_for_create, &profile_path_for_create);
        });

        // Restore Backup handler
        let popover_for_restore = popover.clone();
        let profile_name_for_restore = profile_name_str.clone();
        let profile_path_for_restore = profile_path_buf.clone();
        let widget_for_restore = widget.clone();
        restore_btn.connect_clicked(move |btn| {
            Self::show_restore_backup_view(btn, &popover_for_restore, &profile_name_for_restore, &profile_path_for_restore, &widget_for_restore);
        });

        popover.set_child(Some(&main_box));
        popover.set_parent(btn);
        popover.popup();
    }

    /// Show the create backup UI
    fn show_create_backup_view(
        _btn: &Button,
        popover: &gtk4::Popover,
        profile_name: &str,
        profile_path: &PathBuf,
    ) {
        let Ok(backup_manager) = BackupManager::new(profile_name) else {
            eprintln!("Failed to create BackupManager");
            return;
        };

        // Replace popover content with create form
        let create_box = Box::new(Orientation::Vertical, 6);
        create_box.set_margin_top(6);
        create_box.set_margin_bottom(6);
        create_box.set_margin_start(6);
        create_box.set_margin_end(6);

        let label = Label::new(Some("Backup Name:"));
        label.set_xalign(0.0);
        create_box.append(&label);

        let entry = Entry::new();
        entry.set_text(&backup_manager.get_default_name());
        entry.set_width_chars(25);
        create_box.append(&entry);

        let button_row = Box::new(Orientation::Horizontal, 6);
        button_row.set_halign(gtk4::Align::End);

        let cancel_btn = Button::with_label("Cancel");
        cancel_btn.add_css_class("flat");
        let create_btn = Button::with_label("Create");
        create_btn.add_css_class("suggested-action");

        button_row.append(&cancel_btn);
        button_row.append(&create_btn);
        create_box.append(&button_row);

        // Cancel handler
        let popover_for_cancel = popover.clone();
        cancel_btn.connect_clicked(move |_| {
            popover_for_cancel.popdown();
        });

        // Create handler
        let popover_for_create = popover.clone();
        let profile_name_owned = profile_name.to_string();
        let profile_path_owned = profile_path.clone();
        let entry_clone = entry.clone();
        create_btn.connect_clicked(move |_| {
            let backup_name = entry_clone.text().to_string();
            if backup_name.is_empty() {
                return;
            }

            let Ok(manager) = BackupManager::new(&profile_name_owned) else {
                eprintln!("Failed to create BackupManager");
                return;
            };

            // mod_state.json is at profile level (parent of mods folder)
            let profile_dir = dirs::config_dir()
                .map(|d| d.join("vmod").join("profiles").join(&profile_name_owned));
            let Some(profile_dir) = profile_dir else {
                eprintln!("Could not find config directory");
                return;
            };
            let mod_state_path = profile_dir.join("mod_state.json");
            // sections.json is in the mods folder (profile_path)
            let sections_path = profile_path_owned.join("sections.json");

            match manager.create_backup(&backup_name, &mod_state_path, &sections_path) {
                Ok(backup_path) => {
                    eprintln!("Backup created at: {:?}", backup_path);
                    popover_for_create.popdown();
                }
                Err(e) => {
                    eprintln!("Failed to create backup: {}", e);
                }
            }
        });

        // Also allow Enter key to create
        let create_btn_for_activate = create_btn.clone();
        entry.connect_activate(move |_| {
            create_btn_for_activate.emit_clicked();
        });

        popover.set_child(Some(&create_box));
    }

    /// Show the restore backup UI
    fn show_restore_backup_view(
        _btn: &Button,
        popover: &gtk4::Popover,
        profile_name: &str,
        profile_path: &PathBuf,
        widget: &super::ModListView,
    ) {
        let Ok(backup_manager) = BackupManager::new(profile_name) else {
            eprintln!("Failed to create BackupManager");
            return;
        };

        let backups = backup_manager.list_backups().unwrap_or_default();

        // Replace popover content with restore list
        let restore_box = Box::new(Orientation::Vertical, 6);
        restore_box.set_margin_top(6);
        restore_box.set_margin_bottom(6);
        restore_box.set_margin_start(6);
        restore_box.set_margin_end(6);

        if backups.is_empty() {
            let label = Label::new(Some("No backups found"));
            label.add_css_class("dim-label");
            restore_box.append(&label);
        } else {
            let label = Label::new(Some("Select backup to restore:"));
            label.set_xalign(0.0);
            restore_box.append(&label);

            // Create a scrolled window for the list
            let scrolled = ScrolledWindow::new();
            scrolled.set_min_content_height(200);
            scrolled.set_min_content_width(250);

            let list_box = gtk4::ListBox::new();
            list_box.set_selection_mode(gtk4::SelectionMode::Single);
            list_box.add_css_class("boxed-list");

            // Store backup names for lookup by index
            let backup_names: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));

            // Add backups (limit to 10)
            for backup in backups.iter().take(10) {
                backup_names.borrow_mut().push(backup.name.clone());

                let row = gtk4::ListBoxRow::new();
                let row_box = Box::new(Orientation::Vertical, 2);
                row_box.set_margin_top(6);
                row_box.set_margin_bottom(6);
                row_box.set_margin_start(6);
                row_box.set_margin_end(6);

                let name_label = Label::new(Some(&backup.name));
                name_label.set_xalign(0.0);
                name_label.add_css_class("heading");

                // Format the date
                let date_str = if let Ok(duration) = backup.created_at.elapsed() {
                    let secs = duration.as_secs();
                    if secs < 60 {
                        "Just now".to_string()
                    } else if secs < 3600 {
                        format!("{} minutes ago", secs / 60)
                    } else if secs < 86400 {
                        format!("{} hours ago", secs / 3600)
                    } else {
                        format!("{} days ago", secs / 86400)
                    }
                } else {
                    "Unknown".to_string()
                };

                let date_label = Label::new(Some(&date_str));
                date_label.set_xalign(0.0);
                date_label.add_css_class("dim-label");

                row_box.append(&name_label);
                row_box.append(&date_label);
                row.set_child(Some(&row_box));

                list_box.append(&row);
            }

            scrolled.set_child(Some(&list_box));
            restore_box.append(&scrolled);

            // Connect row activation (double-click or Enter)
            let popover_for_restore = popover.clone();
            let profile_name_for_restore = profile_name.to_string();
            let profile_path_for_restore = profile_path.clone();
            let widget_for_restore = widget.clone();
            list_box.connect_row_activated(move |_, row| {
                let index = row.index();
                if index < 0 {
                    return;
                }

                let backup_name = backup_names.borrow().get(index as usize).cloned();
                if let Some(name) = backup_name {
                    let Ok(manager) = BackupManager::new(&profile_name_for_restore) else {
                        eprintln!("Failed to create BackupManager");
                        return;
                    };

                    // mod_state.json is at profile level (parent of mods folder)
                    let profile_dir = dirs::config_dir()
                        .map(|d| d.join("vmod").join("profiles").join(&profile_name_for_restore));
                    let Some(profile_dir) = profile_dir else {
                        eprintln!("Could not find config directory");
                        return;
                    };
                    let mod_state_dest = profile_dir.join("mod_state.json");
                    // sections.json is in the mods folder (profile_path)
                    let sections_dest = profile_path_for_restore.join("sections.json");

                    match manager.restore_backup(&name, &mod_state_dest, &sections_dest) {
                        Ok(()) => {
                            eprintln!("Backup '{}' restored successfully", name);
                            popover_for_restore.popdown();

                            // Reload the mod list to reflect restored state
                            widget_for_restore.reload();
                        }
                        Err(e) => {
                            eprintln!("Failed to restore backup: {}", e);
                        }
                    }
                }
            });
        }

        // Cancel button
        let cancel_btn = Button::with_label("Cancel");
        cancel_btn.add_css_class("flat");
        cancel_btn.set_halign(gtk4::Align::End);

        let popover_for_cancel = popover.clone();
        cancel_btn.connect_clicked(move |_| {
            popover_for_cancel.popdown();
        });

        restore_box.append(&cancel_btn);

        popover.set_child(Some(&restore_box));
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

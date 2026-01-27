//! UI construction logic for ModListView.

use gtk4::prelude::*;
use gtk4::subclass::prelude::*;
use gtk4::{
    gio, glib, Box, Button, ColumnView, CustomFilter, FilterChange, FilterListModel, Label,
    Orientation, Paned, PolicyType, ProgressBar, ScrolledWindow, SearchEntry, SingleSelection,
    UriLauncher,
};
use std::cell::RefCell;
use std::rc::Rc;

use crate::conflict_panel::ConflictPanel;
use crate::mod_entry::{ModEntry, SectionHeader};
use crate::mods_json_view::ModsJsonView;
use super::imp::ModListView;
use super::model_utils::find_item_position_in_model;

impl ModListView {
    /// Build the entire UI for the ModListView widget
    pub fn build_ui(&self) {
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

        // Backup button
        let backup_button = Button::with_label("Backup");
        backup_button.set_tooltip_text(Some("Create or restore mod list backups"));
        filter_row.append(&backup_button);

        // Refresh button (also triggers conflict scanning)
        let refresh_button = Button::with_label("Refresh");
        refresh_button.set_tooltip_text(Some("Rescan mod folders and detect conflicts"));
        self.refresh_button.replace(Some(refresh_button.clone()));
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
        self.add_checkbox_column(&column_view, &settings);
        self.add_name_column(&column_view, &settings);
        self.add_version_column(&column_view, &settings);
        self.add_order_column(&column_view, &settings);
        self.add_conflicts_column(&column_view, &settings);

        // Wrap in scrolled window
        let scrolled_window = ScrolledWindow::new();
        scrolled_window.set_vexpand(true);
        scrolled_window.set_hexpand(true);
        scrolled_window.set_policy(PolicyType::Never, PolicyType::Automatic);  // No horizontal scroll
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
                    model_borrow.as_ref().and_then(|m| find_item_position_in_model(m, &item))
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
                    model_borrow.as_ref().and_then(|m| find_item_position_in_model(m, &item))
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
                    model_borrow.as_ref().and_then(|m| find_item_position_in_model(m, &item))
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
                    model_borrow.as_ref().and_then(|m| find_item_position_in_model(m, &item))
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

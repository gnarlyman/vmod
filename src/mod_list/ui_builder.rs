//! UI construction logic for ModListView.

use gtk4::prelude::*;
use gtk4::subclass::prelude::*;
use gtk4::{
    gio, glib, Box, Button, ColumnView, CustomFilter, FilterChange, FilterListModel, Label,
    ListView, Orientation, Paned, PolicyType, ProgressBar, ScrolledWindow, SearchEntry,
    SignalListItemFactory, SingleSelection, UriLauncher,
};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use crate::conflict_panel::{ConflictPanel, DownloadItem};
use crate::mod_entry::{ModEntry, ModMetadata, SectionHeader, save_metadata};
use crate::mods_json_view::ModsJsonView;
use crate::nexus_api::{downloads_dir, DownloadMetadata};
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
                let search_lower = search.to_lowercase();
                return mod_entry.name().to_lowercase().contains(&search_lower)
                    || mod_entry.display_name().to_lowercase().contains(&search_lower);
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
        let column_view_clone = column_view.clone();
        top_button.connect_clicked(move |_| {
            // Get selected item and find its position in the underlying model (not filtered)
            if let Some(item) = selection_clone.selected_item() {
                let position = {
                    let model_borrow = model_clone.borrow();
                    model_borrow.as_ref().and_then(|m| find_item_position_in_model(m, &item))
                };
                if let Some(pos) = position {
                    // Save scroll position before move
                    let scroll_pos = column_view_clone.vadjustment().map(|adj| adj.value());
                    Self::move_mod_to_top_static(&model_clone, pos, &vfs_clone, &profile_clone, &selection_clone, &sections_config_clone, &profile_path_clone);
                    // Restore scroll position after GTK finishes processing
                    if let Some(scroll_val) = scroll_pos {
                        let cv = column_view_clone.clone();
                        glib::idle_add_local_once(move || {
                            if let Some(adj) = cv.vadjustment() {
                                adj.set_value(scroll_val);
                            }
                        });
                    }
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
        let column_view_clone = column_view.clone();
        bottom_button.connect_clicked(move |_| {
            // Get selected item and find its position in the underlying model (not filtered)
            if let Some(item) = selection_clone.selected_item() {
                let position = {
                    let model_borrow = model_clone.borrow();
                    model_borrow.as_ref().and_then(|m| find_item_position_in_model(m, &item))
                };
                if let Some(pos) = position {
                    // Save scroll position before move
                    let scroll_pos = column_view_clone.vadjustment().map(|adj| adj.value());
                    Self::move_mod_to_bottom_static(&model_clone, pos, &vfs_clone, &profile_clone, &selection_clone, &sections_config_clone, &profile_path_clone);
                    // Restore scroll position after GTK finishes processing
                    if let Some(scroll_val) = scroll_pos {
                        let cv = column_view_clone.clone();
                        glib::idle_add_local_once(move || {
                            if let Some(adj) = cv.vadjustment() {
                                adj.set_value(scroll_val);
                            }
                        });
                    }
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
                    log::error!("Failed to open folder: {}", e);
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
                                log::error!("Failed to open URL: {}", e);
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
        conflict_panel.set_margin_top(6);
        // Pass the shared dfmod cache for the DFMods tab
        conflict_panel.set_dfmod_cache(self.dfmod_cache.clone());
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

        // Create Downloads panel
        let downloads_box = Box::new(Orientation::Vertical, 6);
        downloads_box.set_margin_start(6);
        downloads_box.set_margin_end(12);
        downloads_box.set_margin_top(6);
        downloads_box.set_vexpand(false);  // Don't expand vertically

        let downloads_header = Label::new(Some("Downloads"));
        downloads_header.set_xalign(0.0);
        downloads_header.add_css_class("heading");
        downloads_box.append(&downloads_header);

        let downloads_scroll = ScrolledWindow::new();
        downloads_scroll.set_vexpand(false);
        downloads_scroll.set_hexpand(true);
        downloads_scroll.set_min_content_height(150);
        downloads_scroll.set_max_content_height(200);

        // Create downloads ListStore
        let downloads_store = gio::ListStore::new::<DownloadItem>();
        self.downloads_model.replace(Some(downloads_store.clone()));

        // Create downloads ListView
        let downloads_selection = SingleSelection::new(Some(downloads_store.clone()));
        downloads_selection.set_autoselect(false);
        downloads_selection.set_can_unselect(true);

        let downloads_list = ListView::new(Some(downloads_selection.clone()), None::<SignalListItemFactory>);
        downloads_list.set_show_separators(true);

        // Set up factory for downloads list
        let downloads_factory = SignalListItemFactory::new();
        downloads_factory.connect_setup(|_factory, item| {
            let list_item = item.downcast_ref::<gtk4::ListItem>().unwrap();
            let hbox = Box::new(Orientation::Horizontal, 8);
            hbox.set_margin_start(6);
            hbox.set_margin_end(6);
            hbox.set_margin_top(4);
            hbox.set_margin_bottom(4);

            let name_label = Label::new(None);
            name_label.set_xalign(0.0);
            name_label.set_hexpand(true);
            name_label.set_ellipsize(gtk4::pango::EllipsizeMode::Middle);

            let size_label = Label::new(None);
            size_label.add_css_class("dim-label");

            hbox.append(&name_label);
            hbox.append(&size_label);
            list_item.set_child(Some(&hbox));
        });

        downloads_factory.connect_bind(|_factory, item| {
            let list_item = item.downcast_ref::<gtk4::ListItem>().unwrap();
            if let Some(download_item) = list_item.item().and_downcast::<DownloadItem>() {
                if let Some(hbox) = list_item.child().and_downcast::<Box>() {
                    if let Some(name_label) = hbox.first_child().and_downcast::<Label>() {
                        name_label.set_text(&download_item.display_name());
                        name_label.set_tooltip_text(Some(&download_item.file_name()));
                    }
                    if let Some(size_label) = hbox.last_child().and_downcast::<Label>() {
                        size_label.set_text(&download_item.size_string());
                    }
                }
            }
        });

        downloads_list.set_factory(Some(&downloads_factory));
        downloads_scroll.set_child(Some(&downloads_list));
        downloads_box.append(&downloads_scroll);

        // Button bar for downloads
        let downloads_button_bar = Box::new(Orientation::Horizontal, 6);
        downloads_button_bar.set_margin_top(6);
        downloads_button_bar.set_margin_bottom(6);
        downloads_button_bar.set_hexpand(true);

        let downloads_refresh_button = Button::with_label("Refresh");

        let delete_button = Button::with_label("Delete");
        delete_button.set_sensitive(false);
        delete_button.add_css_class("destructive-action");

        let install_button = Button::with_label("Install");
        install_button.set_sensitive(false);
        install_button.add_css_class("suggested-action");

        // Spacer to push Delete/Install to the right
        let spacer = Box::new(Orientation::Horizontal, 0);
        spacer.set_hexpand(true);

        downloads_button_bar.append(&downloads_refresh_button);
        downloads_button_bar.append(&spacer);
        downloads_button_bar.append(&delete_button);
        downloads_button_bar.append(&install_button);
        downloads_box.append(&downloads_button_bar);

        // Connect selection change to enable/disable buttons
        let install_btn = install_button.clone();
        let delete_btn = delete_button.clone();
        downloads_selection.connect_selected_notify(move |selection| {
            let has_selection = selection.selected() != gtk4::INVALID_LIST_POSITION;
            install_btn.set_sensitive(has_selection);
            delete_btn.set_sensitive(has_selection);
        });

        // Connect Install button click
        let mods_folder_for_install = self.mods_folder.clone();
        let selection_clone = downloads_selection.clone();
        install_button.connect_clicked(move |_| {
            Self::on_install_clicked(&selection_clone, &mods_folder_for_install);
        });

        // Connect Delete button click
        let downloads_model_for_delete = self.downloads_model.clone();
        let selection_clone = downloads_selection.clone();
        delete_button.connect_clicked(move |_| {
            Self::on_delete_clicked(&selection_clone, &downloads_model_for_delete);
        });

        // Connect Refresh button click
        let downloads_model_for_refresh = self.downloads_model.clone();
        downloads_refresh_button.connect_clicked(move |_| {
            Self::refresh_downloads_static(&downloads_model_for_refresh);
        });

        // Create bottom paned: ConflictPanel (left) | Downloads (right)
        let bottom_paned = Paned::new(Orientation::Horizontal);
        bottom_paned.set_wide_handle(true);
        bottom_paned.set_vexpand(false);  // Don't expand vertically
        bottom_paned.set_start_child(Some(&conflict_panel));
        bottom_paned.set_end_child(Some(&downloads_box));
        bottom_paned.set_resize_start_child(true);
        bottom_paned.set_resize_end_child(true);
        bottom_paned.set_shrink_start_child(false);
        bottom_paned.set_shrink_end_child(false);

        // Load saved bottom paned position
        let bottom_saved_position = settings.int("bottom-paned-position");
        bottom_paned.set_position(bottom_saved_position);

        // Save bottom paned position when it changes
        let settings_for_bottom = settings.clone();
        bottom_paned.connect_position_notify(move |paned| {
            let position = paned.position();
            settings_for_bottom.set_int("bottom-paned-position", position).ok();
        });

        self.bottom_paned.replace(Some(bottom_paned.clone()));

        // Add bottom paned (conflict panel + downloads) between main paned and Apply button
        obj.append(&bottom_paned);

        // Load downloads immediately
        Self::refresh_downloads_static(&self.downloads_model);

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

    /// Scan the downloads directory and populate the downloads list
    fn refresh_downloads_static(downloads_model: &RefCell<Option<gio::ListStore>>) {
        if let Some(model) = downloads_model.borrow().as_ref() {
            model.remove_all();

            let Some(download_dir) = downloads_dir() else {
                log::warn!("Could not determine downloads directory");
                return;
            };

            if !download_dir.exists() {
                log::debug!("Downloads directory does not exist: {:?}", download_dir);
                return;
            }

            // Find all .zip files with corresponding .meta.json
            let mut items: Vec<(i64, DownloadItem)> = Vec::new();

            if let Ok(entries) = std::fs::read_dir(&download_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().map(|e| e == "zip").unwrap_or(false) {
                        let file_name = path.file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("")
                            .to_string();

                        // Try to read metadata file
                        let meta_path = download_dir.join(format!("{}.meta.json", file_name));
                        if let Ok(meta_contents) = std::fs::read_to_string(&meta_path) {
                            if let Ok(metadata) = serde_json::from_str::<DownloadMetadata>(&meta_contents) {
                                // Get actual file size from filesystem
                                let size = path.metadata().map(|m| m.len()).unwrap_or(0);

                                // Extract display name from source_url or use mod_name
                                let display_name = extract_display_name(&metadata);

                                let item = DownloadItem::new(
                                    &file_name,
                                    &display_name,
                                    metadata.mod_id,
                                    metadata.file_id,
                                    size,
                                    metadata.downloaded_at,
                                    path.to_str().unwrap_or(""),
                                    &metadata.game,
                                    metadata.version.as_deref().unwrap_or(""),
                                    metadata.mod_name.as_deref().unwrap_or(""),
                                    &metadata.source_url,
                                );

                                items.push((metadata.downloaded_at, item));
                            }
                        }
                    }
                }
            }

            // Sort by download time (newest first)
            items.sort_by(|a, b| b.0.cmp(&a.0));

            // Add to model
            for (_timestamp, item) in items {
                model.append(&item);
            }

            log::debug!("Found {} downloads", model.n_items());
        }
    }

    /// Handle Install button click
    fn on_install_clicked(
        selection: &SingleSelection,
        mods_folder: &Rc<RefCell<Option<PathBuf>>>,
    ) {
        let position = selection.selected();
        if position == gtk4::INVALID_LIST_POSITION {
            return;
        }

        let Some(item) = selection.selected_item().and_downcast::<DownloadItem>() else {
            return;
        };

        log::info!("Installing download: {}", item.display_name());

        // Get the profile's mods folder
        let mods_folder_ref = mods_folder.borrow();
        let Some(mods_folder) = mods_folder_ref.as_ref() else {
            log::warn!("No mods folder set for install - select a profile first");
            return;
        };

        // Determine folder name for the extracted mod
        let folder_name = determine_mod_folder_name(&item);
        let dest_folder = mods_folder.join(&folder_name);

        if dest_folder.exists() {
            log::warn!("Destination folder already exists: {:?}", dest_folder);
            // For now, we'll overwrite - could add a confirmation dialog later
        }

        // Extract the archive
        match extract_mod_archive(&item.path(), &dest_folder) {
            Ok(()) => {
                log::info!("Successfully installed mod to: {:?}", dest_folder);

                // Create vmod_meta.json if we have mod info from the download
                let mod_name = item.mod_name();
                let mod_id = item.mod_id();
                if !mod_name.is_empty() && mod_id > 0 {
                    let metadata = ModMetadata {
                        mod_name,
                        nexus_id: mod_id.to_string(),
                        version: {
                            let v = item.version();
                            if v.is_empty() { None } else { Some(v) }
                        },
                        file_id: {
                            let fid = item.file_id();
                            if fid > 0 { Some(fid) } else { None }
                        },
                        game_domain: {
                            let g = item.game();
                            if g.is_empty() { None } else { Some(g) }
                        },
                        fetched_at: Some(chrono::Utc::now().timestamp()),
                        version_status: 0,
                        latest_version: None,
                        version_checked_at: None,
                    };
                    if let Err(e) = save_metadata(&dest_folder, &metadata) {
                        log::warn!("Failed to save mod metadata: {}", e);
                    } else {
                        log::info!("Saved vmod_meta.json for '{}'", metadata.mod_name);
                    }
                }
            }
            Err(e) => {
                log::error!("Failed to extract mod archive: {}", e);
            }
        }
    }

    /// Handle Delete button click
    fn on_delete_clicked(
        selection: &SingleSelection,
        downloads_model: &RefCell<Option<gio::ListStore>>,
    ) {
        let position = selection.selected();
        if position == gtk4::INVALID_LIST_POSITION {
            return;
        }

        let Some(item) = selection.selected_item().and_downcast::<DownloadItem>() else {
            return;
        };

        log::info!("Deleting download: {}", item.display_name());

        let zip_path = item.path();
        let meta_path = PathBuf::from(format!("{}.meta.json", zip_path.display()));

        // Delete the files
        if let Err(e) = std::fs::remove_file(&zip_path) {
            log::error!("Failed to delete zip file: {}", e);
            return;
        }

        // Also delete metadata file if it exists
        if meta_path.exists() {
            if let Err(e) = std::fs::remove_file(&meta_path) {
                log::warn!("Failed to delete metadata file: {}", e);
            }
        }

        // Refresh the downloads list
        Self::refresh_downloads_static(downloads_model);
    }
}

/// Extract a display name from download metadata
fn extract_display_name(metadata: &DownloadMetadata) -> String {
    // Use the file_name directly, removing .zip extension
    let name = metadata.file_name.trim_end_matches(".zip");

    // Remove the timestamp suffix if present (last segment after dash if it's all digits and > 8 chars)
    if let Some(dash_pos) = name.rfind('-') {
        let suffix = &name[dash_pos + 1..];
        if suffix.chars().all(|c| c.is_ascii_digit()) && suffix.len() > 8 {
            return name[..dash_pos].to_string();
        }
    }

    name.to_string()
}

/// Determine the folder name for an extracted mod
/// Simply uses the archive filename without the .zip extension
fn determine_mod_folder_name(item: &DownloadItem) -> String {
    item.file_name().trim_end_matches(".zip").to_string()
}

/// Extract a mod archive to the destination folder
fn extract_mod_archive(zip_path: &PathBuf, dest_folder: &PathBuf) -> Result<(), String> {
    use std::fs::File;
    use std::io::BufReader;

    let file = File::open(zip_path)
        .map_err(|e| format!("Failed to open archive: {}", e))?;
    let reader = BufReader::new(file);

    let mut archive = zip::ZipArchive::new(reader)
        .map_err(|e| format!("Failed to read archive: {}", e))?;

    // Create destination folder
    std::fs::create_dir_all(dest_folder)
        .map_err(|e| format!("Failed to create destination folder: {}", e))?;

    // Extract all files
    archive.extract(dest_folder)
        .map_err(|e| format!("Failed to extract archive: {}", e))?;

    Ok(())
}

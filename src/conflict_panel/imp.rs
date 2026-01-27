use gtk4::prelude::*;
use gtk4::subclass::prelude::*;
use gtk4::{
    glib, gio, Box, Button, Label, Notebook, Orientation, Paned, ScrolledWindow,
    ListView, SignalListItemFactory, SingleSelection, TreeExpander, TreeListModel,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use crate::mod_entry::{TreeItem, get_children_at_path, ModConflictSummary, DfmodCacheKey, parse_dfmod_basic, extract_dfmod_assets_cached};
use crate::nexus_api::{downloads_dir, DownloadMetadata};
use crate::widgets::tree_filter::{TreeFilterState, TreeFilterWidget};
use super::download_item::DownloadItem;

/// Get children at a given prefix from flat asset paths.
///
/// Converts flat paths like ["Assets/Textures/foo.png", "Assets/Sound/bar.wav"]
/// into a hierarchical structure. At prefix "", returns [("Assets", "Assets", true)].
/// At prefix "Assets", returns [("Textures", "Assets/Textures", true), ("Sound", "Assets/Sound", true)].
///
/// Returns Vec of (name, full_path_to_component, is_dir).
fn get_dfmod_children_at_prefix(
    asset_paths: &[String],
    prefix: &str,
) -> Vec<(String, String, bool)> {
    use std::collections::HashSet;

    let mut seen: HashSet<String> = HashSet::new();
    let mut result = Vec::new();

    let prefix_with_slash = if prefix.is_empty() {
        String::new()
    } else {
        format!("{}/", prefix)
    };

    for path in asset_paths {
        // Only process paths that start with our prefix
        let suffix = if prefix.is_empty() {
            path.as_str()
        } else if let Some(s) = path.strip_prefix(&prefix_with_slash) {
            s
        } else {
            continue;
        };

        // Get the next component after the prefix
        if let Some(slash_pos) = suffix.find('/') {
            // This is a directory component
            let name = &suffix[..slash_pos];
            if seen.insert(name.to_string()) {
                let full_path = if prefix.is_empty() {
                    name.to_string()
                } else {
                    format!("{}/{}", prefix, name)
                };
                result.push((name.to_string(), full_path, true));
            }
        } else if !suffix.is_empty() {
            // This is a file at this level
            if seen.insert(suffix.to_string()) {
                let full_path = if prefix.is_empty() {
                    suffix.to_string()
                } else {
                    format!("{}/{}", prefix, suffix)
                };
                result.push((suffix.to_string(), full_path, false));
            }
        }
    }

    // Sort: directories first, then alphabetically
    result.sort_by(|a, b| {
        match (a.2, b.2) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.0.to_lowercase().cmp(&b.0.to_lowercase()),
        }
    });

    result
}

pub struct ConflictPanel {
    pub notebook: RefCell<Option<Notebook>>,
    pub conflicts_list: RefCell<Option<ListView>>,
    pub files_list: RefCell<Option<ListView>>,
    pub conflicts_model: RefCell<Option<gio::ListStore>>,
    pub files_model: RefCell<Option<gio::ListStore>>,
    // Wrapped in Rc so closure can share the reference
    pub current_mod_path: Rc<RefCell<Option<PathBuf>>>,
    // Store conflict data for the tree model callback
    pub conflict_data: Rc<RefCell<HashMap<String, Vec<String>>>>,
    // Store dfmod asset paths for the tree model callback (dfmod filename -> asset paths)
    pub dfmod_assets: Rc<RefCell<HashMap<String, Vec<String>>>>,
    // Filter state for the Files tab
    pub files_filter_state: Rc<RefCell<TreeFilterState>>,
    // Filter widget reference
    pub filter_widget: RefCell<Option<TreeFilterWidget>>,
    // Reference to files_box for rebuilding tree model
    pub files_box: RefCell<Option<Box>>,
    // Files scroll window for rebuilding
    pub files_scroll: RefCell<Option<ScrolledWindow>>,
    // Shared dfmod cache reference from ModListView
    pub shared_dfmod_cache: RefCell<Option<Arc<Mutex<HashMap<DfmodCacheKey, Vec<String>>>>>>,
    // DFMods tab components
    pub dfmods_list: RefCell<Option<ListView>>,
    pub dfmods_model: RefCell<Option<gio::ListStore>>,
    pub dfmods_filter_state: Rc<RefCell<TreeFilterState>>,
    pub dfmods_filter_widget: RefCell<Option<TreeFilterWidget>>,
    pub dfmods_box: RefCell<Option<Box>>,
    pub dfmods_scroll: RefCell<Option<ScrolledWindow>>,
    // Downloads pane components
    pub downloads_list: RefCell<Option<ListView>>,
    pub downloads_model: RefCell<Option<gio::ListStore>>,
    pub downloads_paned: RefCell<Option<Paned>>,
    pub install_button: RefCell<Option<Button>>,
    pub delete_button: RefCell<Option<Button>>,
}

impl Default for ConflictPanel {
    fn default() -> Self {
        Self {
            notebook: RefCell::new(None),
            conflicts_list: RefCell::new(None),
            files_list: RefCell::new(None),
            conflicts_model: RefCell::new(None),
            files_model: RefCell::new(None),
            current_mod_path: Rc::new(RefCell::new(None)),
            conflict_data: Rc::new(RefCell::new(HashMap::new())),
            dfmod_assets: Rc::new(RefCell::new(HashMap::new())),
            files_filter_state: Rc::new(RefCell::new(TreeFilterState::new())),
            filter_widget: RefCell::new(None),
            files_box: RefCell::new(None),
            files_scroll: RefCell::new(None),
            shared_dfmod_cache: RefCell::new(None),
            dfmods_list: RefCell::new(None),
            dfmods_model: RefCell::new(None),
            dfmods_filter_state: Rc::new(RefCell::new(TreeFilterState::new())),
            dfmods_filter_widget: RefCell::new(None),
            dfmods_box: RefCell::new(None),
            dfmods_scroll: RefCell::new(None),
            downloads_list: RefCell::new(None),
            downloads_model: RefCell::new(None),
            downloads_paned: RefCell::new(None),
            install_button: RefCell::new(None),
            delete_button: RefCell::new(None),
        }
    }
}

#[glib::object_subclass]
impl ObjectSubclass for ConflictPanel {
    const NAME: &'static str = "ConflictPanel";
    type Type = super::ConflictPanel;
    type ParentType = Box;
}

impl ObjectImpl for ConflictPanel {
    fn constructed(&self) {
        self.parent_constructed();

        let obj = self.obj();
        obj.set_orientation(Orientation::Vertical);
        obj.set_spacing(0);

        // Create notebook with two tabs
        let notebook = Notebook::new();
        notebook.set_vexpand(false);
        notebook.set_hexpand(true);

        // Conflicts tab
        let conflicts_box = Box::new(Orientation::Vertical, 0);
        let conflicts_scroll = ScrolledWindow::new();
        conflicts_scroll.set_vexpand(true);
        conflicts_scroll.set_hexpand(true);
        conflicts_scroll.set_min_content_height(150);
        conflicts_scroll.set_max_content_height(200);

        // Create conflicts ListStore and TreeListModel
        let conflicts_store = gio::ListStore::new::<TreeItem>();
        self.conflicts_model.replace(Some(conflicts_store.clone()));

        // Clone the conflict_data reference for the closure
        let conflict_data_ref = self.conflict_data.clone();

        let conflicts_tree_model = TreeListModel::new(
            conflicts_store.clone(),
            false, // passthrough
            false, // autoexpand - start collapsed so user can see structure
            move |item| {
                let tree_item = item.downcast_ref::<TreeItem>().unwrap();
                if tree_item.is_expandable() {
                    // Look up children from stored conflict data
                    let mod_name = tree_item.display_name();
                    let data = conflict_data_ref.borrow();
                    if let Some(files) = data.get(&mod_name) {
                        let children = gio::ListStore::new::<TreeItem>();
                        for file_path in files {
                            let file_item = TreeItem::new_file(file_path, file_path);
                            children.append(&file_item);
                        }
                        Some(children.upcast())
                    } else {
                        None
                    }
                } else {
                    None
                }
            },
        );

        let conflicts_selection = SingleSelection::new(Some(conflicts_tree_model.clone()));
        conflicts_selection.set_autoselect(false);
        conflicts_selection.set_can_unselect(true);

        let conflicts_list = ListView::new(Some(conflicts_selection), None::<SignalListItemFactory>);
        conflicts_list.set_show_separators(true);

        // Set up factory for conflicts list
        let factory = SignalListItemFactory::new();
        factory.connect_setup(|_factory, item| {
            let list_item = item.downcast_ref::<gtk4::ListItem>().unwrap();

            let expander = TreeExpander::new();
            let label = Label::new(None);
            label.set_xalign(0.0);
            label.set_hexpand(true);
            expander.set_child(Some(&label));

            list_item.set_child(Some(&expander));
        });

        factory.connect_bind(|_factory, item| {
            let list_item = item.downcast_ref::<gtk4::ListItem>().unwrap();
            let row = list_item.item().and_downcast::<gtk4::TreeListRow>();

            if let Some(row) = row {
                let expander = list_item.child().and_downcast::<TreeExpander>().unwrap();
                expander.set_list_row(Some(&row));

                if let Some(tree_item) = row.item().and_downcast::<TreeItem>() {
                    let label = expander.child().and_downcast::<Label>().unwrap();

                    let item_type = tree_item.item_type();
                    let display = tree_item.display_name();
                    let conflict_count = tree_item.conflict_count();

                    let text = if item_type == 0 && conflict_count > 0 {
                        // Mod root with conflict count
                        format!("{} ({} files)", display, conflict_count)
                    } else {
                        display
                    };

                    label.set_text(&text);

                    // Add CSS class based on type
                    label.remove_css_class("dim-label");
                    label.remove_css_class("error");
                    if item_type == 0 {
                        label.add_css_class("heading");
                    } else if item_type == 2 {
                        label.add_css_class("dim-label");
                    }
                }
            }
        });

        conflicts_list.set_factory(Some(&factory));
        conflicts_scroll.set_child(Some(&conflicts_list));
        conflicts_box.append(&conflicts_scroll);

        self.conflicts_list.replace(Some(conflicts_list));

        // Files tab with Paned layout (Files left, Downloads right)
        let files_paned = Paned::new(Orientation::Horizontal);
        files_paned.set_wide_handle(true);

        // Left side: Files content
        let files_box = Box::new(Orientation::Vertical, 6);
        files_box.set_margin_start(6);
        files_box.set_margin_end(6);
        files_box.set_margin_top(6);

        // Add filter widget at top of Files section
        let filter_widget = TreeFilterWidget::new();
        filter_widget.set_placeholder_text(Some("Filter files..."));
        files_box.append(&filter_widget);

        // Connect filter changes
        let obj_clone = obj.clone();
        filter_widget.connect_filter_changed(move |text, show_subtrees| {
            obj_clone.imp().on_filter_changed(text, show_subtrees);
        });

        self.filter_widget.replace(Some(filter_widget));

        let files_scroll = ScrolledWindow::new();
        files_scroll.set_vexpand(true);
        files_scroll.set_hexpand(true);
        files_scroll.set_min_content_height(150);
        files_scroll.set_max_content_height(200);

        // Create files ListStore
        let files_store = gio::ListStore::new::<TreeItem>();
        self.files_model.replace(Some(files_store.clone()));

        // Create initial tree model and list view
        let files_list = self.create_files_list_view(files_store.clone());
        files_scroll.set_child(Some(&files_list));
        files_box.append(&files_scroll);

        self.files_list.replace(Some(files_list));
        self.files_box.replace(Some(files_box.clone()));
        self.files_scroll.replace(Some(files_scroll));

        // Right side: Downloads content
        let downloads_box = Box::new(Orientation::Vertical, 6);
        downloads_box.set_margin_start(6);
        downloads_box.set_margin_end(6);
        downloads_box.set_margin_top(6);

        let downloads_header = Label::new(Some("Downloads"));
        downloads_header.set_xalign(0.0);
        downloads_header.add_css_class("heading");
        downloads_box.append(&downloads_header);

        let downloads_scroll = ScrolledWindow::new();
        downloads_scroll.set_vexpand(true);
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

        // Button bar
        let button_bar = Box::new(Orientation::Horizontal, 6);
        button_bar.set_margin_top(6);
        button_bar.set_margin_bottom(6);
        button_bar.set_halign(gtk4::Align::End);

        let delete_button = Button::with_label("Delete");
        delete_button.set_sensitive(false);
        delete_button.add_css_class("destructive-action");

        let install_button = Button::with_label("Install");
        install_button.set_sensitive(false);
        install_button.add_css_class("suggested-action");

        button_bar.append(&delete_button);
        button_bar.append(&install_button);
        downloads_box.append(&button_bar);

        // Connect selection change to enable/disable buttons
        let install_btn = install_button.clone();
        let delete_btn = delete_button.clone();
        downloads_selection.connect_selected_notify(move |selection| {
            let has_selection = selection.selected() != gtk4::INVALID_LIST_POSITION;
            install_btn.set_sensitive(has_selection);
            delete_btn.set_sensitive(has_selection);
        });

        // Connect Install button click
        let obj_clone = obj.clone();
        let selection_clone = downloads_selection.clone();
        install_button.connect_clicked(move |_| {
            obj_clone.imp().on_install_clicked(&selection_clone);
        });

        // Connect Delete button click
        let obj_clone = obj.clone();
        let selection_clone = downloads_selection.clone();
        delete_button.connect_clicked(move |_| {
            obj_clone.imp().on_delete_clicked(&selection_clone);
        });

        self.downloads_list.replace(Some(downloads_list));
        self.install_button.replace(Some(install_button));
        self.delete_button.replace(Some(delete_button));

        // Set up paned with both sides
        files_paned.set_start_child(Some(&files_box));
        files_paned.set_end_child(Some(&downloads_box));
        files_paned.set_resize_start_child(true);
        files_paned.set_resize_end_child(true);
        files_paned.set_shrink_start_child(false);
        files_paned.set_shrink_end_child(false);

        self.downloads_paned.replace(Some(files_paned.clone()));

        // DFMods tab
        let dfmods_box = Box::new(Orientation::Vertical, 6);
        dfmods_box.set_margin_start(6);
        dfmods_box.set_margin_end(6);
        dfmods_box.set_margin_top(6);

        // Add filter widget at top of DFMods tab
        let dfmods_filter_widget = TreeFilterWidget::new();
        dfmods_filter_widget.set_placeholder_text(Some("Filter assets..."));
        dfmods_box.append(&dfmods_filter_widget);

        // Connect filter changes
        let obj_clone = obj.clone();
        dfmods_filter_widget.connect_filter_changed(move |text, show_subtrees| {
            obj_clone.imp().on_dfmods_filter_changed(text, show_subtrees);
        });

        self.dfmods_filter_widget.replace(Some(dfmods_filter_widget));

        let dfmods_scroll = ScrolledWindow::new();
        dfmods_scroll.set_vexpand(true);
        dfmods_scroll.set_hexpand(true);
        dfmods_scroll.set_min_content_height(150);
        dfmods_scroll.set_max_content_height(200);

        // Create dfmods ListStore
        let dfmods_store = gio::ListStore::new::<TreeItem>();
        self.dfmods_model.replace(Some(dfmods_store.clone()));

        // Create initial tree model and list view
        let dfmods_list = self.create_dfmods_list_view(dfmods_store.clone());
        dfmods_scroll.set_child(Some(&dfmods_list));
        dfmods_box.append(&dfmods_scroll);

        self.dfmods_list.replace(Some(dfmods_list));
        self.dfmods_box.replace(Some(dfmods_box.clone()));
        self.dfmods_scroll.replace(Some(dfmods_scroll));

        // Add tabs to notebook (Files is default/first tab)
        let conflicts_label = Label::new(Some("Conflicts"));
        let files_label = Label::new(Some("Files"));
        let dfmods_label = Label::new(Some("DFMods"));

        notebook.append_page(&files_paned, Some(&files_label));
        notebook.append_page(&dfmods_box, Some(&dfmods_label));
        notebook.append_page(&conflicts_box, Some(&conflicts_label));

        self.notebook.replace(Some(notebook.clone()));

        obj.append(&notebook);
    }
}

impl WidgetImpl for ConflictPanel {}
impl BoxImpl for ConflictPanel {}

impl ConflictPanel {
    /// Create the files ListView with TreeListModel
    fn create_files_list_view(&self, files_store: gio::ListStore) -> ListView {
        // Store the mod path for child model creation
        let mod_path_ref = self.current_mod_path.clone();
        // Store dfmod assets reference for the callback
        let dfmod_assets_ref = self.dfmod_assets.clone();
        // Store filter state for child filtering
        let filter_state_ref = self.files_filter_state.clone();

        let files_tree_model = TreeListModel::new(
            files_store,
            false, // passthrough
            false, // autoexpand (start collapsed)
            move |item| {
                let tree_item = item.downcast_ref::<TreeItem>().unwrap();
                if tree_item.is_expandable() {
                    let item_type = tree_item.item_type();
                    let filter = filter_state_ref.borrow();

                    // Handle dfmod archives (type 3) - return asset paths
                    if item_type == 3 {
                        // full_path stores the dfmod file_name (lookup key)
                        let dfmod_key = tree_item.full_path();
                        let assets = dfmod_assets_ref.borrow();
                        if let Some(asset_paths) = assets.get(&dfmod_key) {
                            let children_store = gio::ListStore::new::<TreeItem>();
                            for asset_path in asset_paths {
                                // Apply filter if active
                                if filter.is_active() && !filter.is_visible(asset_path) {
                                    continue;
                                }
                                let child = TreeItem::new_file(asset_path, asset_path);
                                // Set filter match state
                                child.set_matches_filter(filter.matches(asset_path));
                                child.set_visible_in_filter(true);
                                children_store.append(&child);
                            }
                            if children_store.n_items() > 0 {
                                return Some(children_store.upcast());
                            }
                        }
                        return None;
                    }

                    // Handle folders (type 1) - get filesystem children
                    let relative_path = tree_item.full_path();

                    // Get mod path from stored reference
                    if let Some(mod_path) = mod_path_ref.borrow().as_ref() {
                        let children_data = get_children_at_path(mod_path, &relative_path);

                        if !children_data.is_empty() {
                            let children_store = gio::ListStore::new::<TreeItem>();
                            for (name, rel_path, is_dir) in children_data {
                                // Apply filter if active
                                if filter.is_active() && !filter.is_visible(&rel_path) {
                                    continue;
                                }
                                let child = if is_dir {
                                    TreeItem::new_folder(&name, &rel_path)
                                } else {
                                    TreeItem::new_file(&name, &rel_path)
                                };
                                // Set filter match state
                                child.set_matches_filter(filter.matches(&name));
                                child.set_visible_in_filter(true);
                                children_store.append(&child);
                            }
                            if children_store.n_items() > 0 {
                                return Some(children_store.upcast());
                            }
                        }
                    }
                    None
                } else {
                    None
                }
            },
        );

        let files_selection = SingleSelection::new(Some(files_tree_model));
        files_selection.set_autoselect(false);
        files_selection.set_can_unselect(true);

        let files_list = ListView::new(Some(files_selection), None::<SignalListItemFactory>);
        files_list.set_show_separators(true);

        // Set up factory for files list
        let files_factory = SignalListItemFactory::new();
        files_factory.connect_setup(|_factory, item| {
            let list_item = item.downcast_ref::<gtk4::ListItem>().unwrap();

            let expander = TreeExpander::new();
            let label = Label::new(None);
            label.set_xalign(0.0);
            label.set_hexpand(true);
            expander.set_child(Some(&label));

            list_item.set_child(Some(&expander));
        });

        files_factory.connect_bind(|_factory, item| {
            let list_item = item.downcast_ref::<gtk4::ListItem>().unwrap();
            let row = list_item.item().and_downcast::<gtk4::TreeListRow>();

            if let Some(row) = row {
                let expander = list_item.child().and_downcast::<TreeExpander>().unwrap();
                expander.set_list_row(Some(&row));

                if let Some(tree_item) = row.item().and_downcast::<TreeItem>() {
                    let label = expander.child().and_downcast::<Label>().unwrap();

                    let item_type = tree_item.item_type();
                    let display = tree_item.display_name();
                    let asset_count = tree_item.conflict_count();

                    // Format text based on type
                    let text = if item_type == 3 && asset_count > 0 {
                        // Dfmod archive with asset count
                        format!("{} ({} assets)", display, asset_count)
                    } else {
                        display
                    };
                    label.set_text(&text);

                    // Style based on type
                    label.remove_css_class("heading");
                    label.remove_css_class("dim-label");
                    label.remove_css_class("accent");
                    label.remove_css_class("filter-match");

                    if item_type == 1 {
                        // Folder
                        label.add_css_class("heading");
                    } else if item_type == 2 {
                        // File
                        label.add_css_class("dim-label");
                    } else if item_type == 3 {
                        // Dfmod archive - use accent style
                        label.add_css_class("accent");
                    }

                    // Highlight matching items
                    if tree_item.matches_filter() {
                        label.add_css_class("filter-match");
                    }
                }
            }
        });

        files_list.set_factory(Some(&files_factory));
        files_list
    }

    /// Create the dfmods ListView with TreeListModel for displaying dfmod contents
    fn create_dfmods_list_view(&self, dfmods_store: gio::ListStore) -> ListView {
        // Store dfmod assets reference for the callback
        let dfmod_assets_ref = self.dfmod_assets.clone();
        // Store filter state for child filtering
        let filter_state_ref = self.dfmods_filter_state.clone();

        let dfmods_tree_model = TreeListModel::new(
            dfmods_store,
            false, // passthrough
            false, // autoexpand (start collapsed)
            move |item| {
                let tree_item = item.downcast_ref::<TreeItem>().unwrap();
                if tree_item.is_expandable() {
                    let item_type = tree_item.item_type();
                    let filter = filter_state_ref.borrow();

                    // Handle dfmod roots (type 3) - return top-level asset paths/folders
                    if item_type == 3 {
                        let dfmod_key = tree_item.full_path();
                        let assets = dfmod_assets_ref.borrow();
                        if let Some(asset_paths) = assets.get(&dfmod_key) {
                            // Get top-level children at root prefix
                            let children_data = get_dfmod_children_at_prefix(asset_paths, "");
                            if !children_data.is_empty() {
                                let children_store = gio::ListStore::new::<TreeItem>();
                                for (name, full_path, is_dir) in children_data {
                                    // Apply filter if active
                                    if filter.is_active() && !filter.is_visible(&full_path) {
                                        continue;
                                    }
                                    // Store dfmod_key::path in full_path for folder lookups
                                    let lookup_path = format!("{}::{}", dfmod_key, full_path);
                                    let child = if is_dir {
                                        TreeItem::new_folder(&name, &lookup_path)
                                    } else {
                                        TreeItem::new_file(&name, &full_path)
                                    };
                                    child.set_matches_filter(filter.matches(&name));
                                    child.set_visible_in_filter(true);
                                    children_store.append(&child);
                                }
                                if children_store.n_items() > 0 {
                                    return Some(children_store.upcast());
                                }
                            }
                        }
                        return None;
                    }

                    // Handle folders (type 1) - parse dfmod_key::prefix from full_path
                    if item_type == 1 {
                        let lookup_path = tree_item.full_path();
                        // Parse "dfmod_key::folder/path" format
                        if let Some(sep_pos) = lookup_path.find("::") {
                            let dfmod_key = &lookup_path[..sep_pos];
                            let prefix = &lookup_path[sep_pos + 2..];

                            let assets = dfmod_assets_ref.borrow();
                            if let Some(asset_paths) = assets.get(dfmod_key) {
                                let children_data = get_dfmod_children_at_prefix(asset_paths, prefix);
                                if !children_data.is_empty() {
                                    let children_store = gio::ListStore::new::<TreeItem>();
                                    for (name, full_path, is_dir) in children_data {
                                        // Apply filter if active
                                        if filter.is_active() && !filter.is_visible(&full_path) {
                                            continue;
                                        }
                                        // Store dfmod_key::path for nested folder lookups
                                        let child_lookup = format!("{}::{}", dfmod_key, full_path);
                                        let child = if is_dir {
                                            TreeItem::new_folder(&name, &child_lookup)
                                        } else {
                                            TreeItem::new_file(&name, &full_path)
                                        };
                                        child.set_matches_filter(filter.matches(&name));
                                        child.set_visible_in_filter(true);
                                        children_store.append(&child);
                                    }
                                    if children_store.n_items() > 0 {
                                        return Some(children_store.upcast());
                                    }
                                }
                            }
                        }
                    }
                    None
                } else {
                    None
                }
            },
        );

        let dfmods_selection = SingleSelection::new(Some(dfmods_tree_model));
        dfmods_selection.set_autoselect(false);
        dfmods_selection.set_can_unselect(true);

        let dfmods_list = ListView::new(Some(dfmods_selection), None::<SignalListItemFactory>);
        dfmods_list.set_show_separators(true);

        // Set up factory for dfmods list
        let dfmods_factory = SignalListItemFactory::new();
        dfmods_factory.connect_setup(|_factory, item| {
            let list_item = item.downcast_ref::<gtk4::ListItem>().unwrap();

            let expander = TreeExpander::new();
            let label = Label::new(None);
            label.set_xalign(0.0);
            label.set_hexpand(true);
            expander.set_child(Some(&label));

            list_item.set_child(Some(&expander));
        });

        dfmods_factory.connect_bind(|_factory, item| {
            let list_item = item.downcast_ref::<gtk4::ListItem>().unwrap();
            let row = list_item.item().and_downcast::<gtk4::TreeListRow>();

            if let Some(row) = row {
                let expander = list_item.child().and_downcast::<TreeExpander>().unwrap();
                expander.set_list_row(Some(&row));

                if let Some(tree_item) = row.item().and_downcast::<TreeItem>() {
                    let label = expander.child().and_downcast::<Label>().unwrap();

                    let item_type = tree_item.item_type();
                    let display = tree_item.display_name();
                    let asset_count = tree_item.conflict_count();

                    // Format text based on type
                    let text = if item_type == 3 && asset_count > 0 {
                        // Dfmod root with asset count
                        format!("{} ({} assets)", display, asset_count)
                    } else {
                        display
                    };
                    label.set_text(&text);

                    // Style based on type
                    label.remove_css_class("heading");
                    label.remove_css_class("dim-label");
                    label.remove_css_class("accent");
                    label.remove_css_class("filter-match");

                    if item_type == 1 {
                        // Folder
                        label.add_css_class("heading");
                    } else if item_type == 2 {
                        // File
                        label.add_css_class("dim-label");
                    } else if item_type == 3 {
                        // Dfmod archive root - use accent style
                        label.add_css_class("accent");
                    }

                    // Highlight matching items
                    if tree_item.matches_filter() {
                        label.add_css_class("filter-match");
                    }
                }
            }
        });

        dfmods_list.set_factory(Some(&dfmods_factory));
        dfmods_list
    }

    /// Handle filter changes from the filter widget
    fn on_filter_changed(&self, text: &str, show_subtrees: bool) {
        // Update filter state
        {
            let mut filter = self.files_filter_state.borrow_mut();
            filter.set_search(text, show_subtrees);

            // Pre-compute visibility for all known paths
            if filter.is_active() {
                let mut all_paths = Vec::new();

                // Collect filesystem paths if we have a mod path
                if let Some(mod_path) = self.current_mod_path.borrow().as_ref() {
                    self.collect_all_paths(mod_path, "", &mut all_paths);
                }

                // Collect dfmod asset paths
                for paths in self.dfmod_assets.borrow().values() {
                    all_paths.extend(paths.iter().cloned());
                }

                filter.compute_visibility(all_paths.iter().map(|s| s.as_str()));

                // Update match count in filter widget
                if let Some(widget) = self.filter_widget.borrow().as_ref() {
                    widget.set_match_count(filter.match_count());
                }
            }
        }

        // Rebuild the tree model to apply the filter
        self.rebuild_files_tree();
    }

    /// Recursively collect all file paths in a mod directory
    fn collect_all_paths(&self, mod_path: &PathBuf, relative_path: &str, paths: &mut Vec<String>) {
        let children = get_children_at_path(mod_path, relative_path);

        for (_name, rel_path, is_dir) in children {
            paths.push(rel_path.clone());
            if is_dir {
                self.collect_all_paths(mod_path, &rel_path, paths);
            }
        }
    }

    /// Rebuild the files tree model to apply filter changes
    fn rebuild_files_tree(&self) {
        if let Some(model) = self.files_model.borrow().as_ref() {
            if let Some(mod_path) = self.current_mod_path.borrow().as_ref() {
                // Rebuild root items
                model.remove_all();

                let filter = self.files_filter_state.borrow();
                let children = get_children_at_path(mod_path, "");

                if children.is_empty() {
                    let empty = TreeItem::new("No files found", "", false, 2);
                    model.append(&empty);
                } else {
                    for (name, rel_path, is_dir) in children {
                        // Apply filter if active
                        if filter.is_active() && !filter.is_visible(&rel_path) {
                            continue;
                        }
                        let item = if is_dir {
                            TreeItem::new_folder(&name, &rel_path)
                        } else {
                            TreeItem::new_file(&name, &rel_path)
                        };
                        // Set filter match state
                        item.set_matches_filter(filter.matches(&name));
                        item.set_visible_in_filter(true);
                        model.append(&item);
                    }

                    // If filter is active but nothing visible, show a message
                    if filter.is_active() && model.n_items() == 0 {
                        let no_matches = TreeItem::new("No matches found", "", false, 2);
                        model.append(&no_matches);
                    }
                }
            }
        }

        // Recreate the tree list view with the new model to refresh child closures
        if let Some(files_store) = self.files_model.borrow().clone() {
            let new_files_list = self.create_files_list_view(files_store);

            if let Some(scroll) = self.files_scroll.borrow().as_ref() {
                scroll.set_child(Some(&new_files_list));
            }

            self.files_list.replace(Some(new_files_list));
        }
    }

    /// Handle filter changes from the dfmods filter widget
    fn on_dfmods_filter_changed(&self, text: &str, show_subtrees: bool) {
        // Update filter state
        {
            let mut filter = self.dfmods_filter_state.borrow_mut();
            filter.set_search(text, show_subtrees);

            // Pre-compute visibility for all dfmod asset paths
            if filter.is_active() {
                let mut all_paths = Vec::new();

                // Collect all dfmod asset paths
                for paths in self.dfmod_assets.borrow().values() {
                    all_paths.extend(paths.iter().cloned());
                }

                filter.compute_visibility(all_paths.iter().map(|s| s.as_str()));

                // Update match count in filter widget
                if let Some(widget) = self.dfmods_filter_widget.borrow().as_ref() {
                    widget.set_match_count(filter.match_count());
                }
            }
        }

        // Rebuild the tree model to apply the filter
        self.rebuild_dfmods_tree();
    }

    /// Rebuild the dfmods tree model to apply filter changes
    fn rebuild_dfmods_tree(&self) {
        if let Some(model) = self.dfmods_model.borrow().as_ref() {
            model.remove_all();

            let filter = self.dfmods_filter_state.borrow();
            let assets = self.dfmod_assets.borrow();

            if assets.is_empty() {
                let empty = TreeItem::new("No DFMod files found", "", false, 2);
                model.append(&empty);
            } else {
                for (dfmod_key, asset_paths) in assets.iter() {
                    // Check if any assets are visible under this dfmod
                    let has_visible = !filter.is_active()
                        || asset_paths.iter().any(|p| filter.is_visible(p));

                    if has_visible {
                        let item = TreeItem::new_dfmod(
                            dfmod_key,
                            dfmod_key,
                            asset_paths.len() as u32,
                        );
                        item.set_matches_filter(filter.matches(dfmod_key));
                        item.set_visible_in_filter(true);
                        model.append(&item);
                    }
                }

                // If filter is active but nothing visible, show a message
                if filter.is_active() && model.n_items() == 0 {
                    let no_matches = TreeItem::new("No matches found", "", false, 2);
                    model.append(&no_matches);
                }
            }
        }

        // Recreate the tree list view with the new model to refresh child closures
        if let Some(dfmods_store) = self.dfmods_model.borrow().clone() {
            let new_dfmods_list = self.create_dfmods_list_view(dfmods_store);

            if let Some(scroll) = self.dfmods_scroll.borrow().as_ref() {
                scroll.set_child(Some(&new_dfmods_list));
            }

            self.dfmods_list.replace(Some(new_dfmods_list));
        }
    }

    /// Update the panel using cached conflict data from a scan
    pub fn update_with_cached_conflicts(
        &self,
        mod_path: &PathBuf,
        conflict_summary: Option<&ModConflictSummary>,
    ) {
        // Store mod path for tree model child creation
        self.current_mod_path.replace(Some(mod_path.clone()));

        // Clear filters when switching mods
        {
            let mut filter = self.files_filter_state.borrow_mut();
            filter.clear();
        }
        if let Some(widget) = self.filter_widget.borrow().as_ref() {
            widget.clear();
        }
        {
            let mut filter = self.dfmods_filter_state.borrow_mut();
            filter.clear();
        }
        if let Some(widget) = self.dfmods_filter_widget.borrow().as_ref() {
            widget.clear();
        }

        // Update conflicts tab with cached data
        self.update_conflicts_from_cache(conflict_summary);

        // Update files tab (just shows folder structure, no dfmod parsing)
        self.update_files(mod_path);

        // Update dfmods tab using shared cache
        self.update_dfmods(mod_path);

        // Refresh downloads list
        self.refresh_downloads();
    }

    /// Update the conflicts tab using cached conflict summary
    fn update_conflicts_from_cache(&self, conflict_summary: Option<&ModConflictSummary>) {
        // Store conflict data for the TreeListModel callback
        {
            let mut data = self.conflict_data.borrow_mut();
            data.clear();
            if let Some(summary) = conflict_summary {
                for conflict in &summary.conflicts {
                    data.insert(
                        conflict.other_mod_name.clone(),
                        conflict.conflicting_files.clone(),
                    );
                }
            }
        }

        if let Some(model) = self.conflicts_model.borrow().as_ref() {
            model.remove_all();

            match conflict_summary {
                None => {
                    // No scan done yet
                    let no_scan = TreeItem::new("Run 'Scan Conflicts' to detect conflicts", "", false, 2);
                    model.append(&no_scan);
                }
                Some(summary) if summary.conflicts.is_empty() => {
                    // Scan done, no conflicts
                    let no_conflicts = TreeItem::new("No conflicts detected", "", false, 2);
                    model.append(&no_conflicts);
                }
                Some(summary) => {
                    // Add only mod roots - children are provided by TreeListModel callback
                    for conflict in &summary.conflicts {
                        let mod_item = TreeItem::new_mod_root(
                            &conflict.other_mod_name,
                            conflict.other_mod_path.to_str().unwrap_or(""),
                            conflict.conflicting_files.len() as u32,
                        );
                        model.append(&mod_item);
                    }
                }
            }
        }

        // Update tab label with count
        if let Some(notebook) = self.notebook.borrow().as_ref() {
            let total_conflicts = conflict_summary
                .map(|s| s.total_conflict_count)
                .unwrap_or(0);

            let label_text = if total_conflicts > 0 {
                format!("Conflicts ({})", total_conflicts)
            } else {
                "Conflicts".to_string()
            };

            // Get the third page (conflicts tab at index 2) and update its label
            if let Some(page) = notebook.nth_page(Some(2)) {
                let label = Label::new(Some(&label_text));
                notebook.set_tab_label(&page, Some(&label));
            }
        }
    }

    /// Update the files tab (shows folder structure only, no dfmod parsing)
    fn update_files(&self, mod_path: &PathBuf) {
        // Clear dfmod assets from previous selection
        self.dfmod_assets.borrow_mut().clear();

        if let Some(model) = self.files_model.borrow().as_ref() {
            model.remove_all();

            // Get top-level children (folders in mod root)
            let children = get_children_at_path(mod_path, "");

            if children.is_empty() {
                let empty = TreeItem::new("No files found", "", false, 2);
                model.append(&empty);
            } else {
                // Add loose files/folders
                for (name, rel_path, is_dir) in children {
                    let item = if is_dir {
                        TreeItem::new_folder(&name, &rel_path)
                    } else {
                        TreeItem::new_file(&name, &rel_path)
                    };
                    model.append(&item);
                }
            }
        }

        // Recreate the list view to ensure fresh closures
        if let Some(files_store) = self.files_model.borrow().clone() {
            let new_files_list = self.create_files_list_view(files_store);

            if let Some(scroll) = self.files_scroll.borrow().as_ref() {
                scroll.set_child(Some(&new_files_list));
            }

            self.files_list.replace(Some(new_files_list));
        }
    }

    /// Update the dfmods tab using the shared dfmod cache
    fn update_dfmods(&self, mod_path: &PathBuf) {
        // Clear previous dfmod data
        self.dfmod_assets.borrow_mut().clear();

        if let Some(model) = self.dfmods_model.borrow().as_ref() {
            model.remove_all();

            // Get dfmod file list (fast, no extraction)
            match parse_dfmod_basic(mod_path) {
                Ok(dfmod_infos) if !dfmod_infos.is_empty() => {
                    // Get cached assets from shared cache
                    let cache_ref = self.shared_dfmod_cache.borrow();

                    let mut local_cache: HashMap<DfmodCacheKey, Vec<String>> = cache_ref
                        .as_ref()
                        .and_then(|c| c.lock().ok())
                        .map(|g| (*g).clone())
                        .unwrap_or_default();

                    let mut total_assets = 0u32;

                    for info in &dfmod_infos {
                        // Build dfmod path and get cached assets
                        let dfmod_path = mod_path.join("Mods").join(format!("{}.dfmod", info.file_name));
                        let assets = extract_dfmod_assets_cached(&dfmod_path, &mut local_cache);

                        // Store for lazy loading in tree model
                        self.dfmod_assets.borrow_mut()
                            .insert(info.file_name.clone(), assets.clone());

                        total_assets += assets.len() as u32;

                        // Create dfmod root TreeItem
                        let item = TreeItem::new_dfmod(
                            &info.title,
                            &info.file_name,
                            assets.len() as u32,
                        );
                        model.append(&item);
                    }

                    // Merge any new cache entries back to shared cache
                    // (cache_ref borrow was dropped when we cloned into local_cache)
                    if let Some(shared) = self.shared_dfmod_cache.borrow().as_ref() {
                        if let Ok(mut guard) = shared.lock() {
                            for (key, value) in local_cache {
                                guard.entry(key).or_insert(value);
                            }
                        }
                    }

                    // Update tab label with total asset count
                    self.update_dfmods_tab_label(total_assets);
                }
                _ => {
                    let empty = TreeItem::new("No DFMod files found", "", false, 2);
                    model.append(&empty);
                    self.update_dfmods_tab_label(0);
                }
            }
        }

        // Recreate the list view to ensure fresh closures
        if let Some(dfmods_store) = self.dfmods_model.borrow().clone() {
            let new_dfmods_list = self.create_dfmods_list_view(dfmods_store);

            if let Some(scroll) = self.dfmods_scroll.borrow().as_ref() {
                scroll.set_child(Some(&new_dfmods_list));
            }

            self.dfmods_list.replace(Some(new_dfmods_list));
        }
    }

    /// Update the DFMods tab label with asset count
    fn update_dfmods_tab_label(&self, asset_count: u32) {
        if let Some(notebook) = self.notebook.borrow().as_ref() {
            let label_text = if asset_count > 0 {
                format!("DFMods ({})", asset_count)
            } else {
                "DFMods".to_string()
            };

            // DFMods is at index 1
            if let Some(page) = notebook.nth_page(Some(1)) {
                let label = Label::new(Some(&label_text));
                notebook.set_tab_label(&page, Some(&label));
            }
        }
    }

    /// Clear the panel
    pub fn clear(&self) {
        self.current_mod_path.replace(None);
        self.conflict_data.borrow_mut().clear();
        self.dfmod_assets.borrow_mut().clear();

        // Clear filters
        {
            let mut filter = self.files_filter_state.borrow_mut();
            filter.clear();
        }
        if let Some(widget) = self.filter_widget.borrow().as_ref() {
            widget.clear();
        }
        {
            let mut filter = self.dfmods_filter_state.borrow_mut();
            filter.clear();
        }
        if let Some(widget) = self.dfmods_filter_widget.borrow().as_ref() {
            widget.clear();
        }

        if let Some(model) = self.conflicts_model.borrow().as_ref() {
            model.remove_all();
            let placeholder = TreeItem::new("Select a mod to view conflicts", "", false, 2);
            model.append(&placeholder);
        }

        if let Some(model) = self.files_model.borrow().as_ref() {
            model.remove_all();
            let placeholder = TreeItem::new("Select a mod to view files", "", false, 2);
            model.append(&placeholder);
        }

        if let Some(model) = self.dfmods_model.borrow().as_ref() {
            model.remove_all();
            let placeholder = TreeItem::new("Select a mod to view DFMod contents", "", false, 2);
            model.append(&placeholder);
        }

        // Reset tab labels
        if let Some(notebook) = self.notebook.borrow().as_ref() {
            // DFMods is at index 1
            if let Some(page) = notebook.nth_page(Some(1)) {
                let label = Label::new(Some("DFMods"));
                notebook.set_tab_label(&page, Some(&label));
            }
            // Conflicts is at index 2
            if let Some(page) = notebook.nth_page(Some(2)) {
                let label = Label::new(Some("Conflicts"));
                notebook.set_tab_label(&page, Some(&label));
            }
        }

        // Refresh downloads list (downloads are global, not per-mod)
        self.refresh_downloads();
    }

    /// Scan the downloads directory and populate the downloads list
    pub fn refresh_downloads(&self) {
        if let Some(model) = self.downloads_model.borrow().as_ref() {
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
    fn on_install_clicked(&self, selection: &SingleSelection) {
        let position = selection.selected();
        if position == gtk4::INVALID_LIST_POSITION {
            return;
        }

        let Some(item) = selection.selected_item().and_downcast::<DownloadItem>() else {
            return;
        };

        log::info!("Installing download: {}", item.display_name());

        // Get current mod path for the profile
        let mod_path = self.current_mod_path.borrow();
        let Some(mod_path) = mod_path.as_ref() else {
            log::warn!("No mod path set for install");
            return;
        };

        // Get the profile's mods folder (parent of current mod)
        let Some(mods_folder) = mod_path.parent() else {
            log::error!("Could not determine mods folder from path: {:?}", mod_path);
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
                // Note: The mod list will need to be refreshed by the parent window
                // We could emit a signal here for that purpose
            }
            Err(e) => {
                log::error!("Failed to extract mod archive: {}", e);
            }
        }
    }

    /// Handle Delete button click
    fn on_delete_clicked(&self, selection: &SingleSelection) {
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
        self.refresh_downloads();
    }
}

/// Extract a display name from download metadata
fn extract_display_name(metadata: &DownloadMetadata) -> String {
    // Try to parse from source_url (contains original filename)
    // URL format: https://.../Bestiary%20Linux-222-2-2-1-1708550222.zip
    if let Some(last_segment) = metadata.source_url.split('/').last() {
        if let Ok(decoded) = urlencoding::decode(last_segment) {
            let name = decoded.trim_end_matches(".zip");
            // Remove the timestamp suffix if present (last segment after dash if it's all digits)
            if let Some(dash_pos) = name.rfind('-') {
                let suffix = &name[dash_pos + 1..];
                if suffix.chars().all(|c| c.is_ascii_digit()) && suffix.len() > 8 {
                    return name[..dash_pos].to_string();
                }
            }
            return name.to_string();
        }
    }

    // Fall back to mod_name if available
    if let Some(mod_name) = &metadata.mod_name {
        if !mod_name.is_empty() {
            return mod_name.clone();
        }
    }

    // Last resort: use the file name
    metadata.file_name.clone()
}

/// Determine the folder name for an extracted mod
fn determine_mod_folder_name(item: &DownloadItem) -> String {
    // Try to parse from source_url (contains original Nexus filename)
    // Format: "Bestiary Linux-222-2-2-1-1708550222.zip"
    let source_url = item.source_url();
    if let Some(last_segment) = source_url.split('/').last() {
        if let Ok(decoded) = urlencoding::decode(last_segment) {
            let name = decoded.trim_end_matches(".zip");
            // This is already in the correct format for mod folders
            return name.to_string();
        }
    }

    // Fall back to constructing from metadata
    let mod_name = if item.mod_name().is_empty() {
        "Unknown".to_string()
    } else {
        item.mod_name()
    };

    let version = if item.version().is_empty() {
        "1-0".to_string()
    } else {
        item.version().replace('.', "-")
    };

    let timestamp = item.downloaded_at();

    format!("{}-{}-{}-{}", mod_name, item.mod_id(), version, timestamp)
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

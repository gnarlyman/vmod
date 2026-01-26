use gtk4::prelude::*;
use gtk4::subclass::prelude::*;
use gtk4::{
    glib, gio, Box, Label, Notebook, Orientation, ScrolledWindow,
    ListView, SignalListItemFactory, SingleSelection, TreeExpander, TreeListModel,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

use crate::mod_entry::{TreeItem, get_children_at_path, ModConflictSummary};

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

        // Files tab
        let files_box = Box::new(Orientation::Vertical, 0);
        let files_scroll = ScrolledWindow::new();
        files_scroll.set_vexpand(true);
        files_scroll.set_hexpand(true);
        files_scroll.set_min_content_height(150);
        files_scroll.set_max_content_height(200);

        // Create files ListStore - we'll use a flat list with indentation
        let files_store = gio::ListStore::new::<TreeItem>();
        self.files_model.replace(Some(files_store.clone()));

        // Store the mod path for child model creation
        let mod_path_ref = self.current_mod_path.clone();
        // Store dfmod assets reference for the callback
        let dfmod_assets_ref = self.dfmod_assets.clone();

        let files_tree_model = TreeListModel::new(
            files_store.clone(),
            false, // passthrough
            false, // autoexpand (start collapsed)
            move |item| {
                let tree_item = item.downcast_ref::<TreeItem>().unwrap();
                if tree_item.is_expandable() {
                    let item_type = tree_item.item_type();

                    // Handle dfmod archives (type 3) - return asset paths
                    if item_type == 3 {
                        // full_path stores the dfmod file_name (lookup key)
                        let dfmod_key = tree_item.full_path();
                        let assets = dfmod_assets_ref.borrow();
                        if let Some(asset_paths) = assets.get(&dfmod_key) {
                            let children_store = gio::ListStore::new::<TreeItem>();
                            for asset_path in asset_paths {
                                let child = TreeItem::new_file(asset_path, asset_path);
                                children_store.append(&child);
                            }
                            return Some(children_store.upcast());
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
                                let child = if is_dir {
                                    TreeItem::new_folder(&name, &rel_path)
                                } else {
                                    TreeItem::new_file(&name, &rel_path)
                                };
                                children_store.append(&child);
                            }
                            return Some(children_store.upcast());
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
                }
            }
        });

        files_list.set_factory(Some(&files_factory));
        files_scroll.set_child(Some(&files_list));
        files_box.append(&files_scroll);

        self.files_list.replace(Some(files_list));

        // Add tabs to notebook
        let conflicts_label = Label::new(Some("Conflicts"));
        let files_label = Label::new(Some("Files"));

        notebook.append_page(&conflicts_box, Some(&conflicts_label));
        notebook.append_page(&files_box, Some(&files_label));

        self.notebook.replace(Some(notebook.clone()));

        obj.append(&notebook);
    }
}

impl WidgetImpl for ConflictPanel {}
impl BoxImpl for ConflictPanel {}

impl ConflictPanel {
    /// Update the panel using cached conflict data from a scan
    pub fn update_with_cached_conflicts(
        &self,
        mod_path: &PathBuf,
        conflict_summary: Option<&ModConflictSummary>,
    ) {
        // Store mod path for tree model child creation
        self.current_mod_path.replace(Some(mod_path.clone()));

        // Update conflicts tab with cached data
        self.update_conflicts_from_cache(conflict_summary);

        // Update files tab (just shows folder structure, no dfmod parsing)
        self.update_files(mod_path);
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

            // Get the first page (conflicts tab) and update its label
            if let Some(page) = notebook.nth_page(Some(0)) {
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
    }

    /// Clear the panel
    pub fn clear(&self) {
        self.current_mod_path.replace(None);
        self.conflict_data.borrow_mut().clear();
        self.dfmod_assets.borrow_mut().clear();

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

        // Reset tab labels
        if let Some(notebook) = self.notebook.borrow().as_ref() {
            if let Some(page) = notebook.nth_page(Some(0)) {
                let label = Label::new(Some("Conflicts"));
                notebook.set_tab_label(&page, Some(&label));
            }
        }
    }
}

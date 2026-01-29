//! Core ModListView implementation - struct definition and ObjectSubclass.

use gtk4::subclass::prelude::*;
use gtk4::{
    glib, gio, Box, Button, ColumnView, CustomFilter, FilterListModel, Label,
    ProgressBar, Paned, SearchEntry, SingleSelection,
};
use std::collections::HashSet;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use crate::mod_entry::{DfmodCacheKey, ModConflictSummary, SectionsConfig, VersionCache, VirtualFileSystem};
use crate::mods_json_view::ModsJsonView;
use crate::conflict_panel::ConflictPanel;

use super::vfs_state::{rebuild_vfs_static, save_mod_state_static};

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
    pub refresh_button: RefCell<Option<Button>>,
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
    pub mods_folder: Rc<RefCell<Option<PathBuf>>>,
    pub game_mods_folder: RefCell<Option<PathBuf>>,
    // Downloads panel state
    pub downloads_model: RefCell<Option<gio::ListStore>>,
    pub bottom_paned: RefCell<Option<Paned>>,
    // Version checking state
    pub version_cache: Rc<RefCell<VersionCache>>,
    pub is_version_checking: Rc<RefCell<bool>>,
    pub check_updates_button: RefCell<Option<Button>>,
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
            refresh_button: RefCell::new(None),
            progress_box: RefCell::new(None),
            progress_bar: RefCell::new(None),
            progress_label: RefCell::new(None),
            conflict_results: Rc::new(RefCell::new(HashMap::new())),
            dfmod_cache: Arc::new(Mutex::new(HashMap::new())),
            is_scanning: Rc::new(RefCell::new(false)),
            sections_config: Rc::new(RefCell::new(SectionsConfig::default())),
            collapsed_sections: Rc::new(RefCell::new(HashSet::new())),
            profile_path: Rc::new(RefCell::new(None)),
            mods_folder: Rc::new(RefCell::new(None)),
            game_mods_folder: RefCell::new(None),
            downloads_model: RefCell::new(None),
            bottom_paned: RefCell::new(None),
            version_cache: Rc::new(RefCell::new(VersionCache::load())),
            is_version_checking: Rc::new(RefCell::new(false)),
            check_updates_button: RefCell::new(None),
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
        // Delegate to ui_builder module
        self.build_ui();
    }
}

impl WidgetImpl for ModListView {}
impl BoxImpl for ModListView {}

impl ModListView {
    /// Rebuild all symlinks based on enabled mods and their order
    pub fn rebuild_vfs(&self) {
        rebuild_vfs_static(&self.model, &self.vfs);
    }

    /// Save mod state
    pub fn save_mod_state(&self) {
        save_mod_state_static(&self.model, &self.profile_name);
    }
}

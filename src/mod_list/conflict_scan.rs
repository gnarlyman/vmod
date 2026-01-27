//! Async conflict scanning functionality.

use gtk4::prelude::*;
use gtk4::{glib, Box, Button, Label, ProgressBar};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use crate::mod_entry::{detect_all_conflicts, DfmodCacheKey, ModConflictSummary, ModEntry};
use super::imp::ModListView;

/// Progress state shared between background thread and main thread
pub struct ScanProgressState {
    pub current: usize,
    pub total: usize,
    pub current_mod: String,
    pub completed: bool,
    pub results: Option<HashMap<PathBuf, ModConflictSummary>>,
}

impl ModListView {
    /// Start async conflict scanning on a background thread
    pub fn start_conflict_scan(
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

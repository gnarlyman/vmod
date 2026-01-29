//! Async version checking functionality for Nexus Mods updates.

use gtk4::prelude::*;
use gtk4::{glib, Box, Button, Label, ProgressBar};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use crate::mod_entry::{ModEntry, VersionCache};
use crate::nexus_api::{ModFile, NexusClient};
use super::imp::ModListView;

/// Version status constants
pub const VERSION_UNKNOWN: u8 = 0;
pub const VERSION_UPTODATE: u8 = 1;
pub const VERSION_OUTDATED: u8 = 2;

/// Progress state shared between background thread and main thread
pub struct VersionCheckState {
    pub current: usize,
    pub total: usize,
    pub current_mod: String,
    pub completed: bool,
    /// Results: (folder_name, status, latest_version, nexus_id)
    pub results: Vec<(String, u8, Option<String>, Option<String>)>,
}

impl ModListView {
    /// Start async version checking on a background thread
    pub fn start_version_check(
        model: &RefCell<Option<gio::ListStore>>,
        is_checking: &Rc<RefCell<bool>>,
        version_cache: &Rc<RefCell<VersionCache>>,
        progress_box: &Box,
        progress_bar: &ProgressBar,
        progress_label: &Label,
        check_button: &Button,
        api_key: String,
        game_domain: String,
        force_recheck: bool,
    ) {
        // Check if already checking
        if *is_checking.borrow() {
            return;
        }
        *is_checking.borrow_mut() = true;

        // Collect mods with nexus_id that need checking
        let mut mods_to_check: Vec<(String, String, String)> = Vec::new(); // (folder_name, version, nexus_id)
        let cache = version_cache.borrow();

        if let Some(model_store) = model.borrow().as_ref() {
            for i in 0..model_store.n_items() {
                if let Some(item) = model_store.item(i) {
                    if let Ok(entry) = item.downcast::<ModEntry>() {
                        if let Some(nexus_id) = entry.nexus_id() {
                            let folder_name = entry.name();
                            // Check if needs checking (not in cache or force recheck)
                            if force_recheck || cache.needs_check(&folder_name) {
                                mods_to_check.push((
                                    folder_name,
                                    entry.version(),
                                    nexus_id,
                                ));
                            }
                        }
                    }
                }
            }
        }
        drop(cache);

        if mods_to_check.is_empty() {
            log::info!("No mods need version checking");
            *is_checking.borrow_mut() = false;
            return;
        }

        // Group mods by nexus_id to minimize API calls
        let mut mods_by_id: HashMap<String, Vec<(String, String)>> = HashMap::new();
        for (folder_name, version, nexus_id) in mods_to_check {
            mods_by_id
                .entry(nexus_id)
                .or_default()
                .push((folder_name, version));
        }

        let total_unique = mods_by_id.len();
        log::info!("Starting version check for {} unique Nexus mods", total_unique);

        // Show progress UI and disable button during check
        progress_box.set_visible(true);
        check_button.set_sensitive(false);
        progress_bar.set_fraction(0.0);
        progress_label.set_text("Checking versions...");

        // Shared state for progress
        let progress_state = Arc::new(Mutex::new(VersionCheckState {
            current: 0,
            total: total_unique,
            current_mod: String::new(),
            completed: false,
            results: Vec::new(),
        }));

        let progress_state_thread = progress_state.clone();

        // Spawn background thread for API calls
        std::thread::spawn(move || {
            // Create API client
            let client = match NexusClient::new(api_key, game_domain) {
                Ok(c) => c,
                Err(e) => {
                    log::error!("Failed to create Nexus client: {}", e);
                    let mut state = progress_state_thread.lock().unwrap();
                    state.completed = true;
                    return;
                }
            };

            let mut current = 0;
            for (nexus_id, folders) in mods_by_id {
                current += 1;

                // Update progress
                {
                    let mut state = progress_state_thread.lock().unwrap();
                    state.current = current;
                    state.current_mod = nexus_id.clone();
                }

                // Parse nexus_id to u64
                let mod_id: u64 = match nexus_id.parse() {
                    Ok(id) => id,
                    Err(_) => {
                        log::warn!("Invalid nexus_id: {}", nexus_id);
                        // Mark all folders as unknown
                        let mut state = progress_state_thread.lock().unwrap();
                        for (folder_name, _) in folders {
                            state.results.push((folder_name, VERSION_UNKNOWN, None, Some(nexus_id.clone())));
                        }
                        continue;
                    }
                };

                // Fetch mod info from API
                let mod_version = match client.get_mod_info(mod_id) {
                    Ok(response) => Some(response.data.version),
                    Err(e) => {
                        log::warn!("Failed to get mod info for {}: {}", nexus_id, e);
                        None
                    }
                };

                // Fetch mod files from API
                let files = match client.get_mod_files(mod_id) {
                    Ok(response) => response.data.files,
                    Err(e) => {
                        log::warn!("Failed to get mod files for {}: {}", nexus_id, e);
                        Vec::new()
                    }
                };

                // Find latest version among MAIN files (or all files if no MAIN)
                let latest_version = find_latest_version(&files, &mod_version);

                // Check each folder against available versions
                for (folder_name, local_version) in folders {
                    let status = check_version_status(&local_version, &mod_version, &files);
                    let mut state = progress_state_thread.lock().unwrap();
                    state.results.push((
                        folder_name,
                        status,
                        latest_version.clone(),
                        Some(nexus_id.clone()),
                    ));
                }

                // Rate limiting: 100ms delay between requests
                std::thread::sleep(std::time::Duration::from_millis(100));
            }

            // Mark as completed
            let mut state = progress_state_thread.lock().unwrap();
            state.completed = true;
            log::info!("Version check completed with {} results", state.results.len());
        });

        // Poll progress from main thread
        let is_checking_clone = is_checking.clone();
        let version_cache_clone = version_cache.clone();
        let model_clone = model.borrow().clone();
        let progress_box_clone = progress_box.clone();
        let progress_bar_clone = progress_bar.clone();
        let progress_label_clone = progress_label.clone();
        let check_button_clone = check_button.clone();

        glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
            let state = progress_state.lock().unwrap();

            if state.completed {
                // Apply results to ModEntry objects and update cache
                if let Some(ref model_store) = model_clone {
                    let mut cache = version_cache_clone.borrow_mut();

                    for (folder_name, status, latest_version, nexus_id) in &state.results {
                        // Update cache
                        cache.set(
                            folder_name.clone(),
                            *status,
                            latest_version.clone(),
                            nexus_id.clone(),
                        );

                        // Find and update the ModEntry
                        for i in 0..model_store.n_items() {
                            if let Some(item) = model_store.item(i) {
                                if let Ok(entry) = item.downcast::<ModEntry>() {
                                    if entry.name() == *folder_name {
                                        entry.set_version_status(*status);
                                        entry.set_latest_version_opt(latest_version.clone());
                                        break;
                                    }
                                }
                            }
                        }
                    }

                    // Save cache to disk
                    if let Err(e) = cache.save() {
                        log::error!("Failed to save version cache: {}", e);
                    }
                }

                // Hide progress UI and re-enable button
                progress_box_clone.set_visible(false);
                check_button_clone.set_sensitive(true);
                *is_checking_clone.borrow_mut() = false;

                return glib::ControlFlow::Break;
            }

            // Update progress UI
            let fraction = if state.total > 0 {
                state.current as f64 / state.total as f64
            } else {
                0.0
            };
            progress_bar_clone.set_fraction(fraction);
            progress_label_clone.set_text(&format!(
                "Checking {}/{}: mod {}",
                state.current,
                state.total,
                state.current_mod
            ));

            glib::ControlFlow::Continue
        });
    }
}

/// Normalize a version string for comparison
fn normalize_version(version: &str) -> String {
    let mut v = version.trim().to_lowercase();

    // Remove common prefixes
    for prefix in ["version", "ver", "v.", "v"] {
        if v.starts_with(prefix) {
            v = v[prefix.len()..].trim_start().to_string();
        }
    }

    // Replace separators with dots
    v = v.replace('-', ".").replace('_', ".");

    // Remove leading/trailing dots
    v.trim_matches('.').to_string()
}

/// Check if two versions match after normalization
fn versions_match(local: &str, remote: &str) -> bool {
    normalize_version(local) == normalize_version(remote)
}

/// Find the latest version from mod files
fn find_latest_version(files: &[ModFile], headline_version: &Option<String>) -> Option<String> {
    // Prefer MAIN category files
    let mut main_files: Vec<&ModFile> = files
        .iter()
        .filter(|f| f.category_name == "MAIN")
        .collect();

    // Fall back to all files if no MAIN
    if main_files.is_empty() {
        main_files = files.iter().collect();
    }

    // Sort by upload timestamp descending
    main_files.sort_by(|a, b| b.uploaded_timestamp.cmp(&a.uploaded_timestamp));

    // Return the newest file's version, or headline if no files
    main_files
        .first()
        .map(|f| f.version.clone())
        .or_else(|| headline_version.clone())
}

/// Check version status for a local mod version against Nexus
fn check_version_status(
    local_version: &str,
    headline_version: &Option<String>,
    files: &[ModFile],
) -> u8 {
    // Find the latest version (newest MAIN file, or headline)
    let latest = find_latest_version(files, headline_version);

    match latest {
        Some(ref latest_ver) => {
            if versions_match(local_version, latest_ver) {
                VERSION_UPTODATE  // Green: matches latest
            } else {
                VERSION_OUTDATED  // Red: doesn't match latest
            }
        }
        None => VERSION_UNKNOWN,  // No version info available
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_version() {
        assert_eq!(normalize_version("1.5"), "1.5");
        assert_eq!(normalize_version("v1.5"), "1.5");
        assert_eq!(normalize_version("V1.5"), "1.5");
        assert_eq!(normalize_version("version 1.5"), "1.5");
        assert_eq!(normalize_version("1-5"), "1.5");
        assert_eq!(normalize_version("1_5"), "1.5");
        assert_eq!(normalize_version(" 1.5 "), "1.5");
    }

    #[test]
    fn test_versions_match() {
        assert!(versions_match("1.5", "1.5"));
        assert!(versions_match("v1.5", "1.5"));
        assert!(versions_match("1-5", "1.5"));
        assert!(!versions_match("1.5", "1.6"));
        assert!(!versions_match("1.5", "2.0"));
    }
}

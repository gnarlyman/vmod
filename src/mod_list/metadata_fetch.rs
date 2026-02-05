//! Batch metadata and version checking from Nexus Mods API.
//!
//! Fetches mod names, metadata, and version status for all mods
//! that have a nexus_id. Skips mods that already have up-to-date
//! metadata unless a force refresh is requested.

use gtk4::prelude::*;
use gtk4::{gio, glib, Box, Button, Label, ProgressBar};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use crate::mod_entry::{ModEntry, ModMetadata, load_metadata, save_metadata};
use crate::nexus_api::{ModFile, NexusClient};
use super::imp::ModListView;

/// Version status constants
pub const VERSION_UNKNOWN: u8 = 0;
pub const VERSION_UPTODATE: u8 = 1;
pub const VERSION_OUTDATED: u8 = 2;

/// Progress state shared between background thread and main thread
pub struct MetadataFetchState {
    pub current: usize,
    pub total: usize,
    pub current_mod: String,
    pub completed: bool,
    /// Results: (folder_name, mod_path, mod_name, version_status, latest_version)
    pub results: Vec<(String, PathBuf, String, u8, Option<String>)>,
}

impl ModListView {
    /// Start async metadata fetch and version check on a background thread.
    ///
    /// Processes all mods with a nexus_id:
    /// - Mods without `vmod_meta.json`: creates metadata from API
    /// - All mods: checks version status against Nexus files
    pub fn start_metadata_fetch(
        model: &RefCell<Option<gio::ListStore>>,
        is_fetching: &Rc<RefCell<bool>>,
        progress_box: &Box,
        progress_bar: &ProgressBar,
        progress_label: &Label,
        fetch_button: &Button,
        api_key: String,
        game_domain: String,
    ) {
        // Check if already fetching
        if *is_fetching.borrow() {
            return;
        }
        *is_fetching.borrow_mut() = true;

        // Collect mods with nexus_id that need fetching:
        // - no vmod_meta.json yet, OR
        // - version_status is still unknown (never checked)
        let mut mods_to_fetch: Vec<(String, PathBuf, String, String)> = Vec::new(); // (folder_name, path, version, nexus_id)

        if let Some(model_store) = model.borrow().as_ref() {
            for i in 0..model_store.n_items() {
                if let Some(item) = model_store.item(i) {
                    if let Ok(entry) = item.downcast::<ModEntry>() {
                        if let Some(nexus_id) = entry.nexus_id() {
                            let needs_fetch = entry.version_status() == VERSION_UNKNOWN;
                            if needs_fetch {
                                mods_to_fetch.push((
                                    entry.name(),
                                    entry.path(),
                                    entry.version(),
                                    nexus_id,
                                ));
                            }
                        }
                    }
                }
            }
        }

        if mods_to_fetch.is_empty() {
            log::info!("All mods already up to date");
            *is_fetching.borrow_mut() = false;
            return;
        }

        // Group mods by nexus_id to minimize API calls
        let mut mods_by_id: HashMap<String, Vec<(String, PathBuf, String)>> = HashMap::new();
        for (folder_name, path, version, nexus_id) in mods_to_fetch {
            mods_by_id
                .entry(nexus_id)
                .or_default()
                .push((folder_name, path, version));
        }

        let total_unique = mods_by_id.len();
        log::info!("Starting metadata fetch for {} unique Nexus mods", total_unique);

        // Show progress UI and disable button during fetch
        progress_box.set_visible(true);
        fetch_button.set_sensitive(false);
        progress_bar.set_fraction(0.0);
        progress_label.set_text("Fetching mod info...");

        // Shared state for progress
        let progress_state = Arc::new(Mutex::new(MetadataFetchState {
            current: 0,
            total: total_unique,
            current_mod: String::new(),
            completed: false,
            results: Vec::new(),
        }));

        let progress_state_thread = progress_state.clone();
        let game_domain_thread = game_domain.clone();

        // Spawn background thread for API calls
        std::thread::spawn(move || {
            // Create API client
            let client = match NexusClient::new(api_key, game_domain_thread.clone()) {
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
                        continue;
                    }
                };

                // Fetch mod info from API
                let mod_info = client.get_mod_info(mod_id).ok().map(|r| r.data);
                let mod_version = mod_info.as_ref().map(|i| i.version.clone());
                let mod_name = mod_info.as_ref().map(|i| i.name.clone());

                // Fetch mod files from API for version checking
                let files = match client.get_mod_files(mod_id) {
                    Ok(response) => response.data.files,
                    Err(e) => {
                        log::warn!("Failed to get mod files for {}: {}", nexus_id, e);
                        Vec::new()
                    }
                };

                let latest_version = find_latest_version(&files, &mod_version);

                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);

                // Process each folder sharing this nexus_id
                for (folder_name, path, local_version) in &folders {
                    let status = check_version_status(local_version, &mod_version, &files);

                    // Load existing metadata or create new
                    let mut metadata = load_metadata(path).unwrap_or_else(|| ModMetadata {
                        mod_name: mod_name.clone().unwrap_or_else(|| folder_name.clone()),
                        nexus_id: nexus_id.clone(),
                        version: Some(local_version.clone()),
                        file_id: None,
                        game_domain: Some(game_domain_thread.clone()),
                        fetched_at: Some(chrono::Utc::now().timestamp()),
                        version_status: 0,
                        latest_version: None,
                        version_checked_at: None,
                    });

                    // Update all fields from API
                    if let Some(ref name) = mod_name {
                        metadata.mod_name = name.clone();
                    }
                    metadata.version_status = status;
                    metadata.latest_version = latest_version.clone();
                    metadata.version_checked_at = Some(now);
                    metadata.fetched_at = Some(chrono::Utc::now().timestamp());

                    if let Err(e) = save_metadata(path, &metadata) {
                        log::warn!("Failed to save metadata for {}: {}", folder_name, e);
                    }

                    let mut state = progress_state_thread.lock().unwrap();
                    state.results.push((
                        folder_name.clone(),
                        path.clone(),
                        metadata.mod_name.clone(),
                        status,
                        latest_version.clone(),
                    ));
                }

                // Rate limiting: 100ms delay between requests
                std::thread::sleep(std::time::Duration::from_millis(100));
            }

            // Mark as completed
            let mut state = progress_state_thread.lock().unwrap();
            state.completed = true;
            log::info!("Metadata fetch completed with {} results", state.results.len());
        });

        // Poll progress from main thread
        let is_fetching_clone = is_fetching.clone();
        let model_clone = model.borrow().clone();
        let progress_box_clone = progress_box.clone();
        let progress_bar_clone = progress_bar.clone();
        let progress_label_clone = progress_label.clone();
        let fetch_button_clone = fetch_button.clone();

        glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
            let state = progress_state.lock().unwrap();

            if state.completed {
                // Apply results to ModEntry objects
                if let Some(ref model_store) = model_clone {
                    for (folder_name, _path, mod_name, status, latest_version) in &state.results {
                        for i in 0..model_store.n_items() {
                            if let Some(item) = model_store.item(i) {
                                if let Ok(entry) = item.downcast::<ModEntry>() {
                                    if entry.name() == *folder_name {
                                        entry.set_display_name(mod_name.clone());
                                        entry.set_version_status(*status);
                                        entry.set_latest_version_opt(latest_version.clone());
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }

                // Hide progress UI and re-enable button
                progress_box_clone.set_visible(false);
                fetch_button_clone.set_sensitive(true);
                *is_fetching_clone.borrow_mut() = false;

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
    let latest = find_latest_version(files, headline_version);

    match latest {
        Some(ref latest_ver) => {
            if versions_match(local_version, latest_ver) {
                VERSION_UPTODATE
            } else {
                VERSION_OUTDATED
            }
        }
        None => VERSION_UNKNOWN,
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

//! Batch metadata and version checking from Nexus Mods API.
//!
//! Two separate operations:
//! - `start_metadata_fetch()`: Fetches mod names/metadata only (idempotent)
//! - `start_version_check()`: Checks ALL mods against Nexus for updates (non-idempotent)

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
    /// Results: (folder_name, mod_path, mod_name)
    pub results: Vec<(String, PathBuf, String)>,
}

/// Progress state for version checking
pub struct VersionCheckState {
    pub current: usize,
    pub total: usize,
    pub current_mod: String,
    pub completed: bool,
    /// Results: (folder_name, mod_path, version_status, latest_version)
    pub results: Vec<(String, PathBuf, u8, Option<String>)>,
}

/// Result for single mod version check
pub struct SingleVersionResult {
    pub completed: bool,
    pub version_status: u8,
    pub latest_version: Option<String>,
    pub error: Option<String>,
}

impl ModListView {
    /// Start async metadata fetch (mod names only, no version checking).
    ///
    /// Idempotent: Only processes mods that lack `vmod_meta.json` or have no real mod name.
    /// Updates `mod_name`, `nexus_id`, `version`, `fetched_at` in `vmod_meta.json`.
    /// Does NOT touch `version_status`, `latest_version`, or `version_checked_at`.
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

        // Collect mods that need metadata:
        // - Has nexus_id AND display_name equals folder name (no real name yet)
        let mut mods_to_fetch: Vec<(String, PathBuf, String, String)> = Vec::new(); // (folder_name, path, version, nexus_id)

        if let Some(model_store) = model.borrow().as_ref() {
            for i in 0..model_store.n_items() {
                if let Some(item) = model_store.item(i) {
                    if let Ok(entry) = item.downcast::<ModEntry>() {
                        if let Some(nexus_id) = entry.nexus_id() {
                            // Check if display_name == folder name (no real mod name yet)
                            let folder_name = entry.name();
                            let display_name = entry.display_name();
                            let needs_fetch = folder_name == display_name;

                            if needs_fetch {
                                mods_to_fetch.push((
                                    folder_name,
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
            log::info!("All mods already have metadata");
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

                // Fetch mod info from API (NOT mod files - no version checking)
                let mod_info = match client.get_mod_info(mod_id) {
                    Ok(r) => Some(r.data),
                    Err(e) => {
                        log::warn!("Failed to get mod info for {}: {}", nexus_id, e);
                        None
                    }
                };

                if mod_info.is_none() {
                    continue;
                }

                let mod_info = mod_info.unwrap();
                let mod_name = mod_info.name.clone();
                let mod_version = mod_info.version.clone();

                // Process each folder sharing this nexus_id
                for (folder_name, path, local_version) in &folders {
                    // Load existing metadata or create new
                    let mut metadata = load_metadata(path).unwrap_or_else(|| ModMetadata {
                        mod_name: mod_name.clone(),
                        nexus_id: nexus_id.clone(),
                        version: Some(local_version.clone()),
                        file_id: None,
                        game_domain: Some(game_domain_thread.clone()),
                        fetched_at: Some(chrono::Utc::now().timestamp()),
                        version_status: VERSION_UNKNOWN,
                        latest_version: None,
                        version_checked_at: None,
                    });

                    // Update only metadata fields (NOT version status fields)
                    metadata.mod_name = mod_name.clone();
                    metadata.version = Some(mod_version.clone());
                    metadata.fetched_at = Some(chrono::Utc::now().timestamp());
                    // Do NOT update: version_status, latest_version, version_checked_at

                    if let Err(e) = save_metadata(path, &metadata) {
                        log::warn!("Failed to save metadata for {}: {}", folder_name, e);
                    }

                    let mut state = progress_state_thread.lock().unwrap();
                    state.results.push((
                        folder_name.clone(),
                        path.clone(),
                        metadata.mod_name.clone(),
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
                // Apply results to ModEntry objects (only update display_name)
                if let Some(ref model_store) = model_clone {
                    for (folder_name, _path, mod_name) in &state.results {
                        for i in 0..model_store.n_items() {
                            if let Some(item) = model_store.item(i) {
                                if let Ok(entry) = item.downcast::<ModEntry>() {
                                    if entry.name() == *folder_name {
                                        entry.set_display_name(mod_name.clone());
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
                "Fetching {}/{}: mod {}",
                state.current,
                state.total,
                state.current_mod
            ));

            glib::ControlFlow::Continue
        });
    }

    /// Start async version check for ALL mods with nexus_id.
    ///
    /// Non-idempotent: Checks ALL mods every time, regardless of current status.
    /// Updates `version_status`, `latest_version`, `version_checked_at` in `vmod_meta.json`.
    pub fn start_version_check(
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

        // Collect ALL mods with nexus_id (no filtering by status)
        let mut mods_to_check: Vec<(String, PathBuf, String, String)> = Vec::new(); // (folder_name, path, version, nexus_id)

        if let Some(model_store) = model.borrow().as_ref() {
            for i in 0..model_store.n_items() {
                if let Some(item) = model_store.item(i) {
                    if let Ok(entry) = item.downcast::<ModEntry>() {
                        if let Some(nexus_id) = entry.nexus_id() {
                            mods_to_check.push((
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

        if mods_to_check.is_empty() {
            log::info!("No mods with Nexus ID to check");
            *is_fetching.borrow_mut() = false;
            return;
        }

        // Group mods by nexus_id to minimize API calls
        let mut mods_by_id: HashMap<String, Vec<(String, PathBuf, String)>> = HashMap::new();
        for (folder_name, path, version, nexus_id) in mods_to_check {
            mods_by_id
                .entry(nexus_id)
                .or_default()
                .push((folder_name, path, version));
        }

        let total_unique = mods_by_id.len();
        log::info!("Starting version check for {} unique Nexus mods", total_unique);

        // Show progress UI and disable button during check
        progress_box.set_visible(true);
        fetch_button.set_sensitive(false);
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

                // Fetch mod info to get headline version
                let mod_version = client.get_mod_info(mod_id).ok().map(|r| r.data.version);

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

                    // Load existing metadata or create minimal one
                    let mut metadata = load_metadata(path).unwrap_or_else(|| ModMetadata {
                        mod_name: folder_name.clone(),
                        nexus_id: nexus_id.clone(),
                        version: Some(local_version.clone()),
                        file_id: None,
                        game_domain: Some(game_domain_thread.clone()),
                        fetched_at: None,
                        version_status: VERSION_UNKNOWN,
                        latest_version: None,
                        version_checked_at: None,
                    });

                    // Update only version check fields
                    metadata.version_status = status;
                    metadata.latest_version = latest_version.clone();
                    metadata.version_checked_at = Some(now);
                    // Do NOT update: mod_name, fetched_at (those are metadata fetch responsibility)

                    if let Err(e) = save_metadata(path, &metadata) {
                        log::warn!("Failed to save metadata for {}: {}", folder_name, e);
                    }

                    let mut state = progress_state_thread.lock().unwrap();
                    state.results.push((
                        folder_name.clone(),
                        path.clone(),
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
            log::info!("Version check completed with {} results", state.results.len());
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
                // Apply results to ModEntry objects (update version status)
                if let Some(ref model_store) = model_clone {
                    for (folder_name, _path, status, latest_version) in &state.results {
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

    /// Check version for a single mod.
    ///
    /// Spawns a background request for a single mod, updates its metadata and UI.
    /// Returns immediately; use the callback closure to be notified when complete.
    pub fn check_single_mod_version(
        model: &RefCell<Option<gio::ListStore>>,
        folder_name: String,
        mod_path: PathBuf,
        nexus_id: String,
        local_version: String,
        api_key: String,
        game_domain: String,
    ) {
        log::info!("Checking version for single mod: {} (nexus_id={})", folder_name, nexus_id);

        // Shared state for result
        let result_state = Arc::new(Mutex::new(SingleVersionResult {
            completed: false,
            version_status: VERSION_UNKNOWN,
            latest_version: None,
            error: None,
        }));

        let result_state_thread = result_state.clone();
        let folder_name_thread = folder_name.clone();
        let mod_path_thread = mod_path.clone();
        let game_domain_thread = game_domain.clone();

        // Spawn background thread for single API call
        std::thread::spawn(move || {
            // Create API client
            let client = match NexusClient::new(api_key, game_domain_thread.clone()) {
                Ok(c) => c,
                Err(e) => {
                    log::error!("Failed to create Nexus client: {}", e);
                    let mut state = result_state_thread.lock().unwrap();
                    state.completed = true;
                    state.error = Some(format!("Failed to create API client: {}", e));
                    return;
                }
            };

            // Parse nexus_id to u64
            let mod_id: u64 = match nexus_id.parse() {
                Ok(id) => id,
                Err(_) => {
                    log::warn!("Invalid nexus_id: {}", nexus_id);
                    let mut state = result_state_thread.lock().unwrap();
                    state.completed = true;
                    state.error = Some("Invalid Nexus ID".to_string());
                    return;
                }
            };

            // Fetch mod info to get headline version
            let mod_version = client.get_mod_info(mod_id).ok().map(|r| r.data.version);

            // Fetch mod files from API for version checking
            let files = match client.get_mod_files(mod_id) {
                Ok(response) => response.data.files,
                Err(e) => {
                    log::warn!("Failed to get mod files for {}: {}", nexus_id, e);
                    let mut state = result_state_thread.lock().unwrap();
                    state.completed = true;
                    state.error = Some(format!("Failed to get mod files: {}", e));
                    return;
                }
            };

            let latest_version = find_latest_version(&files, &mod_version);
            let status = check_version_status(&local_version, &mod_version, &files);

            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);

            // Load existing metadata or create minimal one
            let mut metadata = load_metadata(&mod_path_thread).unwrap_or_else(|| ModMetadata {
                mod_name: folder_name_thread.clone(),
                nexus_id: nexus_id.clone(),
                version: Some(local_version.clone()),
                file_id: None,
                game_domain: Some(game_domain_thread.clone()),
                fetched_at: None,
                version_status: VERSION_UNKNOWN,
                latest_version: None,
                version_checked_at: None,
            });

            // Update only version check fields
            metadata.version_status = status;
            metadata.latest_version = latest_version.clone();
            metadata.version_checked_at = Some(now);

            if let Err(e) = save_metadata(&mod_path_thread, &metadata) {
                log::warn!("Failed to save metadata for {}: {}", folder_name_thread, e);
            }

            let mut state = result_state_thread.lock().unwrap();
            state.completed = true;
            state.version_status = status;
            state.latest_version = latest_version;
            log::info!("Version check completed for {}: status={}", folder_name_thread, status);
        });

        // Poll for result from main thread
        let model_clone = model.borrow().clone();
        log::debug!("Model clone is_some: {}", model_clone.is_some());

        glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
            let state = result_state.lock().unwrap();

            if state.completed {
                log::info!("Single version check completed, error: {:?}", state.error);
                if state.error.is_none() {
                    // Apply result to ModEntry object
                    if let Some(ref model_store) = model_clone {
                        log::debug!("Searching {} items for folder_name={}", model_store.n_items(), folder_name);
                        for i in 0..model_store.n_items() {
                            if let Some(item) = model_store.item(i) {
                                if let Ok(entry) = item.downcast::<ModEntry>() {
                                    if entry.name() == folder_name {
                                        log::info!("Updating ModEntry {} with status={}, latest={:?}",
                                            folder_name, state.version_status, state.latest_version);
                                        entry.set_version_status(state.version_status);
                                        entry.set_latest_version_opt(state.latest_version.clone());
                                        break;
                                    }
                                }
                            }
                        }
                    } else {
                        log::warn!("Model clone is None, cannot update UI");
                    }
                }
                return glib::ControlFlow::Break;
            }

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

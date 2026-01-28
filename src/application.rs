use gtk4::prelude::*;
use gtk4::{gio, glib, Application, ProgressBar, Label, Window};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use crate::config;
use crate::nexus_api::{check_existing_file, delete_existing_file, DownloadLink, DownloadManager, DownloadMetadata, DownloadProgress, DownloadState, NexusClient, NexusConfig};
use crate::nxm::NxmLink;
use crate::preferences::PreferencesDialog;
use crate::window::VmodWindow;

pub struct VmodApplication {
    app: Application,
}

impl VmodApplication {
    pub fn new() -> Self {
        let app = Application::builder()
            .application_id(config::APP_ID)
            .flags(gio::ApplicationFlags::HANDLES_OPEN)
            .build();

        let vmod_app = Self { app };

        vmod_app.setup_resources();
        vmod_app.setup_actions();
        vmod_app.setup_signals();

        vmod_app
    }

    fn setup_resources(&self) {
        // Load compiled GResource bundle
        gio::resources_register_include!("vmod.gresource")
            .expect("Failed to register resources");

        // Load CSS
        let provider = gtk4::CssProvider::new();
        provider.load_from_resource("/org/vmod/VMOD/style.css");
        gtk4::style_context_add_provider_for_display(
            &gtk4::gdk::Display::default().expect("Could not connect to a display"),
            &provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );

        // For development: compile schema if not in system location
        // In production, the schema would be installed to /usr/share/glib-2.0/schemas/
        self.setup_schema_for_dev();
    }

    fn setup_schema_for_dev(&self) {
        // Check if schema is already available (installed system-wide)
        if gio::SettingsSchemaSource::default()
            .and_then(|source| source.lookup(config::APP_ID, true))
            .is_some()
        {
            return;
        }

        // For development, use local compiled schema
        let resource_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources");

        if let Some(source) = gio::SettingsSchemaSource::from_directory(
            resource_dir.to_str().unwrap(),
            gio::SettingsSchemaSource::default().as_ref(),
            false,
        )
        .ok()
        {
            if source.lookup(config::APP_ID, true).is_none() {
                log::warn!(
                    "Schema {} not found. Run 'glib-compile-schemas resources/' first.",
                    config::APP_ID
                );
            }
        }
    }

    fn setup_actions(&self) {
        let quit = gio::ActionEntry::builder("quit")
            .activate(move |app: &Application, _, _| {
                app.quit();
            })
            .build();

        let preferences = gio::ActionEntry::builder("preferences")
            .activate(move |app: &Application, _, _| {
                if let Some(window) = app.active_window() {
                    let dialog = PreferencesDialog::new(&window);
                    dialog.present();
                }
            })
            .build();

        let open_profile_folder = gio::ActionEntry::builder("open_profile_folder")
            .activate(move |_app: &Application, _, _| {
                if let Some(config_dir) = dirs::config_dir() {
                    let profile_folder = config_dir.join("vmod");
                    if let Err(e) = open::that(&profile_folder) {
                        log::error!("Failed to open profile folder: {}", e);
                    }
                }
            })
            .build();

        let open_game_folder = gio::ActionEntry::builder("open_game_folder")
            .activate(move |_app: &Application, _, _| {
                if let Ok(profile_list) = crate::profile::profile_data::ProfileList::load() {
                    if let Some(active_profile) = profile_list.get_active_profile() {
                        if let Err(e) = open::that(&active_profile.game_path) {
                            log::error!("Failed to open game folder: {}", e);
                        }
                    }
                }
            })
            .build();

        let open_unity_config_folder = gio::ActionEntry::builder("open_unity_config_folder")
            .activate(move |_app: &Application, _, _| {
                if let Some(config_dir) = dirs::config_dir() {
                    let unity_folder = config_dir.join("unity3d/Daggerfall Workshop/Daggerfall Unity");
                    if let Err(e) = open::that(&unity_folder) {
                        log::error!("Failed to open Unity config folder: {}", e);
                    }
                }
            })
            .build();

        self.app.add_action_entries([quit, preferences, open_profile_folder, open_game_folder, open_unity_config_folder]);

        // Set up keyboard accelerators
        self.app.set_accels_for_action("app.quit", &["<Ctrl>Q"]);
        self.app.set_accels_for_action("app.preferences", &["<Ctrl>comma"]);
    }

    fn setup_signals(&self) {
        self.app.connect_activate(|app| {
            // Only create a new window if none exists
            if app.active_window().is_none() {
                let window = VmodWindow::new(app);
                window.present();
            } else {
                app.active_window().unwrap().present();
            }
        });

        // Handle file/URI opening (including NXM links)
        self.app.connect_open(|app, files, _hint| {
            // First ensure the window exists
            app.activate();

            log::info!("NXM handler invoked with {} URI(s)", files.len());

            // Process any NXM links
            for (i, file) in files.iter().enumerate() {
                let uri = file.uri().to_string();
                log::debug!("Processing URI {}/{}: {}", i + 1, files.len(), uri);

                if uri.starts_with("nxm://") {
                    match NxmLink::parse(&uri) {
                        Ok(nxm) => {
                            nxm.log_info();
                            // Start the download process
                            if let Some(window) = app.active_window() {
                                Self::handle_nxm_download(&window, nxm);
                            }
                        }
                        Err(e) => {
                            log::error!("Failed to parse NXM link: {}", e);
                            log::error!("URL was: {}", uri);
                        }
                    }
                }
            }
        });
    }

    /// Handle downloading a mod from an NXM link
    fn handle_nxm_download(window: &Window, nxm: NxmLink) {
        // Load Nexus config
        let config = NexusConfig::load();

        // Check if we have an API key
        if !config.has_api_key() {
            log::warn!("No Nexus API key configured, cannot download");
            Self::show_auth_required_dialog(window);
            return;
        }

        let api_key = config.api_key.clone().unwrap();

        // Verify we have download params
        let (key, expires) = match (nxm.key.as_ref(), nxm.expires) {
            (Some(k), Some(e)) => (k.clone(), e),
            _ => {
                log::error!("NXM link missing key or expires parameters");
                Self::show_error_dialog(window, "Invalid Download Link",
                    "The download link is missing required parameters. Please try clicking the download button on Nexus Mods again.");
                return;
            }
        };

        // Proceed with fetching links and download
        Self::fetch_links_and_download(window, nxm, api_key, key, expires);
    }

    /// Extract original filename from a Nexus download URL
    fn extract_filename_from_url(url: &str) -> Option<String> {
        // URL format: https://.../Bestiary%20Linux-222-2-2-1-1708550222.zip?md5=...
        url.split('/')
            .last()
            .and_then(|segment| segment.split('?').next())
            .and_then(|encoded| urlencoding::decode(encoded).ok())
            .map(|s| s.into_owned())
    }

    /// Fetch download links and then proceed with download
    fn fetch_links_and_download(
        window: &Window,
        nxm: NxmLink,
        api_key: String,
        key: String,
        expires: u64,
    ) {
        // Show a temporary "Fetching..." dialog
        let status_dialog = gtk4::Window::builder()
            .title("Preparing Download")
            .modal(true)
            .transient_for(window)
            .default_width(300)
            .default_height(80)
            .resizable(false)
            .build();

        let content = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
        content.set_margin_top(18);
        content.set_margin_bottom(18);
        content.set_margin_start(18);
        content.set_margin_end(18);

        let spinner = gtk4::Spinner::new();
        spinner.start();
        let status_label = Label::new(Some("Fetching download link..."));
        content.append(&spinner);
        content.append(&status_label);
        status_dialog.set_child(Some(&content));
        status_dialog.present();

        // Clone data for background thread
        let game = nxm.game.clone();
        let mod_id = nxm.mod_id;
        let file_id = nxm.file_id;
        let api_key_clone = api_key.clone();
        let key_clone = key.clone();

        // Result state
        let fetch_result: Arc<Mutex<Option<Result<Vec<DownloadLink>, String>>>> = Arc::new(Mutex::new(None));
        let fetch_result_thread = fetch_result.clone();

        // Spawn thread to fetch links
        std::thread::spawn(move || {
            let client = match NexusClient::new(api_key_clone, game) {
                Ok(c) => c,
                Err(e) => {
                    *fetch_result_thread.lock().unwrap() = Some(Err(format!("Failed to create API client: {}", e)));
                    return;
                }
            };

            log::info!("Fetching download link for mod {} file {}", mod_id, file_id);
            match client.get_download_link(mod_id, file_id, &key_clone, expires) {
                Ok(response) => {
                    *fetch_result_thread.lock().unwrap() = Some(Ok(response.data));
                }
                Err(e) => {
                    *fetch_result_thread.lock().unwrap() = Some(Err(format!("Failed to get download link: {}", e)));
                }
            }
        });

        // Poll for result - wrap nxm in Option for single consumption
        let window_clone = window.clone();
        let status_dialog_clone = status_dialog.clone();
        let nxm_opt: Rc<RefCell<Option<NxmLink>>> = Rc::new(RefCell::new(Some(nxm)));
        let api_key_opt: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(Some(api_key)));
        let key_opt: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(Some(key)));
        glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
            if let Some(result) = fetch_result.lock().unwrap().take() {
                status_dialog_clone.close();
                match result {
                    Ok(links) if !links.is_empty() => {
                        let nxm = nxm_opt.borrow_mut().take().unwrap();
                        let api_key = api_key_opt.borrow_mut().take().unwrap();
                        let key = key_opt.borrow_mut().take().unwrap();

                        // Extract original filename from first download link
                        let file_name = Self::extract_filename_from_url(&links[0].uri)
                            .unwrap_or_else(|| format!("{}_{}_{}.zip", nxm.game, nxm.mod_id, nxm.file_id));

                        log::info!("Using filename: {}", file_name);

                        // Check if file already exists
                        if let Some(existing_size) = check_existing_file(&file_name) {
                            log::info!("File {} already exists ({} bytes), prompting user", file_name, existing_size);
                            Self::show_file_exists_dialog(&window_clone, nxm, api_key, key, expires, file_name, existing_size, links);
                        } else {
                            // Proceed with download
                            Self::start_download(&window_clone, nxm, api_key, file_name, links);
                        }
                    }
                    Ok(_) => {
                        log::error!("No download links available");
                        Self::show_error_dialog(&window_clone, "Download Failed", "No download links available from Nexus Mods.");
                    }
                    Err(e) => {
                        log::error!("Failed to fetch download links: {}", e);
                        Self::show_error_dialog(&window_clone, "Download Failed", &format!("Failed to get download link: {}", e));
                    }
                }
                return glib::ControlFlow::Break;
            }
            glib::ControlFlow::Continue
        });
    }

    /// Show dialog when download file already exists
    fn show_file_exists_dialog(
        window: &Window,
        nxm: NxmLink,
        api_key: String,
        _key: String,
        _expires: u64,
        file_name: String,
        existing_size: u64,
        links: Vec<DownloadLink>,
    ) {
        let size_str = if existing_size < 1024 * 1024 {
            format!("{:.1} KB", existing_size as f64 / 1024.0)
        } else {
            format!("{:.1} MB", existing_size as f64 / (1024.0 * 1024.0))
        };

        let dialog = gtk4::AlertDialog::builder()
            .modal(true)
            .message("File Already Exists")
            .detail(format!(
                "The file \"{}\" ({}) already exists in the downloads folder.\n\nWould you like to skip or re-download?",
                file_name, size_str
            ))
            .build();

        dialog.set_buttons(&["Skip", "Re-download"]);
        dialog.set_default_button(0);
        dialog.set_cancel_button(0);

        let window_clone = window.clone();
        dialog.choose(Some(window), gio::Cancellable::NONE, move |response| {
            match response {
                Ok(0) => {
                    // Skip - user chose not to download
                    log::info!("User skipped re-downloading {}", file_name);
                }
                Ok(1) => {
                    // Re-download - delete existing file and start download
                    log::info!("User chose to re-download {}", file_name);
                    if let Err(e) = delete_existing_file(&file_name) {
                        log::error!("Failed to delete existing file: {}", e);
                        return;
                    }
                    Self::start_download(&window_clone, nxm, api_key, file_name, links);
                }
                _ => {
                    // Dialog was dismissed
                    log::debug!("File exists dialog dismissed");
                }
            }
        });
    }

    /// Start the actual download process
    fn start_download(
        window: &Window,
        nxm: NxmLink,
        _api_key: String,
        file_name: String,
        links: Vec<DownloadLink>,
    ) {
        // Create progress dialog
        let dialog = gtk4::Window::builder()
            .title("Downloading Mod")
            .modal(true)
            .transient_for(window)
            .default_width(400)
            .default_height(150)
            .resizable(false)
            .build();

        let content = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
        content.set_margin_top(18);
        content.set_margin_bottom(18);
        content.set_margin_start(18);
        content.set_margin_end(18);

        let status_label = Label::new(Some("Starting download..."));
        content.append(&status_label);

        let progress_bar = ProgressBar::new();
        progress_bar.set_show_text(true);
        content.append(&progress_bar);

        let details_label = Label::new(Some(""));
        details_label.add_css_class("dim-label");
        content.append(&details_label);

        let cancel_button = gtk4::Button::with_label("Cancel");
        cancel_button.set_halign(gtk4::Align::End);
        content.append(&cancel_button);

        dialog.set_child(Some(&content));
        dialog.present();

        // Create shared state for the download
        let download_cancelled = Arc::new(Mutex::new(false));
        let download_cancelled_clone = download_cancelled.clone();

        // Connect cancel button
        cancel_button.connect_clicked(glib::clone!(
            #[weak] dialog,
            move |_| {
                *download_cancelled_clone.lock().unwrap() = true;
                dialog.close();
            }
        ));

        // Clone data for background thread
        let game = nxm.game.clone();
        let mod_id = nxm.mod_id;
        let file_id = nxm.file_id;
        let file_name_thread = file_name.clone();

        // Shared state for progress updates
        let progress_state: Arc<Mutex<Option<DownloadProgress>>> = Arc::new(Mutex::new(None));
        let progress_state_thread = progress_state.clone();
        let download_result: Arc<Mutex<Option<Result<PathBuf, String>>>> = Arc::new(Mutex::new(None));
        let download_result_thread = download_result.clone();

        // Spawn download thread
        std::thread::spawn(move || {
            // Create download manager
            let download_manager = match DownloadManager::new() {
                Ok(dm) => dm,
                Err(e) => {
                    *download_result_thread.lock().unwrap() = Some(Err(format!("Failed to create download manager: {}", e)));
                    return;
                }
            };

            // Get progress reference
            let progress_ref = download_manager.progress();
            let cancel_flag = download_manager.cancel_flag();

            // Monitor cancellation
            let download_cancelled_monitor = download_cancelled.clone();
            std::thread::spawn(move || {
                loop {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    if *download_cancelled_monitor.lock().unwrap() {
                        *cancel_flag.lock().unwrap() = true;
                        break;
                    }
                }
            });

            // Update progress periodically
            let progress_state_update = progress_state_thread.clone();
            let progress_ref_clone = progress_ref.clone();
            std::thread::spawn(move || {
                loop {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    let progress = progress_ref_clone.lock().unwrap().clone();
                    *progress_state_update.lock().unwrap() = Some(progress.clone());
                    if matches!(progress.state, DownloadState::Completed | DownloadState::Failed(_) | DownloadState::Cancelled) {
                        break;
                    }
                }
            });

            // Create metadata
            let metadata = DownloadMetadata {
                file_name: file_name_thread.clone(),
                mod_id,
                file_id,
                game: game.clone(),
                mod_name: None,
                version: None,
                source_url: links[0].uri.clone(),
                size: 0,
                downloaded_at: chrono::Utc::now().timestamp(),
            };

            // Start download
            match download_manager.download(&links, &file_name_thread, metadata) {
                Ok(path) => {
                    *download_result_thread.lock().unwrap() = Some(Ok(path));
                }
                Err(e) => {
                    *download_result_thread.lock().unwrap() = Some(Err(e.to_string()));
                }
            }
        });

        // Poll progress from main thread
        let dialog_clone = dialog.clone();
        glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
            // Check for result
            if let Some(result) = download_result.lock().unwrap().take() {
                dialog_clone.close();
                match result {
                    Ok(path) => {
                        log::info!("Download completed: {:?}", path);
                        // TODO: Show success notification or prompt to install
                    }
                    Err(e) => {
                        log::error!("Download failed: {}", e);
                        // Note: Can't show dialog here as window reference is gone
                    }
                }
                return glib::ControlFlow::Break;
            }

            // Update progress UI
            if let Some(progress) = progress_state.lock().unwrap().as_ref() {
                status_label.set_text(&format!("Downloading: {}", progress.file_name));
                progress_bar.set_fraction(progress.fraction());
                progress_bar.set_text(Some(&format!("{:.0}%", progress.fraction() * 100.0)));
                details_label.set_text(&format!("{} - {}", progress.progress_string(), progress.speed_string()));
            }

            glib::ControlFlow::Continue
        });
    }

    /// Show dialog indicating authentication is required
    fn show_auth_required_dialog(window: &Window) {
        let dialog = gtk4::AlertDialog::builder()
            .modal(true)
            .message("Nexus Mods Authentication Required")
            .detail("You need to connect to Nexus Mods before downloading. Go to Preferences to authenticate.")
            .build();

        dialog.set_buttons(&["Open Preferences", "Cancel"]);
        dialog.set_default_button(0);
        dialog.set_cancel_button(1);

        dialog.choose(Some(window), gio::Cancellable::NONE, move |response| {
            if response == Ok(0) {
                // Open preferences - user will need to do this manually for now
                log::info!("User requested to open preferences for Nexus auth");
            }
        });
    }

    /// Show a generic error dialog
    fn show_error_dialog(window: &Window, title: &str, message: &str) {
        let dialog = gtk4::AlertDialog::builder()
            .modal(true)
            .message(title)
            .detail(message)
            .build();

        dialog.set_buttons(&["OK"]);
        dialog.show(Some(window));
    }

    pub fn run(&self) -> glib::ExitCode {
        self.app.run()
    }
}

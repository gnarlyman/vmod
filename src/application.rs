use gtk4::prelude::*;
use gtk4::{gio, glib, Application};
use std::path::PathBuf;

use crate::config;
use crate::preferences::PreferencesDialog;
use crate::window::VmodWindow;

pub struct VmodApplication {
    app: Application,
}

impl VmodApplication {
    pub fn new() -> Self {
        let app = Application::builder()
            .application_id(config::APP_ID)
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
                eprintln!(
                    "Warning: Schema {} not found. Run 'glib-compile-schemas resources/' first.",
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
                        eprintln!("Failed to open profile folder: {}", e);
                    }
                }
            })
            .build();

        let open_game_folder = gio::ActionEntry::builder("open_game_folder")
            .activate(move |_app: &Application, _, _| {
                if let Ok(profile_list) = crate::profile::profile_data::ProfileList::load() {
                    if let Some(active_profile) = profile_list.get_active_profile() {
                        if let Err(e) = open::that(&active_profile.game_path) {
                            eprintln!("Failed to open game folder: {}", e);
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
                        eprintln!("Failed to open Unity config folder: {}", e);
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
            let window = VmodWindow::new(app);
            window.present();
        });
    }

    pub fn run(&self) -> glib::ExitCode {
        self.app.run()
    }
}

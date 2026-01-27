mod application;
mod config;
mod conflict_panel;
mod logging;
mod mod_entry;
mod mod_list;
mod mods_json_view;
mod nexus_api;
mod nxm;
mod preferences;
mod profile;
mod running_panel;
mod widgets;
mod window;

use application::VmodApplication;
use std::env;

fn main() -> glib::ExitCode {
    // Initialize logging first
    if let Err(e) = logging::init() {
        eprintln!("Failed to initialize logging: {}", e);
    }

    // Set up schema directory for development (before GTK init)
    if let Ok(current_dir) = env::current_dir() {
        let schema_dir = current_dir.join("resources");
        env::set_var("GSETTINGS_SCHEMA_DIR", schema_dir);
    }

    // Initialize GTK
    gtk4::init().expect("Failed to initialize GTK");

    // Create and run application
    let app = VmodApplication::new();
    app.run()
}

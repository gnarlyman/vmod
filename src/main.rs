mod application;
mod config;
mod conflict_panel;
mod mod_entry;
mod mod_list;
mod mods_json_view;
mod preferences;
mod profile;
mod widgets;
mod window;

use application::VmodApplication;
use std::env;

fn main() -> glib::ExitCode {
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

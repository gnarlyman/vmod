mod application;
mod config;
mod preferences;
mod window;

use application::VmodApplication;

fn main() -> glib::ExitCode {
    // Initialize GTK
    gtk4::init().expect("Failed to initialize GTK");

    // Create and run application
    let app = VmodApplication::new();
    app.run()
}

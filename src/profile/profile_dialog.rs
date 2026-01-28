use gtk4::prelude::*;
use gtk4::{
    gio, glib, Box as GtkBox, Button, Entry, FileDialog, Label, Orientation, Window,
};
use std::cell::RefCell;
use std::rc::Rc;

use super::Profile;

pub struct ProfileDialog {
    window: Window,
    result: Rc<RefCell<Option<Profile>>>,
}

impl ProfileDialog {
    pub fn new(parent: &Window) -> Self {
        let window = Window::builder()
            .title("Create New Profile")
            .transient_for(parent)
            .modal(true)
            .default_width(500)
            .default_height(200)
            .build();

        // Create main vertical box
        let vbox = GtkBox::new(Orientation::Vertical, 12);
        vbox.set_margin_top(12);
        vbox.set_margin_bottom(12);
        vbox.set_margin_start(12);
        vbox.set_margin_end(12);

        // Profile name section
        let name_label = Label::new(Some("Profile Name:"));
        name_label.set_halign(gtk4::Align::Start);
        vbox.append(&name_label);

        let name_entry = Entry::new();
        name_entry.set_placeholder_text(Some("My Daggerfall Profile"));
        vbox.append(&name_entry);

        // Game path section
        let game_path_label = Label::new(Some("Game Folder:"));
        game_path_label.set_halign(gtk4::Align::Start);
        game_path_label.set_margin_top(12);
        vbox.append(&game_path_label);

        let game_path_box = GtkBox::new(Orientation::Horizontal, 6);
        let game_path_entry = Entry::new();
        game_path_entry.set_placeholder_text(Some("Select game installation folder"));
        game_path_entry.set_editable(false);
        game_path_entry.set_hexpand(true);
        game_path_box.append(&game_path_entry);

        let browse_button = Button::with_label("Browse...");
        game_path_box.append(&browse_button);
        vbox.append(&game_path_box);

        // Buttons section
        let button_box = GtkBox::new(Orientation::Horizontal, 6);
        button_box.set_halign(gtk4::Align::End);
        button_box.set_margin_top(12);

        let cancel_button = Button::with_label("Cancel");
        button_box.append(&cancel_button);

        let create_button = Button::with_label("Create");
        create_button.add_css_class("suggested-action");
        button_box.append(&create_button);

        vbox.append(&button_box);
        window.set_child(Some(&vbox));

        let game_path = Rc::new(RefCell::new(None));
        let result = Rc::new(RefCell::new(None));

        // Set up browse button click handler
        let window_clone = window.clone();
        let game_path_clone = game_path.clone();
        let game_path_entry_clone = game_path_entry.clone();
        browse_button.connect_clicked(move |_| {
            let file_dialog = FileDialog::new();
            file_dialog.set_title("Select Game Folder");

            let game_path_clone2 = game_path_clone.clone();
            let game_path_entry_clone2 = game_path_entry_clone.clone();

            file_dialog.select_folder(
                Some(&window_clone),
                None::<&gio::Cancellable>,
                move |result| {
                    if let Ok(file) = result {
                        if let Some(path) = file.path() {
                            game_path_entry_clone2.set_text(&path.display().to_string());
                            *game_path_clone2.borrow_mut() = Some(path);
                        }
                    }
                },
            );
        });

        // Set up cancel button
        let window_clone = window.clone();
        cancel_button.connect_clicked(move |_| {
            window_clone.close();
        });

        // Set up create button
        let window_clone = window.clone();
        let name_entry_clone = name_entry.clone();
        let game_path_clone = game_path.clone();
        let result_clone = result.clone();
        create_button.connect_clicked(move |_| {
            let name = name_entry_clone.text().to_string();
            let game_path_opt = game_path_clone.borrow().clone();

            if name.is_empty() {
                // TODO: Show error dialog
                return;
            }

            if let Some(game_path) = game_path_opt {
                // Create profile with auto-detection
                let profile_result = Profile::new_with_auto_detect(name, game_path);

                match profile_result {
                    Ok(profile) => {
                        // Validate game installation
                        match profile.validate_game_installation() {
                            Ok(_) => {
                                *result_clone.borrow_mut() = Some(profile);
                                window_clone.close();
                            }
                            Err(err) => {
                                // TODO: Show error dialog with validation message
                                eprintln!("Validation error: {}", err);
                            }
                        }
                    }
                    Err(err) => {
                        // TODO: Show error dialog with auto-detection error
                        eprintln!("Auto-detection error: {}", err);
                    }
                }
            }
        });

        Self {
            window,
            result,
        }
    }

    pub fn present(&self) {
        self.window.present();
    }

    pub fn connect_close<F>(&self, callback: F)
    where
        F: Fn(Option<Profile>) + 'static,
    {
        let result = self.result.clone();
        self.window.connect_close_request(move |_| {
            callback(result.borrow().clone());
            glib::Propagation::Proceed
        });
    }
}

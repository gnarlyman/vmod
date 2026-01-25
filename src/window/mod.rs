mod imp;

use gtk4::prelude::*;
use gtk4::subclass::prelude::*;
use gtk4::{gio, glib, Application, Box, Button, DropDown, Label, Orientation, StringList, StringObject};

use crate::profile::ProfileDialog;

glib::wrapper! {
    pub struct VmodWindow(ObjectSubclass<imp::VmodWindow>)
        @extends gtk4::ApplicationWindow, gtk4::Window, gtk4::Widget,
        @implements gio::ActionGroup, gio::ActionMap, gtk4::Accessible, gtk4::Buildable,
                    gtk4::ConstraintTarget, gtk4::Native, gtk4::Root, gtk4::ShortcutManager;
}

impl VmodWindow {
    pub fn new(app: &Application) -> Self {
        let window: Self = glib::Object::builder().property("application", app).build();
        window.setup_profile_ui();
        window
    }

    pub fn content_box(&self) -> Box {
        self.imp().content_box.clone()
    }

    pub fn load_window_state(&self) {
        let settings = self.imp().settings.borrow();
        let settings = settings.as_ref().expect("Settings not initialized");

        let width = settings.int("window-width");
        let height = settings.int("window-height");
        let is_maximized = settings.boolean("window-maximized");

        self.set_default_size(width, height);

        if is_maximized {
            self.maximize();
        }
    }

    pub fn save_window_state(&self) {
        let settings = self.imp().settings.borrow();
        let settings = settings.as_ref().expect("Settings not initialized");

        let size = self.default_size();
        settings.set_int("window-width", size.0).ok();
        settings.set_int("window-height", size.1).ok();
        settings.set_boolean("window-maximized", self.is_maximized()).ok();
    }

    fn setup_profile_ui(&self) {
        let content_box = self.content_box();

        // Create profile toolbar
        let profile_box = Box::new(Orientation::Horizontal, 12);
        profile_box.set_margin_top(12);
        profile_box.set_margin_bottom(12);
        profile_box.set_margin_start(12);
        profile_box.set_margin_end(12);

        let profile_label = Label::new(Some("Profile:"));
        profile_box.append(&profile_label);

        // Load profiles
        let profile_list = match crate::profile::profile_data::ProfileList::load() {
            Ok(list) => list,
            Err(e) => {
                eprintln!("Failed to load profiles: {}", e);
                crate::profile::profile_data::ProfileList::new()
            }
        };

        // Create dropdown with profile names
        let profile_names: Vec<String> = profile_list
            .profiles
            .iter()
            .map(|p| p.name.clone())
            .collect();

        let profile_names_str: Vec<&str> = if profile_names.is_empty() {
            vec!["No profiles"]
        } else {
            profile_names.iter().map(|s| s.as_str()).collect()
        };

        let profile_dropdown = DropDown::from_strings(&profile_names_str);
        profile_dropdown.set_hexpand(true);

        if let Some(active) = profile_list.active_profile {
            profile_dropdown.set_selected(active as u32);
        }

        profile_box.append(&profile_dropdown);

        // Store the dropdown reference for later updates
        self.imp().profile_dropdown.replace(Some(profile_dropdown.clone()));

        // Set up dropdown selection handler to save active profile
        profile_dropdown.connect_selected_item_notify(move |dropdown| {
            let selected_index = dropdown.selected();

            // Load profile list and update active profile
            let mut profile_list = match crate::profile::profile_data::ProfileList::load() {
                Ok(list) => list,
                Err(e) => {
                    eprintln!("Failed to load profiles: {}", e);
                    return;
                }
            };

            // Check if there are profiles and the selection is valid
            if profile_list.profiles.is_empty() {
                return;
            }

            // Update active profile and save
            profile_list.set_active_profile(selected_index as usize);
            if let Err(e) = profile_list.save() {
                eprintln!("Failed to save active profile: {}", e);
            }
        });

        // Create new profile button
        let new_profile_button = Button::with_label("New Profile...");
        profile_box.append(&new_profile_button);

        // Add profile box to content
        content_box.prepend(&profile_box);

        // Set up new profile button click handler
        let window_weak = self.downgrade();
        new_profile_button.connect_clicked(move |_| {
            let window = match window_weak.upgrade() {
                Some(w) => w,
                None => return,
            };

            let dialog = ProfileDialog::new(window.upcast_ref::<gtk4::Window>());

            let window_clone = window.clone();
            dialog.connect_close(move |result| {
                if let Some(profile) = result {
                    let mut profile_list = match crate::profile::profile_data::ProfileList::load() {
                        Ok(list) => list,
                        Err(_) => crate::profile::profile_data::ProfileList::new(),
                    };

                    profile_list.add_profile(profile);

                    if let Err(e) = profile_list.save() {
                        eprintln!("Failed to save profiles: {}", e);
                    } else {
                        // Reload profile UI
                        window_clone.refresh_profile_ui();
                    }
                }
            });

            dialog.present();
        });
    }

    fn refresh_profile_ui(&self) {
        // Load the updated profile list from disk
        let profile_list = match crate::profile::profile_data::ProfileList::load() {
            Ok(list) => list,
            Err(e) => {
                eprintln!("Failed to load profiles: {}", e);
                return;
            }
        };

        // Get the dropdown reference
        let dropdown_ref = self.imp().profile_dropdown.borrow();
        let dropdown = match dropdown_ref.as_ref() {
            Some(d) => d,
            None => {
                eprintln!("Profile dropdown not initialized");
                return;
            }
        };

        // Create a new StringList with updated profile names
        let profile_names: Vec<String> = profile_list
            .profiles
            .iter()
            .map(|p| p.name.clone())
            .collect();

        let string_list = if profile_names.is_empty() {
            StringList::new(&["No profiles"])
        } else {
            StringList::new(&profile_names.iter().map(|s| s.as_str()).collect::<Vec<&str>>())
        };

        // Update the dropdown's model
        dropdown.set_model(Some(&string_list));

        // Set the expression to display the "string" property of StringObject items
        let expression = StringObject::this_expression("string");
        dropdown.set_expression(Some(&expression));

        // Set the selected index to the newly added profile (last one)
        if let Some(active) = profile_list.active_profile {
            dropdown.set_selected(active as u32);
        }
    }
}

use gtk4::prelude::*;
use gtk4::subclass::prelude::*;
use gtk4::{glib, Label, Button, Box, Orientation, Align, Entry, PasswordEntry};
use std::cell::RefCell;
use std::sync::{Arc, Mutex};

use crate::nexus_api::{NexusClient, NexusConfig};

#[derive(Debug, Default)]
pub struct PreferencesDialog {
    nexus_status_label: RefCell<Option<Label>>,
    nexus_api_key_entry: RefCell<Option<PasswordEntry>>,
    nexus_save_button: RefCell<Option<Button>>,
    nexus_disconnect_button: RefCell<Option<Button>>,
}

#[glib::object_subclass]
impl ObjectSubclass for PreferencesDialog {
    const NAME: &'static str = "PreferencesDialog";
    type Type = super::PreferencesDialog;
    type ParentType = gtk4::Window;
}

impl ObjectImpl for PreferencesDialog {
    fn constructed(&self) {
        self.parent_constructed();

        let obj = self.obj();
        obj.set_title(Some("Preferences"));
        obj.set_modal(true);
        obj.set_default_size(500, 400);

        // Create main vertical box
        let vbox = Box::new(Orientation::Vertical, 18);
        vbox.set_margin_top(20);
        vbox.set_margin_bottom(20);
        vbox.set_margin_start(20);
        vbox.set_margin_end(20);

        // Nexus Mods section
        let nexus_section = self.build_nexus_section();
        vbox.append(&nexus_section);

        // Spacer
        let spacer = Box::new(Orientation::Vertical, 0);
        spacer.set_vexpand(true);
        vbox.append(&spacer);

        // Close button
        let button = Button::with_label("Close");
        button.set_halign(Align::End);
        button.connect_clicked(glib::clone!(
            #[weak]
            obj,
            move |_| {
                obj.close();
            }
        ));
        vbox.append(&button);

        obj.set_child(Some(&vbox));

        // Load initial state
        self.update_nexus_status();
    }
}

impl PreferencesDialog {
    fn build_nexus_section(&self) -> Box {
        let section = Box::new(Orientation::Vertical, 12);

        // Section header
        let header = Label::new(Some("Nexus Mods"));
        header.set_halign(Align::Start);
        header.add_css_class("heading");
        section.append(&header);

        // Status row
        let status_label = Label::new(Some("Not connected"));
        status_label.set_halign(Align::Start);
        *self.nexus_status_label.borrow_mut() = Some(status_label.clone());
        section.append(&status_label);

        // API Key entry row
        let api_key_box = Box::new(Orientation::Horizontal, 8);

        let api_key_label = Label::new(Some("API Key:"));
        api_key_label.set_width_chars(10);
        api_key_label.set_halign(Align::Start);
        api_key_box.append(&api_key_label);

        let api_key_entry = PasswordEntry::new();
        api_key_entry.set_show_peek_icon(true);
        api_key_entry.set_hexpand(true);
        api_key_entry.set_placeholder_text(Some("Paste your API key here"));
        api_key_box.append(&api_key_entry);
        *self.nexus_api_key_entry.borrow_mut() = Some(api_key_entry.clone());

        section.append(&api_key_box);

        // Button row
        let button_row = Box::new(Orientation::Horizontal, 8);
        button_row.set_halign(Align::Start);

        // Get API Key button (opens browser)
        let get_key_button = Button::with_label("Get API Key");
        get_key_button.set_tooltip_text(Some("Open Nexus Mods to generate an API key"));
        get_key_button.connect_clicked(|_| {
            let url = "https://www.nexusmods.com/users/myaccount?tab=api";
            if let Err(e) = open::that(url) {
                log::error!("Failed to open browser: {}", e);
            }
        });
        button_row.append(&get_key_button);

        // Save button
        let save_button = Button::with_label("Save & Validate");
        save_button.add_css_class("suggested-action");
        save_button.connect_clicked(glib::clone!(
            #[weak(rename_to = dialog)]
            self,
            move |btn| {
                dialog.save_api_key(btn);
            }
        ));
        button_row.append(&save_button);
        *self.nexus_save_button.borrow_mut() = Some(save_button);

        // Disconnect button
        let disconnect_button = Button::with_label("Disconnect");
        disconnect_button.add_css_class("destructive-action");
        disconnect_button.set_visible(false);
        disconnect_button.connect_clicked(glib::clone!(
            #[weak(rename_to = dialog)]
            self,
            move |_| {
                dialog.disconnect_nexus();
            }
        ));
        button_row.append(&disconnect_button);
        *self.nexus_disconnect_button.borrow_mut() = Some(disconnect_button);

        section.append(&button_row);

        // Help text
        let help_label = Label::new(Some(
            "To download mods from Nexus Mods:\n\
             1. Click \"Get API Key\" to open Nexus Mods\n\
             2. Log in and generate a Personal API Key\n\
             3. Copy the key and paste it above\n\
             4. Click \"Save & Validate\""
        ));
        help_label.set_halign(Align::Start);
        help_label.set_wrap(true);
        help_label.add_css_class("dim-label");
        section.append(&help_label);

        section
    }

    fn update_nexus_status(&self) {
        let config = NexusConfig::load();

        let status_label = self.nexus_status_label.borrow();
        let api_key_entry = self.nexus_api_key_entry.borrow();
        let save_button = self.nexus_save_button.borrow();
        let disconnect_button = self.nexus_disconnect_button.borrow();

        if config.has_api_key() {
            let status_text = if let Some(name) = &config.user_name {
                let premium_str = if config.is_premium { " (Premium)" } else { "" };
                format!("Connected as {}{}", name, premium_str)
            } else {
                "Connected".to_string()
            };

            if let Some(label) = status_label.as_ref() {
                label.set_text(&status_text);
                label.remove_css_class("error");
                label.add_css_class("success");
            }

            // Hide entry and save button when connected
            if let Some(entry) = api_key_entry.as_ref() {
                entry.set_visible(false);
            }
            if let Some(btn) = save_button.as_ref() {
                btn.set_visible(false);
            }
            if let Some(btn) = disconnect_button.as_ref() {
                btn.set_visible(true);
            }
        } else {
            if let Some(label) = status_label.as_ref() {
                label.set_text("Not connected");
                label.remove_css_class("success");
                label.remove_css_class("error");
            }

            // Show entry and save button when not connected
            if let Some(entry) = api_key_entry.as_ref() {
                entry.set_visible(true);
                entry.set_text("");
            }
            if let Some(btn) = save_button.as_ref() {
                btn.set_visible(true);
            }
            if let Some(btn) = disconnect_button.as_ref() {
                btn.set_visible(false);
            }
        }
    }

    fn save_api_key(&self, button: &Button) {
        let api_key = {
            let entry = self.nexus_api_key_entry.borrow();
            entry.as_ref().map(|e| e.text().to_string()).unwrap_or_default()
        };

        if api_key.trim().is_empty() {
            if let Some(label) = self.nexus_status_label.borrow().as_ref() {
                label.set_text("Please enter an API key");
                label.add_css_class("error");
            }
            return;
        }

        button.set_sensitive(false);
        button.set_label("Validating...");

        if let Some(label) = self.nexus_status_label.borrow().as_ref() {
            label.set_text("Validating API key...");
            label.remove_css_class("error");
            label.remove_css_class("success");
        }

        // Validate in background thread
        let api_key_clone = api_key.trim().to_string();
        let validation_result: Arc<Mutex<Option<Result<(String, u64, bool), String>>>> = Arc::new(Mutex::new(None));
        let validation_result_thread = validation_result.clone();

        std::thread::spawn(move || {
            let config = NexusConfig::load();
            match NexusClient::new(api_key_clone.clone(), config.game_domain.clone()) {
                Ok(client) => {
                    match client.validate_key() {
                        Ok(response) => {
                            let user = response.data;
                            *validation_result_thread.lock().unwrap() = Some(Ok((
                                user.name,
                                user.user_id,
                                user.is_premium,
                            )));
                        }
                        Err(e) => {
                            *validation_result_thread.lock().unwrap() = Some(Err(e.to_string()));
                        }
                    }
                }
                Err(e) => {
                    *validation_result_thread.lock().unwrap() = Some(Err(e.to_string()));
                }
            }
        });

        // Poll for result
        let status_label = self.nexus_status_label.borrow().clone();
        let api_key_entry = self.nexus_api_key_entry.borrow().clone();
        let save_button = self.nexus_save_button.borrow().clone();
        let disconnect_button = self.nexus_disconnect_button.borrow().clone();
        let api_key_to_save = api_key.trim().to_string();

        glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
            if let Some(result) = validation_result.lock().unwrap().take() {
                match result {
                    Ok((name, user_id, is_premium)) => {
                        log::info!("API key validated for user: {} (premium: {})", name, is_premium);

                        // Save credentials
                        let mut config = NexusConfig::load();
                        config.set_credentials(
                            api_key_to_save.clone(),
                            Some(name.clone()),
                            Some(user_id),
                            is_premium,
                        );
                        if let Err(e) = config.save() {
                            log::error!("Failed to save config: {}", e);
                        }

                        let premium_str = if is_premium { " (Premium)" } else { "" };
                        if let Some(label) = status_label.as_ref() {
                            label.set_text(&format!("Connected as {}{}", name, premium_str));
                            label.remove_css_class("error");
                            label.add_css_class("success");
                        }

                        // Hide entry and save, show disconnect
                        if let Some(entry) = api_key_entry.as_ref() {
                            entry.set_visible(false);
                        }
                        if let Some(btn) = save_button.as_ref() {
                            btn.set_visible(false);
                            btn.set_sensitive(true);
                            btn.set_label("Save & Validate");
                        }
                        if let Some(btn) = disconnect_button.as_ref() {
                            btn.set_visible(true);
                        }
                    }
                    Err(e) => {
                        log::error!("API key validation failed: {}", e);
                        if let Some(label) = status_label.as_ref() {
                            label.set_text(&format!("Invalid API key: {}", e));
                            label.add_css_class("error");
                        }
                        if let Some(btn) = save_button.as_ref() {
                            btn.set_sensitive(true);
                            btn.set_label("Save & Validate");
                        }
                    }
                }

                return glib::ControlFlow::Break;
            }

            glib::ControlFlow::Continue
        });
    }

    fn disconnect_nexus(&self) {
        let mut config = NexusConfig::load();
        config.clear_credentials();
        if let Err(e) = config.save() {
            log::error!("Failed to save config: {}", e);
        }

        self.update_nexus_status();
        log::info!("Disconnected from Nexus Mods");
    }
}

impl WidgetImpl for PreferencesDialog {}
impl WindowImpl for PreferencesDialog {}

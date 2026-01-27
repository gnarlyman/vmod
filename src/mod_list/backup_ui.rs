//! Backup/restore popover UI.

use gtk4::prelude::*;
use gtk4::{Box, Button, Entry, Label, Orientation, ScrolledWindow};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use crate::mod_entry::BackupManager;
use super::imp::ModListView;

impl ModListView {
    /// Show backup/restore popover
    pub fn show_backup_popover(
        btn: &Button,
        profile_name: &Rc<RefCell<Option<String>>>,
        profile_path: &Rc<RefCell<Option<PathBuf>>>,
        widget: &crate::mod_list::ModListView,
    ) {
        let profile_name_opt = profile_name.borrow().clone();
        let Some(profile_name_str) = profile_name_opt else {
            eprintln!("No profile selected");
            return;
        };

        let profile_path_opt = profile_path.borrow().clone();
        let Some(profile_path_buf) = profile_path_opt else {
            eprintln!("Profile path not set");
            return;
        };

        // Create the main popover
        let popover = gtk4::Popover::new();
        let main_box = Box::new(Orientation::Vertical, 6);
        main_box.set_margin_top(6);
        main_box.set_margin_bottom(6);
        main_box.set_margin_start(6);
        main_box.set_margin_end(6);

        // Create Backup button
        let create_btn = Button::with_label("Create Backup");
        create_btn.add_css_class("flat");

        // Restore Backup button
        let restore_btn = Button::with_label("Restore Backup");
        restore_btn.add_css_class("flat");

        main_box.append(&create_btn);
        main_box.append(&restore_btn);

        // Create Backup handler
        let popover_for_create = popover.clone();
        let profile_name_for_create = profile_name_str.clone();
        let profile_path_for_create = profile_path_buf.clone();
        create_btn.connect_clicked(move |btn| {
            Self::show_create_backup_view(btn, &popover_for_create, &profile_name_for_create, &profile_path_for_create);
        });

        // Restore Backup handler
        let popover_for_restore = popover.clone();
        let profile_name_for_restore = profile_name_str.clone();
        let profile_path_for_restore = profile_path_buf.clone();
        let widget_for_restore = widget.clone();
        restore_btn.connect_clicked(move |btn| {
            Self::show_restore_backup_view(btn, &popover_for_restore, &profile_name_for_restore, &profile_path_for_restore, &widget_for_restore);
        });

        popover.set_child(Some(&main_box));
        popover.set_parent(btn);
        popover.popup();
    }

    /// Show the create backup UI
    pub fn show_create_backup_view(
        _btn: &Button,
        popover: &gtk4::Popover,
        profile_name: &str,
        profile_path: &PathBuf,
    ) {
        let Ok(backup_manager) = BackupManager::new(profile_name) else {
            eprintln!("Failed to create BackupManager");
            return;
        };

        // Replace popover content with create form
        let create_box = Box::new(Orientation::Vertical, 6);
        create_box.set_margin_top(6);
        create_box.set_margin_bottom(6);
        create_box.set_margin_start(6);
        create_box.set_margin_end(6);

        let label = Label::new(Some("Backup Name:"));
        label.set_xalign(0.0);
        create_box.append(&label);

        let entry = Entry::new();
        entry.set_text(&backup_manager.get_default_name());
        entry.set_width_chars(25);
        create_box.append(&entry);

        let button_row = Box::new(Orientation::Horizontal, 6);
        button_row.set_halign(gtk4::Align::End);

        let cancel_btn = Button::with_label("Cancel");
        cancel_btn.add_css_class("flat");
        let create_btn = Button::with_label("Create");
        create_btn.add_css_class("suggested-action");

        button_row.append(&cancel_btn);
        button_row.append(&create_btn);
        create_box.append(&button_row);

        // Cancel handler
        let popover_for_cancel = popover.clone();
        cancel_btn.connect_clicked(move |_| {
            popover_for_cancel.popdown();
        });

        // Create handler
        let popover_for_create = popover.clone();
        let profile_name_owned = profile_name.to_string();
        let profile_path_owned = profile_path.clone();
        let entry_clone = entry.clone();
        create_btn.connect_clicked(move |_| {
            let backup_name = entry_clone.text().to_string();
            if backup_name.is_empty() {
                return;
            }

            let Ok(manager) = BackupManager::new(&profile_name_owned) else {
                eprintln!("Failed to create BackupManager");
                return;
            };

            // mod_state.json is at profile level (parent of mods folder)
            let profile_dir = dirs::config_dir()
                .map(|d| d.join("vmod").join("profiles").join(&profile_name_owned));
            let Some(profile_dir) = profile_dir else {
                eprintln!("Could not find config directory");
                return;
            };
            let mod_state_path = profile_dir.join("mod_state.json");
            // sections.json is in the mods folder (profile_path)
            let sections_path = profile_path_owned.join("sections.json");

            match manager.create_backup(&backup_name, &mod_state_path, &sections_path) {
                Ok(backup_path) => {
                    eprintln!("Backup created at: {:?}", backup_path);
                    popover_for_create.popdown();
                }
                Err(e) => {
                    eprintln!("Failed to create backup: {}", e);
                }
            }
        });

        // Also allow Enter key to create
        let create_btn_for_activate = create_btn.clone();
        entry.connect_activate(move |_| {
            create_btn_for_activate.emit_clicked();
        });

        popover.set_child(Some(&create_box));
    }

    /// Show the restore backup UI
    pub fn show_restore_backup_view(
        _btn: &Button,
        popover: &gtk4::Popover,
        profile_name: &str,
        profile_path: &PathBuf,
        widget: &crate::mod_list::ModListView,
    ) {
        let Ok(backup_manager) = BackupManager::new(profile_name) else {
            eprintln!("Failed to create BackupManager");
            return;
        };

        let backups = backup_manager.list_backups().unwrap_or_default();

        // Replace popover content with restore list
        let restore_box = Box::new(Orientation::Vertical, 6);
        restore_box.set_margin_top(6);
        restore_box.set_margin_bottom(6);
        restore_box.set_margin_start(6);
        restore_box.set_margin_end(6);

        if backups.is_empty() {
            let label = Label::new(Some("No backups found"));
            label.add_css_class("dim-label");
            restore_box.append(&label);
        } else {
            let label = Label::new(Some("Select backup to restore:"));
            label.set_xalign(0.0);
            restore_box.append(&label);

            // Create a scrolled window for the list
            let scrolled = ScrolledWindow::new();
            scrolled.set_min_content_height(200);
            scrolled.set_min_content_width(250);

            let list_box = gtk4::ListBox::new();
            list_box.set_selection_mode(gtk4::SelectionMode::Single);
            list_box.add_css_class("boxed-list");

            // Store backup names for lookup by index
            let backup_names: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));

            // Add backups (limit to 10)
            for backup in backups.iter().take(10) {
                backup_names.borrow_mut().push(backup.name.clone());

                let row = gtk4::ListBoxRow::new();
                let row_box = Box::new(Orientation::Vertical, 2);
                row_box.set_margin_top(6);
                row_box.set_margin_bottom(6);
                row_box.set_margin_start(6);
                row_box.set_margin_end(6);

                let name_label = Label::new(Some(&backup.name));
                name_label.set_xalign(0.0);
                name_label.add_css_class("heading");

                // Format the date
                let date_str = if let Ok(duration) = backup.created_at.elapsed() {
                    let secs = duration.as_secs();
                    if secs < 60 {
                        "Just now".to_string()
                    } else if secs < 3600 {
                        format!("{} minutes ago", secs / 60)
                    } else if secs < 86400 {
                        format!("{} hours ago", secs / 3600)
                    } else {
                        format!("{} days ago", secs / 86400)
                    }
                } else {
                    "Unknown".to_string()
                };

                let date_label = Label::new(Some(&date_str));
                date_label.set_xalign(0.0);
                date_label.add_css_class("dim-label");

                row_box.append(&name_label);
                row_box.append(&date_label);
                row.set_child(Some(&row_box));

                list_box.append(&row);
            }

            scrolled.set_child(Some(&list_box));
            restore_box.append(&scrolled);

            // Connect row activation (double-click or Enter)
            let popover_for_restore = popover.clone();
            let profile_name_for_restore = profile_name.to_string();
            let profile_path_for_restore = profile_path.clone();
            let widget_for_restore = widget.clone();
            list_box.connect_row_activated(move |_, row| {
                let index = row.index();
                if index < 0 {
                    return;
                }

                let backup_name = backup_names.borrow().get(index as usize).cloned();
                if let Some(name) = backup_name {
                    let Ok(manager) = BackupManager::new(&profile_name_for_restore) else {
                        eprintln!("Failed to create BackupManager");
                        return;
                    };

                    // mod_state.json is at profile level (parent of mods folder)
                    let profile_dir = dirs::config_dir()
                        .map(|d| d.join("vmod").join("profiles").join(&profile_name_for_restore));
                    let Some(profile_dir) = profile_dir else {
                        eprintln!("Could not find config directory");
                        return;
                    };
                    let mod_state_dest = profile_dir.join("mod_state.json");
                    // sections.json is in the mods folder (profile_path)
                    let sections_dest = profile_path_for_restore.join("sections.json");

                    match manager.restore_backup(&name, &mod_state_dest, &sections_dest) {
                        Ok(()) => {
                            eprintln!("Backup '{}' restored successfully", name);
                            popover_for_restore.popdown();

                            // Reload the mod list to reflect restored state
                            widget_for_restore.reload();
                        }
                        Err(e) => {
                            eprintln!("Failed to restore backup: {}", e);
                        }
                    }
                }
            });
        }

        // Cancel button
        let cancel_btn = Button::with_label("Cancel");
        cancel_btn.add_css_class("flat");
        cancel_btn.set_halign(gtk4::Align::End);

        let popover_for_cancel = popover.clone();
        cancel_btn.connect_clicked(move |_| {
            popover_for_cancel.popdown();
        });

        restore_box.append(&cancel_btn);

        popover.set_child(Some(&restore_box));
    }
}

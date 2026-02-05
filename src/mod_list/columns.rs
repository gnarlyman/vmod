//! Column factory definitions for the ColumnView.

use gtk4::prelude::*;
use gtk4::{
    gio, glib, Box, Button, CheckButton, ColumnView, ColumnViewColumn, EditableLabel,
    EventControllerMotion, FilterChange, GestureClick, Label, Orientation, PopoverMenu,
    SignalListItemFactory,
};

use crate::mod_entry::{ModEntry, SectionHeader};
use super::imp::ModListView;
use super::reordering::{move_mod_to_section_static, remove_section_static};
use super::vfs_state::save_mod_state_static;

impl ModListView {
    pub fn add_checkbox_column(&self, column_view: &ColumnView, _settings: &gio::Settings) {
        let factory = SignalListItemFactory::new();

        // Setup: Create a Box that can hold either CheckButton or expander Button
        factory.connect_setup(move |_factory, item| {
            let list_item = item.downcast_ref::<gtk4::ListItem>()
                .expect("Item must be ListItem");
            let container = Box::new(Orientation::Horizontal, 0);
            list_item.set_child(Some(&container));
        });

        // Bind: Connect the CheckButton to the ModEntry's enabled property
        let model_ref = self.model.clone();
        let profile_name_ref = self.profile_name.clone();
        let collapsed_sections = self.collapsed_sections.clone();
        let filter_ref = self.filter.clone();
        let sections_config_for_checkbox = self.sections_config.clone();
        let profile_path_for_checkbox = self.profile_path.clone();
        factory.connect_bind(move |_factory, item| {
            let list_item = item.downcast_ref::<gtk4::ListItem>()
                .expect("Item must be ListItem");

            let container = list_item
                .child()
                .and_downcast::<Box>()
                .expect("Child must be Box");

            // Clear any previous children
            while let Some(child) = container.first_child() {
                container.remove(&child);
            }

            // Check if this is a section header or mod entry
            if let Some(section) = list_item.item().and_downcast::<SectionHeader>() {
                // Create expand/collapse button for section
                let is_expanded = !collapsed_sections.borrow().contains(&section.section_id());
                let icon_name = if is_expanded { "pan-down-symbolic" } else { "pan-end-symbolic" };
                let button = Button::from_icon_name(icon_name);
                button.add_css_class("flat");

                let collapsed_clone = collapsed_sections.clone();
                let section_id = section.section_id();
                let filter_clone = filter_ref.clone();
                let sections_config_clone = sections_config_for_checkbox.clone();
                let profile_path_clone = profile_path_for_checkbox.clone();
                let handler_id = button.connect_clicked(move |btn| {
                    let mut collapsed = collapsed_clone.borrow_mut();
                    let is_expanding = collapsed.contains(&section_id);
                    if is_expanding {
                        collapsed.remove(&section_id);
                        btn.set_icon_name("pan-down-symbolic");
                    } else {
                        collapsed.insert(section_id.clone());
                        btn.set_icon_name("pan-end-symbolic");
                    }
                    drop(collapsed);

                    // Persist expanded state
                    sections_config_clone.borrow_mut().update_section_expanded(&section_id, is_expanding);
                    if let Some(path) = profile_path_clone.borrow().as_ref() {
                        let _ = sections_config_clone.borrow().save(path);
                    }

                    // Trigger filter update
                    if let Some(filter) = filter_clone.borrow().as_ref() {
                        filter.changed(FilterChange::Different);
                    }
                });

                container.append(&button);

                unsafe {
                    list_item.set_data("is-section", true);
                    list_item.set_data("section-handler-id", handler_id);
                }
            } else if let Some(mod_entry) = list_item.item().and_downcast::<ModEntry>() {
                // Create checkbox for mod entry
                let check_button = CheckButton::new();

                // Bind the enabled property and store the binding
                let binding = mod_entry
                    .bind_property("enabled", &check_button, "active")
                    .bidirectional()
                    .sync_create()
                    .build();

                // Connect to toggled signal to save state
                let model_clone = model_ref.clone();
                let profile_name_clone = profile_name_ref.clone();
                let handler_id = check_button.connect_toggled(move |_btn| {
                    // Save mod state (VFS rebuild happens on Apply button)
                    save_mod_state_static(&model_clone, &profile_name_clone);
                });

                container.append(&check_button);

                unsafe {
                    list_item.set_data("is-section", false);
                    list_item.set_data("binding", binding);
                    list_item.set_data("handler-id", handler_id);
                }
            }
        });

        // Unbind: Clean up bindings and signal handlers
        factory.connect_unbind(move |_factory, item| {
            let list_item = item.downcast_ref::<gtk4::ListItem>()
                .expect("Item must be ListItem");

            let is_section: bool = unsafe {
                list_item.steal_data::<bool>("is-section").unwrap_or(false)
            };

            if is_section {
                // Clean up section button handler
                if let Some(container) = list_item.child().and_downcast::<Box>() {
                    if let Some(button) = container.first_child().and_downcast::<Button>() {
                        unsafe {
                            if let Some(handler_id) = list_item.steal_data::<glib::SignalHandlerId>("section-handler-id") {
                                button.disconnect(handler_id);
                            }
                        }
                    }
                }
            } else {
                // Unbind the property binding
                unsafe {
                    if let Some(binding) = list_item.steal_data::<glib::Binding>("binding") {
                        binding.unbind();
                    }
                }

                // Disconnect the checkbox handler
                if let Some(container) = list_item.child().and_downcast::<Box>() {
                    if let Some(check_button) = container.first_child().and_downcast::<CheckButton>() {
                        unsafe {
                            if let Some(handler_id) = list_item.steal_data::<glib::SignalHandlerId>("handler-id") {
                                check_button.disconnect(handler_id);
                            }
                        }
                    }
                }
            }
        });

        let column = ColumnViewColumn::new(Some("On"), Some(factory));
        column.set_resizable(false);
        column.set_fixed_width(35);
        column_view.append_column(&column);
    }

    pub fn add_name_column(&self, column_view: &ColumnView, _settings: &gio::Settings) {
        let factory = SignalListItemFactory::new();

        // Setup: Create a Box container that can hold either Label or EditableLabel
        factory.connect_setup(move |_factory, item| {
            let list_item = item.downcast_ref::<gtk4::ListItem>()
                .expect("Item must be ListItem");
            let container = Box::new(Orientation::Horizontal, 0);
            list_item.set_child(Some(&container));
        });

        // Clone refs needed for bind closure
        let sections_config_ref = self.sections_config.clone();
        let profile_path_ref = self.profile_path.clone();
        let model_ref = self.model.clone();
        let filter_ref = self.filter.clone();
        let profile_name_ref = self.profile_name.clone();

        factory.connect_bind(move |_factory, item| {
            let list_item = item.downcast_ref::<gtk4::ListItem>()
                .expect("Item must be ListItem");

            let container = list_item
                .child()
                .and_downcast::<Box>()
                .expect("Child must be Box");

            // Clear any previous children
            while let Some(child) = container.first_child() {
                container.remove(&child);
            }

            // Check if this is a section header or mod entry
            if let Some(section) = list_item.item().and_downcast::<SectionHeader>() {
                // Section header: use EditableLabel for inline editing
                let editable = EditableLabel::new(&section.name());
                editable.add_css_class("heading");
                editable.set_hexpand(true);
                editable.set_halign(gtk4::Align::Fill);

                // Connect to changed signal to save rename when editing completes
                let section_clone = section.clone();
                let sections_config_clone = sections_config_ref.clone();
                let profile_path_clone = profile_path_ref.clone();
                let handler_id = editable.connect_changed(move |label| {
                    let new_name = label.text().to_string();
                    if !new_name.is_empty() {
                        // Update the section's name property
                        section_clone.set_name(new_name.clone());
                        // Update the config and save
                        sections_config_clone.borrow_mut().rename_section(&section_clone.section_id(), &new_name);
                        if let Some(path) = profile_path_clone.borrow().as_ref() {
                            let _ = sections_config_clone.borrow().save(path);
                        }
                    }
                });

                container.append(&editable);

                // Create delete button (hidden by default)
                let delete_button = Button::from_icon_name("user-trash-symbolic");
                delete_button.add_css_class("flat");
                delete_button.set_visible(false);
                container.append(&delete_button);

                // Connect delete button click
                let section_id = section.section_id();
                let model_clone = model_ref.clone();
                let sections_config_clone2 = sections_config_ref.clone();
                let profile_path_clone2 = profile_path_ref.clone();
                let filter_clone = filter_ref.clone();
                let delete_handler_id = delete_button.connect_clicked(move |_btn| {
                    remove_section_static(
                        &model_clone,
                        &section_id,
                        &sections_config_clone2,
                        &profile_path_clone2,
                        &filter_clone,
                    );
                });

                // Add hover controller to show/hide delete button
                let motion_controller = EventControllerMotion::new();
                let delete_button_enter = delete_button.clone();
                motion_controller.connect_enter(move |_controller, _x, _y| {
                    delete_button_enter.set_visible(true);
                });
                let delete_button_leave = delete_button.clone();
                motion_controller.connect_leave(move |_controller| {
                    delete_button_leave.set_visible(false);
                });
                container.add_controller(motion_controller.clone());

                unsafe {
                    list_item.set_data("is-section-name", true);
                    list_item.set_data("editable-handler-id", handler_id);
                    list_item.set_data("delete-handler-id", delete_handler_id);
                    list_item.set_data("motion-controller", motion_controller);
                }
            } else if let Some(mod_entry) = list_item.item().and_downcast::<ModEntry>() {
                // Regular mod entry: use Label (not editable)
                let label = Label::new(None);
                label.set_xalign(0.0);
                label.set_hexpand(true);

                let binding = mod_entry
                    .bind_property("name", &label, "label")
                    .sync_create()
                    .build();

                // Show display name in tooltip when it differs from folder name
                let folder_name = mod_entry.name();
                let display = mod_entry.display_name();
                if folder_name != display {
                    label.set_tooltip_text(Some(&display));
                }

                container.append(&label);

                // Create right-click gesture for context menu
                let gesture = GestureClick::new();
                gesture.set_button(3); // Secondary button (right-click)

                // Clone refs needed for the gesture closure
                let sections_config_for_menu = sections_config_ref.clone();
                let model_for_menu = model_ref.clone();
                let profile_name_for_menu = profile_name_ref.clone();
                let profile_path_for_menu = profile_path_ref.clone();
                let mod_path = mod_entry.path();
                let mod_folder_name = mod_entry.name();
                let mod_nexus_id = mod_entry.nexus_id();
                let mod_version = mod_entry.version();

                gesture.connect_pressed(move |gesture, _n_press, x, y| {
                    // Build menu dynamically from current sections
                    let menu = gio::Menu::new();
                    let send_section = gio::Menu::new();

                    let sections = sections_config_for_menu.borrow();
                    for section_data in &sections.sections {
                        let item = gio::MenuItem::new(
                            Some(&section_data.name),
                            Some(&format!("mod.send-to-section::{}", section_data.section_id)),
                        );
                        send_section.append_item(&item);
                    }
                    drop(sections);

                    menu.append_submenu(Some("Send to Section"), &send_section);

                    // Add "Check Version" if mod has nexus_id
                    if mod_nexus_id.is_some() {
                        menu.append(Some("Check Version"), Some("mod.check-version"));
                    }

                    // Create popover menu
                    let popover = PopoverMenu::from_model(Some(&menu));
                    if let Some(widget) = gesture.widget() {
                        popover.set_parent(&widget);
                    }
                    popover.set_has_arrow(false);

                    // Position at click location
                    let rect = gtk4::gdk::Rectangle::new(x as i32, y as i32, 1, 1);
                    popover.set_pointing_to(Some(&rect));

                    // Create action group for section actions
                    let action_group = gio::SimpleActionGroup::new();
                    let send_action = gio::SimpleAction::new(
                        "send-to-section",
                        Some(&String::static_variant_type()),
                    );

                    // Clone for action closure
                    let model_clone = model_for_menu.clone();
                    let profile_name_clone = profile_name_for_menu.clone();
                    let profile_path_clone = profile_path_for_menu.clone();
                    let sections_config_clone = sections_config_for_menu.clone();
                    let mod_path_clone = mod_path.clone();
                    let popover_clone = popover.clone();

                    send_action.connect_activate(move |_action, param| {
                        if let Some(section_id) = param.and_then(|p| p.get::<String>()) {
                            move_mod_to_section_static(
                                &model_clone,
                                mod_path_clone.as_path(),
                                &section_id,
                                &sections_config_clone,
                                &profile_name_clone,
                                &profile_path_clone,
                            );
                        }
                        popover_clone.popdown();
                    });

                    action_group.add_action(&send_action);

                    // Add check-version action if mod has nexus_id
                    if let Some(ref nexus_id) = mod_nexus_id {
                        let check_version_action = gio::SimpleAction::new("check-version", None);
                        let model_for_version = model_for_menu.clone();
                        let folder_name_clone = mod_folder_name.clone();
                        let mod_path_for_version = mod_path.clone();
                        let nexus_id_clone = nexus_id.clone();
                        let version_clone = mod_version.clone();
                        let popover_for_version = popover.clone();

                        check_version_action.connect_activate(move |_action, _param| {
                            log::info!("Check Version action triggered for: {}", folder_name_clone);
                            let nexus_config = crate::nexus_api::NexusConfig::load();
                            if let Some(api_key) = nexus_config.api_key {
                                log::info!("API key found, calling check_single_mod_version");
                                super::imp::ModListView::check_single_mod_version(
                                    &model_for_version,
                                    folder_name_clone.clone(),
                                    mod_path_for_version.clone(),
                                    nexus_id_clone.clone(),
                                    version_clone.clone(),
                                    api_key,
                                    nexus_config.game_domain,
                                );
                            } else {
                                log::warn!("No Nexus API key configured");
                            }
                            popover_for_version.popdown();
                        });

                        action_group.add_action(&check_version_action);
                    }

                    popover.insert_action_group("mod", Some(&action_group));

                    popover.popup();
                });

                container.add_controller(gesture.clone());

                unsafe {
                    list_item.set_data("name-binding", binding);
                    list_item.set_data("is-section-name", false);
                    list_item.set_data("context-gesture", gesture);
                }
            }
        });

        factory.connect_unbind(move |_factory, item| {
            let list_item = item.downcast_ref::<gtk4::ListItem>()
                .expect("Item must be ListItem");

            let is_section: bool = unsafe {
                list_item.steal_data::<bool>("is-section-name").unwrap_or(false)
            };

            if is_section {
                // Clean up EditableLabel handler and delete button handler
                if let Some(container) = list_item.child().and_downcast::<Box>() {
                    if let Some(editable) = container.first_child().and_downcast::<EditableLabel>() {
                        editable.remove_css_class("heading");
                        unsafe {
                            if let Some(handler_id) = list_item.steal_data::<glib::SignalHandlerId>("editable-handler-id") {
                                editable.disconnect(handler_id);
                            }
                        }
                        // Clean up delete button handler (sibling of editable)
                        if let Some(delete_button) = editable.next_sibling().and_downcast::<Button>() {
                            unsafe {
                                if let Some(handler_id) = list_item.steal_data::<glib::SignalHandlerId>("delete-handler-id") {
                                    delete_button.disconnect(handler_id);
                                }
                            }
                        }
                    }
                    // Clean up motion controller
                    unsafe {
                        if let Some(controller) = list_item.steal_data::<EventControllerMotion>("motion-controller") {
                            container.remove_controller(&controller);
                        }
                    }
                }
            } else {
                // Clean up Label binding and context gesture
                if let Some(container) = list_item.child().and_downcast::<Box>() {
                    unsafe {
                        if let Some(gesture) = list_item.steal_data::<GestureClick>("context-gesture") {
                            container.remove_controller(&gesture);
                        }
                    }
                }
                unsafe {
                    if let Some(binding) = list_item.steal_data::<glib::Binding>("name-binding") {
                        binding.unbind();
                    }
                }
            }
        });

        let column = ColumnViewColumn::new(Some("Name"), Some(factory));
        column.set_resizable(false);
        column.set_expand(true);
        column_view.append_column(&column);
    }

    pub fn add_version_column(&self, column_view: &ColumnView, _settings: &gio::Settings) {
        let factory = SignalListItemFactory::new();

        factory.connect_setup(move |_factory, item| {
            let list_item = item.downcast_ref::<gtk4::ListItem>()
                .expect("Item must be ListItem");
            let label = Label::new(None);
            label.set_xalign(0.0);
            list_item.set_child(Some(&label));
        });

        factory.connect_bind(move |_factory, item| {
            let list_item = item.downcast_ref::<gtk4::ListItem>()
                .expect("Item must be ListItem");

            let label = list_item
                .child()
                .and_downcast::<Label>()
                .expect("Child must be Label");

            // Section headers show nothing in version column
            if list_item.item().and_downcast::<SectionHeader>().is_some() {
                label.set_text("");
                unsafe {
                    list_item.set_data("is-section-version", true);
                }
            } else if let Some(mod_entry) = list_item.item().and_downcast::<ModEntry>() {
                let binding = mod_entry
                    .bind_property("version", &label, "label")
                    .sync_create()
                    .build();

                // Apply initial CSS class based on version_status
                apply_version_status_css(&label, mod_entry.version_status());

                // Update CSS class when version_status changes
                let label_clone = label.clone();
                let handler_id = mod_entry.connect_notify_local(
                    Some("version-status"),
                    move |entry, _| {
                        apply_version_status_css(&label_clone, entry.version_status());
                    },
                );

                unsafe {
                    list_item.set_data("version-binding", binding);
                    list_item.set_data("version-status-handler-id", handler_id);
                    list_item.set_data("is-section-version", false);
                }
            }
        });

        factory.connect_unbind(move |_factory, item| {
            let list_item = item.downcast_ref::<gtk4::ListItem>()
                .expect("Item must be ListItem");

            unsafe {
                if let Some(binding) = list_item.steal_data::<glib::Binding>("version-binding") {
                    binding.unbind();
                }
                let is_section = list_item.steal_data::<bool>("is-section-version").unwrap_or(false);
                if !is_section {
                    if let Some(mod_entry) = list_item.item().and_downcast::<ModEntry>() {
                        if let Some(handler_id) = list_item.steal_data::<glib::SignalHandlerId>("version-status-handler-id") {
                            mod_entry.disconnect(handler_id);
                        }
                    }
                }
            }

            // Clear version status CSS classes
            if let Some(label) = list_item.child().and_downcast::<Label>() {
                label.remove_css_class("version-uptodate");
                label.remove_css_class("version-outdated");
            }
        });

        let column = ColumnViewColumn::new(Some("Ver"), Some(factory));
        column.set_resizable(false);
        column.set_fixed_width(60);
        column_view.append_column(&column);
    }

    pub fn add_order_column(&self, column_view: &ColumnView, _settings: &gio::Settings) {
        let factory = SignalListItemFactory::new();

        factory.connect_setup(move |_factory, item| {
            let list_item = item.downcast_ref::<gtk4::ListItem>()
                .expect("Item must be ListItem");
            let label = Label::new(None);
            label.set_xalign(0.5);
            list_item.set_child(Some(&label));
        });

        factory.connect_bind(move |_factory, item| {
            let list_item = item.downcast_ref::<gtk4::ListItem>()
                .expect("Item must be ListItem");

            let label = list_item
                .child()
                .and_downcast::<Label>()
                .expect("Child must be Label");

            // Section headers show nothing in order column
            if list_item.item().and_downcast::<SectionHeader>().is_some() {
                label.set_text("");
                unsafe {
                    list_item.set_data("is-section-order", true);
                }
            } else if let Some(mod_entry) = list_item.item().and_downcast::<ModEntry>() {
                // Set initial value
                label.set_text(&mod_entry.order().to_string());

                // Update when order property changes
                let label_clone = label.clone();
                let handler_id = mod_entry.connect_notify_local(
                    Some("order"),
                    move |entry, _| {
                        label_clone.set_text(&entry.order().to_string());
                    },
                );

                // Store handler for cleanup
                unsafe {
                    list_item.set_data("order-handler-id", handler_id);
                    list_item.set_data("is-section-order", false);
                }
            }
        });

        factory.connect_unbind(move |_factory, item| {
            let list_item = item.downcast_ref::<gtk4::ListItem>()
                .expect("Item must be ListItem");

            unsafe {
                let is_section = list_item.steal_data::<bool>("is-section-order").unwrap_or(false);
                if !is_section {
                    if let Some(mod_entry) = list_item.item().and_downcast::<ModEntry>() {
                        if let Some(handler_id) = list_item.steal_data::<glib::SignalHandlerId>("order-handler-id") {
                            mod_entry.disconnect(handler_id);
                        }
                    }
                }
            }
        });

        let column = ColumnViewColumn::new(Some("#"), Some(factory));
        column.set_resizable(false);
        column.set_fixed_width(35);
        column_view.append_column(&column);
    }

    pub fn add_conflicts_column(&self, column_view: &ColumnView, _settings: &gio::Settings) {
        let factory = SignalListItemFactory::new();

        factory.connect_setup(move |_factory, item| {
            let list_item = item.downcast_ref::<gtk4::ListItem>()
                .expect("Item must be ListItem");
            let label = Label::new(Some("-"));
            label.set_xalign(0.5);
            list_item.set_child(Some(&label));
        });

        factory.connect_bind(move |_factory, item| {
            let list_item = item.downcast_ref::<gtk4::ListItem>()
                .expect("Item must be ListItem");

            let label = list_item
                .child()
                .and_downcast::<Label>()
                .expect("Child must be Label");

            // Section headers show nothing in conflicts column
            if list_item.item().and_downcast::<SectionHeader>().is_some() {
                label.set_text("");
                label.remove_css_class("warning");
                unsafe {
                    list_item.set_data("is-section-conflict", true);
                }
            } else if let Some(mod_entry) = list_item.item().and_downcast::<ModEntry>() {
                // Initial value
                let count = mod_entry.conflict_count();
                if count > 0 {
                    label.set_text(&count.to_string());
                    label.add_css_class("warning");
                } else {
                    label.set_text("-");
                    label.remove_css_class("warning");
                }

                // Update when conflict_count property changes
                let label_clone = label.clone();
                let handler_id = mod_entry.connect_notify_local(
                    Some("conflict-count"),
                    move |entry, _| {
                        let count = entry.conflict_count();
                        if count > 0 {
                            label_clone.set_text(&count.to_string());
                            label_clone.add_css_class("warning");
                        } else {
                            label_clone.set_text("-");
                            label_clone.remove_css_class("warning");
                        }
                    },
                );

                // Store handler for cleanup
                unsafe {
                    list_item.set_data("conflict-handler-id", handler_id);
                    list_item.set_data("is-section-conflict", false);
                }
            }
        });

        factory.connect_unbind(move |_factory, item| {
            let list_item = item.downcast_ref::<gtk4::ListItem>()
                .expect("Item must be ListItem");

            unsafe {
                let is_section = list_item.steal_data::<bool>("is-section-conflict").unwrap_or(false);
                if !is_section {
                    if let Some(mod_entry) = list_item.item().and_downcast::<ModEntry>() {
                        if let Some(handler_id) = list_item.steal_data::<glib::SignalHandlerId>("conflict-handler-id") {
                            mod_entry.disconnect(handler_id);
                        }
                    }
                }
            }

            // Clear warning class
            if let Some(label) = list_item.child().and_downcast::<Label>() {
                label.remove_css_class("warning");
            }
        });

        let column = ColumnViewColumn::new(Some("⚠"), Some(factory));
        column.set_resizable(false);
        column.set_fixed_width(35);
        column_view.append_column(&column);
    }
}

/// Apply CSS class to label based on version status
/// 0 = unknown (no class), 1 = up-to-date (green), 2 = outdated (red)
fn apply_version_status_css(label: &Label, status: u8) {
    // Remove any existing version status classes
    label.remove_css_class("version-uptodate");
    label.remove_css_class("version-outdated");

    // Apply new class based on status
    match status {
        1 => label.add_css_class("version-uptodate"),
        2 => label.add_css_class("version-outdated"),
        _ => {} // 0 or other = no class
    }
}

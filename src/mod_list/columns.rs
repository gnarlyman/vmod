//! Column factory definitions for the ColumnView.

use gtk4::prelude::*;
use gtk4::{
    gio, glib, Box, Button, CheckButton, ColumnView, ColumnViewColumn, FilterChange,
    Label, Orientation, SignalListItemFactory,
};

use crate::mod_entry::{ModEntry, SectionHeader};
use super::imp::ModListView;
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

            // Check if this is a section header or mod entry
            if let Some(section) = list_item.item().and_downcast::<SectionHeader>() {
                // Section header: show name with bold styling
                label.add_css_class("heading");
                label.set_markup(&format!("<b>{}</b>", glib::markup_escape_text(&section.name())));

                let binding = section
                    .bind_property("name", &label, "label")
                    .transform_to(|_, name: String| {
                        Some(format!("<b>{}</b>", glib::markup_escape_text(&name)))
                    })
                    .sync_create()
                    .build();

                label.set_use_markup(true);

                unsafe {
                    list_item.set_data("name-binding", binding);
                    list_item.set_data("is-section-name", true);
                }
            } else if let Some(mod_entry) = list_item.item().and_downcast::<ModEntry>() {
                // Regular mod entry
                label.remove_css_class("heading");
                label.set_use_markup(false);

                let binding = mod_entry
                    .bind_property("name", &label, "label")
                    .sync_create()
                    .build();

                unsafe {
                    list_item.set_data("name-binding", binding);
                    list_item.set_data("is-section-name", false);
                }
            }
        });

        factory.connect_unbind(move |_factory, item| {
            let list_item = item.downcast_ref::<gtk4::ListItem>()
                .expect("Item must be ListItem");

            // Clean up CSS class if it was a section
            if let Some(label) = list_item.child().and_downcast::<Label>() {
                label.remove_css_class("heading");
                label.set_use_markup(false);
            }

            unsafe {
                if let Some(binding) = list_item.steal_data::<glib::Binding>("name-binding") {
                    binding.unbind();
                }
                let _ = list_item.steal_data::<bool>("is-section-name");
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

                unsafe {
                    list_item.set_data("version-binding", binding);
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
                let _ = list_item.steal_data::<bool>("is-section-version");
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

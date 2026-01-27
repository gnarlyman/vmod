//! UI widget for tree filtering with SearchEntry and options.

use gtk4::glib;
use gtk4::prelude::*;
use gtk4::subclass::prelude::*;
use gtk4::{Box, CheckButton, Label, Orientation, SearchEntry};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct TreeFilterWidget {
        pub search_entry: RefCell<Option<SearchEntry>>,
        pub show_subtrees_check: RefCell<Option<CheckButton>>,
        pub match_count_label: RefCell<Option<Label>>,
        pub debounce_source: RefCell<Option<glib::SourceId>>,
        /// Callback for filter changes
        pub filter_changed_callback: RefCell<Option<Rc<dyn Fn(&str, bool)>>>,
        /// Debounce delay in milliseconds
        pub debounce_ms: Cell<u32>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for TreeFilterWidget {
        const NAME: &'static str = "TreeFilterWidget";
        type Type = super::TreeFilterWidget;
        type ParentType = Box;
    }

    impl ObjectImpl for TreeFilterWidget {
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();
            obj.set_orientation(Orientation::Horizontal);
            obj.set_spacing(6);

            // Default debounce delay
            self.debounce_ms.set(150);

            // Create search entry
            let search_entry = SearchEntry::new();
            search_entry.set_placeholder_text(Some("Filter files..."));
            search_entry.set_hexpand(true);
            obj.append(&search_entry);

            // Create show subtrees checkbox
            let show_subtrees_check = CheckButton::with_label("Show subtrees");
            show_subtrees_check.set_tooltip_text(Some(
                "When enabled, show all children of matching folders",
            ));
            obj.append(&show_subtrees_check);

            // Create match count label (hidden when no search)
            let match_count_label = Label::new(None);
            match_count_label.add_css_class("dim-label");
            match_count_label.set_visible(false);
            obj.append(&match_count_label);

            // Connect search entry changes with debouncing
            let widget = obj.clone();
            search_entry.connect_search_changed(move |_| {
                widget.imp().schedule_filter_change();
            });

            // Connect checkbox changes (immediate, no debounce)
            let widget = obj.clone();
            show_subtrees_check.connect_toggled(move |_| {
                widget.imp().emit_filter_change();
            });

            // Store references
            self.search_entry.replace(Some(search_entry));
            self.show_subtrees_check.replace(Some(show_subtrees_check));
            self.match_count_label.replace(Some(match_count_label));
        }

        fn dispose(&self) {
            // Cancel any pending debounce timer
            if let Some(source_id) = self.debounce_source.borrow_mut().take() {
                source_id.remove();
            }
        }
    }

    impl WidgetImpl for TreeFilterWidget {}
    impl BoxImpl for TreeFilterWidget {}

    impl TreeFilterWidget {
        /// Schedule a debounced filter change.
        fn schedule_filter_change(&self) {
            // Cancel any pending timer
            if let Some(source_id) = self.debounce_source.borrow_mut().take() {
                source_id.remove();
            }

            let obj = self.obj().clone();
            let delay_ms = self.debounce_ms.get();

            let source_id = glib::timeout_add_local_once(
                Duration::from_millis(delay_ms as u64),
                move || {
                    obj.imp().emit_filter_change();
                },
            );

            self.debounce_source.replace(Some(source_id));
        }

        /// Emit the filter changed callback.
        fn emit_filter_change(&self) {
            // Clear debounce source since we're emitting now
            self.debounce_source.replace(None);

            let search_text = self
                .search_entry
                .borrow()
                .as_ref()
                .map(|e| e.text().to_string())
                .unwrap_or_default();

            let show_subtrees = self
                .show_subtrees_check
                .borrow()
                .as_ref()
                .map(|c| c.is_active())
                .unwrap_or(false);

            // Update match count label visibility
            if let Some(label) = self.match_count_label.borrow().as_ref() {
                label.set_visible(!search_text.is_empty());
            }

            // Call the callback if set
            if let Some(callback) = self.filter_changed_callback.borrow().as_ref() {
                callback(&search_text, show_subtrees);
            }
        }
    }
}

glib::wrapper! {
    /// A widget combining SearchEntry and CheckButton for tree filtering.
    ///
    /// Provides:
    /// - SearchEntry for filter text input
    /// - CheckButton to toggle "show subtrees" mode
    /// - Debounced input (default 150ms)
    /// - Match count display
    ///
    /// # Example
    ///
    /// ```ignore
    /// let filter_widget = TreeFilterWidget::new();
    ///
    /// filter_widget.connect_filter_changed(|text, show_subtrees| {
    ///     println!("Filter: '{}', show_subtrees: {}", text, show_subtrees);
    ///     // Update your TreeListModel here
    /// });
    /// ```
    pub struct TreeFilterWidget(ObjectSubclass<imp::TreeFilterWidget>)
        @extends Box, gtk4::Widget,
        @implements gtk4::Accessible, gtk4::Buildable, gtk4::ConstraintTarget, gtk4::Orientable;
}

impl Default for TreeFilterWidget {
    fn default() -> Self {
        Self::new()
    }
}

impl TreeFilterWidget {
    /// Create a new TreeFilterWidget.
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    /// Get the current search text.
    pub fn search_text(&self) -> String {
        self.imp()
            .search_entry
            .borrow()
            .as_ref()
            .map(|e| e.text().to_string())
            .unwrap_or_default()
    }

    /// Set the search text programmatically.
    pub fn set_search_text(&self, text: &str) {
        if let Some(entry) = self.imp().search_entry.borrow().as_ref() {
            entry.set_text(text);
        }
    }

    /// Get whether "show subtrees" is enabled.
    pub fn show_subtrees(&self) -> bool {
        self.imp()
            .show_subtrees_check
            .borrow()
            .as_ref()
            .map(|c| c.is_active())
            .unwrap_or(false)
    }

    /// Set whether "show subtrees" is enabled.
    pub fn set_show_subtrees(&self, show: bool) {
        if let Some(check) = self.imp().show_subtrees_check.borrow().as_ref() {
            check.set_active(show);
        }
    }

    /// Set the debounce delay in milliseconds.
    pub fn set_debounce_ms(&self, ms: u32) {
        self.imp().debounce_ms.set(ms);
    }

    /// Update the match count display.
    pub fn set_match_count(&self, count: usize) {
        if let Some(label) = self.imp().match_count_label.borrow().as_ref() {
            if count > 0 {
                label.set_text(&format!("{} matches", count));
            } else {
                label.set_text("No matches");
            }
        }
    }

    /// Set the placeholder text for the search entry.
    pub fn set_placeholder_text(&self, text: Option<&str>) {
        if let Some(entry) = self.imp().search_entry.borrow().as_ref() {
            entry.set_placeholder_text(text);
        }
    }

    /// Connect a callback for filter changes.
    ///
    /// The callback receives (search_text, show_subtrees) parameters.
    /// Input is debounced by default (150ms).
    pub fn connect_filter_changed<F: Fn(&str, bool) + 'static>(&self, f: F) {
        self.imp()
            .filter_changed_callback
            .replace(Some(Rc::new(f)));
    }

    /// Clear the filter (reset search text and checkbox).
    pub fn clear(&self) {
        if let Some(entry) = self.imp().search_entry.borrow().as_ref() {
            entry.set_text("");
        }
        if let Some(check) = self.imp().show_subtrees_check.borrow().as_ref() {
            check.set_active(false);
        }
    }

    /// Get the SearchEntry widget for additional customization.
    pub fn search_entry(&self) -> Option<SearchEntry> {
        self.imp().search_entry.borrow().clone()
    }

    /// Get the CheckButton widget for additional customization.
    pub fn show_subtrees_check(&self) -> Option<CheckButton> {
        self.imp().show_subtrees_check.borrow().clone()
    }
}

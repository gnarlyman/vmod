//! Trait definition for filterable tree items.

use gtk4::glib;
use gtk4::prelude::*;

/// Trait that tree items must implement to support hierarchical filtering.
///
/// Any GObject that wants tree filtering capabilities should implement this trait.
/// The trait provides the contract between the filter state and tree items.
///
/// # Example
///
/// ```ignore
/// impl FilterableTreeItem for MyTreeItem {
///     fn filter_text(&self) -> String {
///         self.display_name()
///     }
///
///     fn filter_path(&self) -> String {
///         self.full_path()
///     }
///
///     fn is_expandable(&self) -> bool {
///         self.is_expandable()
///     }
///
///     fn matches_filter(&self) -> bool {
///         self.property("matches-filter")
///     }
///
///     fn set_matches_filter(&self, matches: bool) {
///         self.set_property("matches-filter", matches);
///     }
///
///     fn visible_in_filter(&self) -> bool {
///         self.property("visible-in-filter")
///     }
///
///     fn set_visible_in_filter(&self, visible: bool) {
///         self.set_property("visible-in-filter", visible);
///     }
/// }
/// ```
pub trait FilterableTreeItem: IsA<glib::Object> {
    /// Display text used for matching against search query.
    /// Usually the item's name or label.
    fn filter_text(&self) -> String;

    /// Unique path identifier used for ancestor tracking.
    /// Should use "/" as separator for hierarchical paths.
    fn filter_path(&self) -> String;

    /// Whether this item can have children (folder/container).
    fn is_expandable(&self) -> bool;

    /// Get whether this item directly matches the current filter.
    fn matches_filter(&self) -> bool;

    /// Set whether this item directly matches the current filter.
    fn set_matches_filter(&self, matches: bool);

    /// Get whether this item is visible in filter results.
    /// True if this item or any descendant matches the filter.
    fn visible_in_filter(&self) -> bool;

    /// Set whether this item is visible in filter results.
    fn set_visible_in_filter(&self, visible: bool);
}

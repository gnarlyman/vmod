//! Reusable tree filtering module for GTK4 TreeListModel-based views.
//!
//! GTK4's `FilterListModel` only filters flat lists - it doesn't understand tree hierarchy.
//! If a parent is filtered out, children become orphaned. This module provides a reusable
//! abstraction for hierarchical filtering.
//!
//! # Usage
//!
//! 1. Implement `FilterableTreeItem` trait on your tree item GObject
//! 2. Create a `TreeFilterState` to manage filter logic
//! 3. Use `filter_children()` in your TreeListModel's child factory closure
//! 4. Optionally use `TreeFilterWidget` for a ready-made search UI

mod filterable_item;
mod filter_state;
mod filter_widget;

pub use filterable_item::FilterableTreeItem;
pub use filter_state::TreeFilterState;
pub use filter_widget::TreeFilterWidget;

use gtk4::gio;
use gtk4::glib;
use gtk4::prelude::*;

/// Filter children based on current filter state.
///
/// Use this in your TreeListModel's child factory closure to filter
/// children while preserving tree hierarchy.
pub fn filter_children<T, I>(
    children: I,
    filter_state: &TreeFilterState,
) -> Vec<T>
where
    T: FilterableTreeItem + Clone,
    I: Iterator<Item = T>,
{
    if !filter_state.is_active() {
        return children.collect();
    }

    children
        .filter(|item| filter_state.is_visible(&item.filter_path()))
        .map(|item| {
            item.set_matches_filter(filter_state.matches(&item.filter_text()));
            item.set_visible_in_filter(true);
            item
        })
        .collect()
}

/// Build a ListStore from filtered children.
///
/// Convenience function that filters children and builds a ListStore,
/// returning None if no children pass the filter.
pub fn build_filtered_store<T, I>(
    children: I,
    filter_state: &TreeFilterState,
) -> Option<gio::ListModel>
where
    T: FilterableTreeItem + Clone + glib::prelude::IsA<glib::Object>,
    I: Iterator<Item = T>,
{
    let filtered = filter_children(children, filter_state);

    if filtered.is_empty() {
        return None;
    }

    let store = gio::ListStore::new::<T>();
    for item in filtered {
        store.append(&item);
    }
    Some(store.upcast())
}

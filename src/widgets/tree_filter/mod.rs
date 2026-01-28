//! Reusable tree filtering module for GTK4 TreeListModel-based views.
//!
//! GTK4's `FilterListModel` only filters flat lists - it doesn't understand tree hierarchy.
//! If a parent is filtered out, children become orphaned. This module provides a reusable
//! abstraction for hierarchical filtering.
//!
//! # Usage
//!
//! 1. Create a `TreeFilterState` to manage filter logic
//! 2. Optionally use `TreeFilterWidget` for a ready-made search UI

mod filter_state;
mod filter_widget;

pub use filter_state::TreeFilterState;
pub use filter_widget::TreeFilterWidget;

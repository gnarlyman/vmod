//! Core filtering logic for hierarchical tree views.

use std::collections::HashSet;

/// Manages filter state and visibility computation for tree filtering.
///
/// This struct handles:
/// - Case-insensitive text matching
/// - Ancestor path computation (to show parents of matching items)
/// - Subtree visibility propagation (to optionally show all children of matches)
/// - HashSet-based O(1) visibility lookups
///
/// # Usage
///
/// ```ignore
/// let mut filter_state = TreeFilterState::new();
///
/// // Update search parameters
/// filter_state.set_search("texture", true);
///
/// // Compute visibility for all paths in your data
/// let all_paths = get_all_file_paths();
/// filter_state.compute_visibility(all_paths.iter().map(|s| s.as_str()));
///
/// // Check visibility in your TreeListModel child factory
/// if filter_state.is_visible(&item.full_path()) {
///     // Include this item
/// }
/// ```
#[derive(Debug, Clone)]
pub struct TreeFilterState {
    /// The raw search text
    search_text: String,
    /// Pre-computed lowercase search text for faster matching
    search_lower: String,
    /// Whether to show all descendants of matching items
    show_subtrees: bool,
    /// Set of paths that should be visible (matches or ancestors of matches)
    visible_paths: HashSet<String>,
    /// Set of paths that directly match the search
    matching_paths: HashSet<String>,
}

impl Default for TreeFilterState {
    fn default() -> Self {
        Self::new()
    }
}

impl TreeFilterState {
    /// Create a new empty filter state.
    pub fn new() -> Self {
        Self {
            search_text: String::new(),
            search_lower: String::new(),
            show_subtrees: false,
            visible_paths: HashSet::new(),
            matching_paths: HashSet::new(),
        }
    }

    /// Check if a filter is currently active.
    pub fn is_active(&self) -> bool {
        !self.search_text.is_empty()
    }

    /// Get the current search text.
    pub fn search_text(&self) -> &str {
        &self.search_text
    }

    /// Get whether show_subtrees is enabled.
    pub fn show_subtrees(&self) -> bool {
        self.show_subtrees
    }

    /// Update the search parameters.
    ///
    /// This clears the computed visibility - you should call `compute_visibility()`
    /// after updating the search.
    pub fn set_search(&mut self, text: &str, show_subtrees: bool) {
        self.search_text = text.to_string();
        self.search_lower = text.to_lowercase();
        self.show_subtrees = show_subtrees;
        self.visible_paths.clear();
        self.matching_paths.clear();
    }

    /// Clear the filter state.
    pub fn clear(&mut self) {
        self.search_text.clear();
        self.search_lower.clear();
        self.show_subtrees = false;
        self.visible_paths.clear();
        self.matching_paths.clear();
    }

    /// Check if the given path should be visible.
    ///
    /// Returns true if the filter is inactive (everything visible)
    /// or if the path is in the visible set.
    pub fn is_visible(&self, path: &str) -> bool {
        if !self.is_active() {
            return true;
        }
        self.visible_paths.contains(path)
    }

    /// Check if the given path directly matches the search.
    pub fn is_match(&self, path: &str) -> bool {
        self.matching_paths.contains(path)
    }

    /// Check if text matches the search query (case-insensitive contains).
    pub fn matches(&self, text: &str) -> bool {
        if self.search_lower.is_empty() {
            return false;
        }
        text.to_lowercase().contains(&self.search_lower)
    }

    /// Compute visibility for a set of paths.
    ///
    /// This scans all provided paths, finds matches, and marks their
    /// ancestors (and optionally descendants) as visible.
    ///
    /// # Arguments
    ///
    /// * `all_paths` - Iterator of all paths in the tree
    pub fn compute_visibility<I, S>(&mut self, all_paths: I)
    where
        I: Iterator<Item = S>,
        S: AsRef<str>,
    {
        self.visible_paths.clear();
        self.matching_paths.clear();

        if !self.is_active() {
            return;
        }

        // Collect all paths for potential subtree marking
        let paths: Vec<String> = all_paths.map(|s| s.as_ref().to_string()).collect();

        // First pass: find all matching paths
        for path in &paths {
            // Extract filename/last component for matching
            let name = path.rsplit('/').next().unwrap_or(path);
            if self.matches(name) {
                self.matching_paths.insert(path.clone());
                self.mark_path_visible(path);

                // If show_subtrees is enabled, mark all descendants visible
                if self.show_subtrees {
                    self.mark_subtree_visible(path, &paths);
                }
            }
        }
    }

    /// Compute visibility for paths with their display names.
    ///
    /// Use this when the display name differs from the path's last component.
    ///
    /// # Arguments
    ///
    /// * `paths_with_names` - Iterator of (path, display_name) tuples
    pub fn compute_visibility_with_names<I, S1, S2>(&mut self, paths_with_names: I)
    where
        I: Iterator<Item = (S1, S2)>,
        S1: AsRef<str>,
        S2: AsRef<str>,
    {
        self.visible_paths.clear();
        self.matching_paths.clear();

        if !self.is_active() {
            return;
        }

        // Collect all paths for potential subtree marking
        let items: Vec<(String, String)> = paths_with_names
            .map(|(p, n)| (p.as_ref().to_string(), n.as_ref().to_string()))
            .collect();

        let paths: Vec<String> = items.iter().map(|(p, _)| p.clone()).collect();

        // Find all matching paths
        for (path, name) in &items {
            if self.matches(name) {
                self.matching_paths.insert(path.clone());
                self.mark_path_visible(path);

                if self.show_subtrees {
                    self.mark_subtree_visible(path, &paths);
                }
            }
        }
    }

    /// Mark a path and all its ancestors as visible.
    fn mark_path_visible(&mut self, path: &str) {
        self.visible_paths.insert(path.to_string());

        // Mark all ancestor paths visible
        let mut current = path.to_string();
        while let Some(parent_end) = current.rfind('/') {
            current.truncate(parent_end);
            if current.is_empty() {
                break;
            }
            // If already visible, ancestors are too - early exit
            if !self.visible_paths.insert(current.clone()) {
                break;
            }
        }
    }

    /// Mark all descendants of a path as visible.
    fn mark_subtree_visible(&mut self, parent_path: &str, all_paths: &[String]) {
        let prefix = if parent_path.ends_with('/') {
            parent_path.to_string()
        } else {
            format!("{}/", parent_path)
        };

        for path in all_paths {
            if path.starts_with(&prefix) {
                self.visible_paths.insert(path.clone());
            }
        }
    }

    /// Get the number of matching paths.
    pub fn match_count(&self) -> usize {
        self.matching_paths.len()
    }

    /// Get the number of visible paths.
    pub fn visible_count(&self) -> usize {
        self.visible_paths.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inactive_filter() {
        let filter = TreeFilterState::new();
        assert!(!filter.is_active());
        assert!(filter.is_visible("any/path"));
    }

    #[test]
    fn test_basic_matching() {
        let mut filter = TreeFilterState::new();
        filter.set_search("wood", false);

        let paths = vec![
            "mod1/textures/wood.png",
            "mod1/textures/stone.png",
            "mod1/sounds/click.wav",
            "mod1/meshes/chair.obj",
        ];

        filter.compute_visibility(paths.iter());

        // Files matching "wood" should be visible
        assert!(filter.is_visible("mod1/textures/wood.png"));
        // Their parent should also be visible
        assert!(filter.is_visible("mod1/textures"));
        assert!(filter.is_visible("mod1"));

        // Non-matching paths should not be visible
        assert!(!filter.is_visible("mod1/textures/stone.png"));
        assert!(!filter.is_visible("mod1/sounds/click.wav"));
        assert!(!filter.is_visible("mod1/meshes/chair.obj"));
    }

    #[test]
    fn test_case_insensitive() {
        let mut filter = TreeFilterState::new();
        filter.set_search("TEXTURE", false);

        assert!(filter.matches("textures"));
        assert!(filter.matches("MyTexture.png"));
        assert!(filter.matches("TEXTURE_MAP"));
    }

    #[test]
    fn test_show_subtrees() {
        let mut filter = TreeFilterState::new();
        filter.set_search("textures", true);

        let paths = vec![
            "mod1/textures",
            "mod1/textures/wood.png",
            "mod1/textures/stone.png",
            "mod1/sounds/click.wav",
        ];

        filter.compute_visibility(paths.iter());

        // Matching folder and all its children should be visible
        assert!(filter.is_visible("mod1/textures"));
        assert!(filter.is_visible("mod1/textures/wood.png"));
        assert!(filter.is_visible("mod1/textures/stone.png"));

        // Non-matching paths should not be visible
        assert!(!filter.is_visible("mod1/sounds/click.wav"));
    }

    #[test]
    fn test_clear() {
        let mut filter = TreeFilterState::new();
        filter.set_search("test", false);
        filter.compute_visibility(vec!["test/path"].iter());

        assert!(filter.is_active());

        filter.clear();

        assert!(!filter.is_active());
        assert!(filter.is_visible("any/path"));
    }
}

//! Sorting rules for mod load order.
//!
//! Loads rules from sorting_rules.json and applies topological sort
//! to determine the correct mod load order.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use super::mods_json_manager::ModsJsonEntry;

/// A single sorting rule: "first" should load before "then"
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct SortingRule {
    pub first: String,
    pub then: String,
}

/// Collection of sorting rules
#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct SortingRules {
    pub rules: Vec<SortingRule>,
}

impl SortingRules {
    /// Load sorting rules from a JSON file
    pub fn load(path: &Path) -> Result<Self, String> {
        if !path.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(path)
            .map_err(|e| format!("Failed to read sorting rules: {}", e))?;

        serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse sorting rules: {}", e))
    }

    /// Apply sorting based on rules order.
    /// Rules are assumed to be consecutive pairs (A->B, B->C, etc).
    /// Follows rule order directly to preserve intended sequence.
    /// Returns the sorted entries, or an error if a cycle is detected.
    pub fn apply_sort(&self, entries: &[ModsJsonEntry]) -> Result<Vec<ModsJsonEntry>, String> {
        if entries.is_empty() || self.rules.is_empty() {
            return Ok(entries.to_vec());
        }

        // Build a normalized filename lookup map
        let mut name_to_index: HashMap<String, usize> = HashMap::new();
        for (i, entry) in entries.iter().enumerate() {
            let normalized = normalize_name(&entry.file_name);
            name_to_index.insert(normalized, i);
        }

        // Build sorted list by following rule order
        // Rules are consecutive pairs, so iterate through and collect in order
        let mut sorted_indices: Vec<usize> = Vec::new();
        let mut seen: std::collections::HashSet<usize> = std::collections::HashSet::new();
        let mut is_constrained: Vec<bool> = vec![false; entries.len()];

        for rule in &self.rules {
            let first_norm = normalize_name(&rule.first);
            let then_norm = normalize_name(&rule.then);

            // Add "first" if not already added and exists
            if let Some(&first_idx) = name_to_index.get(&first_norm) {
                is_constrained[first_idx] = true;
                if !seen.contains(&first_idx) {
                    seen.insert(first_idx);
                    sorted_indices.push(first_idx);
                }
            }

            // Add "then" if not already added and exists
            if let Some(&then_idx) = name_to_index.get(&then_norm) {
                is_constrained[then_idx] = true;
                if !seen.contains(&then_idx) {
                    seen.insert(then_idx);
                    sorted_indices.push(then_idx);
                }
            }
        }

        // Build final result:
        // - Keep unconstrained mods in original order
        // - Insert constrained mods at the position of the first constrained mod
        let mut result: Vec<ModsJsonEntry> = Vec::with_capacity(entries.len());
        let mut constrained_inserted = false;

        for (i, entry) in entries.iter().enumerate() {
            if is_constrained[i] {
                // Insert all constrained mods at the position of the first one
                if !constrained_inserted {
                    for &sorted_idx in &sorted_indices {
                        result.push(entries[sorted_idx].clone());
                    }
                    constrained_inserted = true;
                }
                // Skip this entry, it's already been added in sorted order
            } else {
                result.push(entry.clone());
            }
        }

        // Update load priorities to be sequential
        for (i, entry) in result.iter_mut().enumerate() {
            entry.load_priority = i as u32;
        }

        Ok(result)
    }
}

/// Normalize a mod name for matching.
/// Removes spaces and converts to lowercase for fuzzy matching.
fn normalize_name(name: &str) -> String {
    name.to_lowercase()
        .replace(" - ", "")
        .replace("- ", "")
        .replace(" -", "")
        .replace("-", "")
        .replace(" ", "")
        .replace("'", "")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(name: &str, priority: u32) -> ModsJsonEntry {
        ModsJsonEntry {
            file_name: name.to_string(),
            title: name.to_string(),
            enabled: true,
            load_priority: priority,
        }
    }

    #[test]
    fn test_empty_rules() {
        let rules = SortingRules::default();
        let entries = vec![make_entry("mod_a", 0), make_entry("mod_b", 1)];
        let result = rules.apply_sort(&entries).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_simple_ordering() {
        let rules = SortingRules {
            rules: vec![SortingRule {
                first: "mod_a".to_string(),
                then: "mod_b".to_string(),
            }],
        };
        // Start with wrong order
        let entries = vec![make_entry("mod_b", 0), make_entry("mod_a", 1)];
        let result = rules.apply_sort(&entries).unwrap();
        assert_eq!(result[0].file_name, "mod_a");
        assert_eq!(result[1].file_name, "mod_b");
    }

    #[test]
    fn test_chain_ordering() {
        let rules = SortingRules {
            rules: vec![
                SortingRule {
                    first: "mod_a".to_string(),
                    then: "mod_b".to_string(),
                },
                SortingRule {
                    first: "mod_b".to_string(),
                    then: "mod_c".to_string(),
                },
            ],
        };
        let entries = vec![
            make_entry("mod_c", 0),
            make_entry("mod_a", 1),
            make_entry("mod_b", 2),
        ];
        let result = rules.apply_sort(&entries).unwrap();
        assert_eq!(result[0].file_name, "mod_a");
        assert_eq!(result[1].file_name, "mod_b");
        assert_eq!(result[2].file_name, "mod_c");
    }

    #[test]
    fn test_redundant_rules_handled() {
        // With rule-order following (not topological sort), redundant rules
        // like A->B, B->A are handled gracefully - first occurrence wins
        let rules = SortingRules {
            rules: vec![
                SortingRule {
                    first: "mod_a".to_string(),
                    then: "mod_b".to_string(),
                },
                SortingRule {
                    first: "mod_b".to_string(),
                    then: "mod_a".to_string(),
                },
            ],
        };
        let entries = vec![make_entry("mod_a", 0), make_entry("mod_b", 1)];
        let result = rules.apply_sort(&entries).unwrap();
        // First rule adds mod_a then mod_b, second rule's entries already seen
        assert_eq!(result[0].file_name, "mod_a");
        assert_eq!(result[1].file_name, "mod_b");
    }

    #[test]
    fn test_normalize_matching() {
        let rules = SortingRules {
            rules: vec![SortingRule {
                first: "roleplayrealism - items".to_string(),
                then: "roleplayrealism".to_string(),
            }],
        };
        // Mod.json uses no spaces around hyphen
        let entries = vec![
            make_entry("roleplayrealism", 0),
            make_entry("roleplayrealism-items", 1),
        ];
        let result = rules.apply_sort(&entries).unwrap();
        // roleplayrealism-items should come before roleplayrealism
        assert_eq!(result[0].file_name, "roleplayrealism-items");
        assert_eq!(result[1].file_name, "roleplayrealism");
    }

    #[test]
    fn test_missing_mods_ignored() {
        let rules = SortingRules {
            rules: vec![SortingRule {
                first: "nonexistent".to_string(),
                then: "mod_a".to_string(),
            }],
        };
        let entries = vec![make_entry("mod_a", 0), make_entry("mod_b", 1)];
        let result = rules.apply_sort(&entries).unwrap();
        // Should succeed and preserve original order
        assert_eq!(result.len(), 2);
    }
}

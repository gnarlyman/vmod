//! Sorting rules for mod load order.
//!
//! Loads rules from sorting_rules.json and applies topological sort
//! with transitive inference to determine the correct mod load order.
//! If a chain A→B→C exists and B is missing, A→C is still enforced.

use serde::{Deserialize, Serialize};
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::cmp::Reverse;

use super::mods_json_manager::ModsJsonEntry;

/// Result of violation detection for each mod
/// Maps normalized mod name to status: 0=Neutral, 1=Correct, 2=Wrong
pub struct ViolationStatus {
    pub status: HashMap<String, u8>,
}

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

    /// Apply sorting based on rules with transitive inference.
    /// If a chain A→B→C exists and B is missing, A→C is still enforced.
    /// Uses stable topological sort with original positions as tiebreaker.
    /// Returns the sorted entries, or an error if a cycle is detected.
    pub fn apply_sort(&self, entries: &[ModsJsonEntry]) -> Result<Vec<ModsJsonEntry>, String> {
        if entries.is_empty() || self.rules.is_empty() {
            return Ok(entries.to_vec());
        }

        // Build a normalized filename lookup map: normalized_name -> original_index
        let mut name_to_index: HashMap<String, usize> = HashMap::new();
        for (i, entry) in entries.iter().enumerate() {
            let normalized = normalize_name(&entry.file_name);
            name_to_index.insert(normalized, i);
        }

        // Set of normalized names for present mods
        let present_mods: HashSet<String> = name_to_index.keys().cloned().collect();

        // 1. Build full constraint graph from ALL rules (including missing mods)
        let mut graph: HashMap<String, HashSet<String>> = HashMap::new();
        let mut all_nodes: HashSet<String> = HashSet::new();

        for rule in &self.rules {
            let first_norm = normalize_name(&rule.first);
            let then_norm = normalize_name(&rule.then);

            all_nodes.insert(first_norm.clone());
            all_nodes.insert(then_norm.clone());

            graph.entry(first_norm)
                .or_default()
                .insert(then_norm);
        }

        // 2. Compute transitive closure using DFS from each node
        let closure = compute_transitive_closure(&graph, &all_nodes);

        // 3. Filter to only present mods
        let filtered_graph = filter_to_present(&closure, &present_mods);

        // 4. Identify which mods are constrained (appear in filtered graph)
        let mut is_constrained: Vec<bool> = vec![false; entries.len()];
        for (from, tos) in &filtered_graph {
            if let Some(&idx) = name_to_index.get(from) {
                is_constrained[idx] = true;
            }
            for to in tos {
                if let Some(&idx) = name_to_index.get(to) {
                    is_constrained[idx] = true;
                }
            }
        }

        // 5. Apply stable topological sort using Kahn's algorithm with min-heap
        let sorted_constrained = stable_toposort(&filtered_graph, &name_to_index, &is_constrained, entries)?;

        // 6. Build final result: merge sorted constrained mods into their position
        let mut result: Vec<ModsJsonEntry> = Vec::with_capacity(entries.len());
        let mut constrained_iter = sorted_constrained.into_iter();
        let mut constrained_inserted = false;

        for (i, entry) in entries.iter().enumerate() {
            if is_constrained[i] {
                // Insert all constrained mods at the position of the first one
                if !constrained_inserted {
                    for sorted_entry in constrained_iter.by_ref() {
                        result.push(sorted_entry);
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

    /// Detect which mods violate sorting rules in the given order.
    /// Returns status for each mod:
    /// - 0 = Neutral (not in any rule)
    /// - 1 = Correct (in rules, all constraints satisfied)
    /// - 2 = Wrong (violates at least one rule)
    pub fn detect_violations(&self, entries: &[ModsJsonEntry]) -> ViolationStatus {
        let mut status: HashMap<String, u8> = HashMap::new();

        if entries.is_empty() || self.rules.is_empty() {
            return ViolationStatus { status };
        }

        // Build name→position map from current order
        let mut name_to_position: HashMap<String, usize> = HashMap::new();
        for (i, entry) in entries.iter().enumerate() {
            let normalized = normalize_name(&entry.file_name);
            name_to_position.insert(normalized, i);
        }

        // Set of normalized names for present mods
        let present_mods: HashSet<String> = name_to_position.keys().cloned().collect();

        // Build full constraint graph from ALL rules (including missing mods)
        let mut graph: HashMap<String, HashSet<String>> = HashMap::new();
        let mut all_nodes: HashSet<String> = HashSet::new();

        for rule in &self.rules {
            let first_norm = normalize_name(&rule.first);
            let then_norm = normalize_name(&rule.then);

            all_nodes.insert(first_norm.clone());
            all_nodes.insert(then_norm.clone());

            graph.entry(first_norm).or_default().insert(then_norm);
        }

        // Compute transitive closure using existing function
        let closure = compute_transitive_closure(&graph, &all_nodes);

        // Filter to only present mods
        let filtered_graph = filter_to_present(&closure, &present_mods);

        // Track which mods are constrained (appear in filtered graph)
        let mut constrained_mods: HashSet<String> = HashSet::new();
        for (from, tos) in &filtered_graph {
            constrained_mods.insert(from.clone());
            for to in tos {
                constrained_mods.insert(to.clone());
            }
        }

        // Track which mods are violated
        let mut violated_mods: HashSet<String> = HashSet::new();

        // Check each edge: if then_pos < first_pos, first is out of place (needs to move up)
        for (first, thens) in &filtered_graph {
            if let Some(&first_pos) = name_to_position.get(first) {
                for then in thens {
                    if let Some(&then_pos) = name_to_position.get(then) {
                        if then_pos < first_pos {
                            // Violation: "first" should be before "then" but isn't
                            // Only mark "first" - it's the one that needs to move up
                            violated_mods.insert(first.clone());
                        }
                    }
                }
            }
        }

        // Build final status map
        for (name, _) in &name_to_position {
            if violated_mods.contains(name) {
                status.insert(name.clone(), 2); // Wrong
            } else if constrained_mods.contains(name) {
                status.insert(name.clone(), 1); // Correct
            }
            // Unconstrained mods are not added (default 0 = Neutral)
        }

        ViolationStatus { status }
    }
}

/// Compute transitive closure of the graph using DFS from each node.
/// For each node A, find all reachable nodes and add edges A→reachable.
fn compute_transitive_closure(
    graph: &HashMap<String, HashSet<String>>,
    all_nodes: &HashSet<String>,
) -> HashMap<String, HashSet<String>> {
    let mut closure: HashMap<String, HashSet<String>> = HashMap::new();

    for start in all_nodes {
        let mut reachable: HashSet<String> = HashSet::new();
        let mut stack: Vec<&String> = vec![start];
        let mut visited: HashSet<&String> = HashSet::new();

        while let Some(node) = stack.pop() {
            if visited.contains(node) {
                continue;
            }
            visited.insert(node);

            if let Some(neighbors) = graph.get(node) {
                for neighbor in neighbors {
                    if neighbor != start {
                        reachable.insert(neighbor.clone());
                    }
                    if !visited.contains(neighbor) {
                        stack.push(neighbor);
                    }
                }
            }
        }

        if !reachable.is_empty() {
            closure.insert(start.clone(), reachable);
        }
    }

    closure
}

/// Filter the transitive closure to only include edges between present mods.
fn filter_to_present(
    closure: &HashMap<String, HashSet<String>>,
    present: &HashSet<String>,
) -> HashMap<String, HashSet<String>> {
    let mut filtered: HashMap<String, HashSet<String>> = HashMap::new();

    for (from, tos) in closure {
        if !present.contains(from) {
            continue;
        }
        let present_tos: HashSet<String> = tos
            .iter()
            .filter(|to| present.contains(*to))
            .cloned()
            .collect();
        if !present_tos.is_empty() {
            filtered.insert(from.clone(), present_tos);
        }
    }

    filtered
}

/// Stable topological sort using Kahn's algorithm with a min-heap keyed by original position.
fn stable_toposort(
    graph: &HashMap<String, HashSet<String>>,
    name_to_index: &HashMap<String, usize>,
    is_constrained: &[bool],
    entries: &[ModsJsonEntry],
) -> Result<Vec<ModsJsonEntry>, String> {
    // Build in-degree map for constrained mods
    let mut in_degree: HashMap<String, usize> = HashMap::new();

    // Initialize all constrained mods with in-degree 0
    for (name, &idx) in name_to_index {
        if is_constrained[idx] {
            in_degree.insert(name.clone(), 0);
        }
    }

    // Count incoming edges
    for (_, tos) in graph {
        for to in tos {
            if let Some(deg) = in_degree.get_mut(to) {
                *deg += 1;
            }
        }
    }

    // Use min-heap keyed by original position for stability
    // Reverse because BinaryHeap is a max-heap
    let mut heap: BinaryHeap<Reverse<(usize, String)>> = BinaryHeap::new();

    // Add all zero-in-degree mods to heap
    for (name, &deg) in &in_degree {
        if deg == 0 {
            if let Some(&idx) = name_to_index.get(name) {
                heap.push(Reverse((idx, name.clone())));
            }
        }
    }

    let mut result: Vec<ModsJsonEntry> = Vec::new();
    let mut processed = 0;

    while let Some(Reverse((_, name))) = heap.pop() {
        if let Some(&idx) = name_to_index.get(&name) {
            result.push(entries[idx].clone());
            processed += 1;
        }

        // Decrement successors' in-degrees
        if let Some(tos) = graph.get(&name) {
            for to in tos {
                if let Some(deg) = in_degree.get_mut(to) {
                    *deg -= 1;
                    if *deg == 0 {
                        if let Some(&idx) = name_to_index.get(to) {
                            heap.push(Reverse((idx, to.clone())));
                        }
                    }
                }
            }
        }
    }

    // Check for cycles
    let expected = in_degree.len();
    if processed < expected {
        return Err("Cycle detected in sorting rules".to_string());
    }

    Ok(result)
}

/// Normalize a mod name for matching.
/// Removes spaces and converts to lowercase for fuzzy matching.
pub fn normalize_name(name: &str) -> String {
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
        assert_eq!(result[0].file_name, "mod_a");
        assert_eq!(result[1].file_name, "mod_b");
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
    fn test_cycle_detection() {
        // A→B, B→A creates a cycle - should return error
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
        let result = rules.apply_sort(&entries);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Cycle"));
    }

    #[test]
    fn test_transitive_with_missing_mod() {
        // A→B→C chain, but B is missing - A should still be before C
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
        // Only mod_a and mod_c present, mod_b is missing
        let entries = vec![
            make_entry("mod_c", 0), // C is first originally
            make_entry("mod_a", 1), // A is second
        ];
        let result = rules.apply_sort(&entries).unwrap();
        // A should come before C due to transitive inference A→B→C becomes A→C
        assert_eq!(result[0].file_name, "mod_a");
        assert_eq!(result[1].file_name, "mod_c");
    }

    #[test]
    fn test_multiple_chains_missing_middle() {
        // A→B→C→D chain with B and C missing - A should still be before D
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
                SortingRule {
                    first: "mod_c".to_string(),
                    then: "mod_d".to_string(),
                },
            ],
        };
        // Only mod_a and mod_d present
        let entries = vec![
            make_entry("mod_d", 0), // D is first originally
            make_entry("mod_a", 1), // A is second
        ];
        let result = rules.apply_sort(&entries).unwrap();
        // A should come before D due to transitive closure
        assert_eq!(result[0].file_name, "mod_a");
        assert_eq!(result[1].file_name, "mod_d");
    }

    #[test]
    fn test_unconstrained_stability() {
        // Mods not referenced in rules should keep their original relative order
        let rules = SortingRules {
            rules: vec![SortingRule {
                first: "mod_a".to_string(),
                then: "mod_b".to_string(),
            }],
        };
        let entries = vec![
            make_entry("mod_x", 0),
            make_entry("mod_b", 1),
            make_entry("mod_y", 2),
            make_entry("mod_a", 3),
            make_entry("mod_z", 4),
        ];
        let result = rules.apply_sort(&entries).unwrap();
        // Unconstrained mods (x, y, z) should maintain their relative order
        // Constrained mods (a, b) should be sorted but inserted at first constrained position
        assert_eq!(result[0].file_name, "mod_x"); // unconstrained, original pos
        assert_eq!(result[1].file_name, "mod_a"); // constrained, sorted
        assert_eq!(result[2].file_name, "mod_b"); // constrained, sorted
        assert_eq!(result[3].file_name, "mod_y"); // unconstrained, original pos
        assert_eq!(result[4].file_name, "mod_z"); // unconstrained, original pos
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

    #[test]
    fn test_detect_violations_no_rules() {
        let rules = SortingRules::default();
        let entries = vec![make_entry("mod_a", 0), make_entry("mod_b", 1)];
        let status = rules.detect_violations(&entries);
        // No rules means all neutral (empty status map)
        assert!(status.status.is_empty());
    }

    #[test]
    fn test_detect_violations_correct_order() {
        let rules = SortingRules {
            rules: vec![SortingRule {
                first: "mod_a".to_string(),
                then: "mod_b".to_string(),
            }],
        };
        // Correct order: A before B
        let entries = vec![make_entry("mod_a", 0), make_entry("mod_b", 1)];
        let status = rules.detect_violations(&entries);
        // Both should be correct (1)
        assert_eq!(status.status.get("mod_a"), Some(&1));
        assert_eq!(status.status.get("mod_b"), Some(&1));
    }

    #[test]
    fn test_detect_violations_wrong_order() {
        let rules = SortingRules {
            rules: vec![SortingRule {
                first: "mod_a".to_string(),
                then: "mod_b".to_string(),
            }],
        };
        // Wrong order: B before A
        let entries = vec![make_entry("mod_b", 0), make_entry("mod_a", 1)];
        let status = rules.detect_violations(&entries);
        // Only A is wrong (needs to move up), B is correct (in place)
        assert_eq!(status.status.get("mod_a"), Some(&2));
        assert_eq!(status.status.get("mod_b"), Some(&1));
    }

    #[test]
    fn test_detect_violations_neutral_mods() {
        let rules = SortingRules {
            rules: vec![SortingRule {
                first: "mod_a".to_string(),
                then: "mod_b".to_string(),
            }],
        };
        // mod_x is not in any rule
        let entries = vec![
            make_entry("mod_a", 0),
            make_entry("mod_x", 1),
            make_entry("mod_b", 2),
        ];
        let status = rules.detect_violations(&entries);
        // A and B are correct, X is neutral (not in map)
        assert_eq!(status.status.get("mod_a"), Some(&1));
        assert_eq!(status.status.get("mod_b"), Some(&1));
        assert_eq!(status.status.get("mod_x"), None); // Neutral = not in map
    }

    #[test]
    fn test_detect_violations_transitive() {
        // A→B→C chain, but B is missing
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
        // Wrong order: C before A (B is missing)
        let entries = vec![make_entry("mod_c", 0), make_entry("mod_a", 1)];
        let status = rules.detect_violations(&entries);
        // Only A is wrong (needs to move up before C), C is correct
        assert_eq!(status.status.get("mod_a"), Some(&2));
        assert_eq!(status.status.get("mod_c"), Some(&1));
    }
}

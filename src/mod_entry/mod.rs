pub mod mod_data;
pub mod mod_state;
pub mod vfs;
pub mod dfmod_entry;
pub mod dfmod_parser;
pub mod mods_json_manager;
pub mod tree_item;
pub mod conflict_detector;
pub mod section;
pub mod list_row;
pub mod sorting_rules;
pub mod backup;
pub mod mod_metadata;

pub use mod_data::{ModEntry, ModList};
pub use mod_state::ModState;
pub use vfs::VirtualFileSystem;
pub use dfmod_entry::DfmodEntry;
// extract_dfmod_assets is used by examples/parse_dfmod.rs
#[allow(unused_imports)]
pub use dfmod_parser::{parse_dfmod_basic, extract_dfmod_assets, extract_dfmod_assets_cached, save_persistent_cache, DfmodCacheKey};
pub use mods_json_manager::{ModsJsonEntry, load_mods_json, save_mods_json};
pub use tree_item::TreeItem;
pub use conflict_detector::{ModConflictSummary, detect_all_conflicts, get_children_at_path};
pub use section::{SectionHeader, SectionsConfig};
pub use sorting_rules::{normalize_name, SortingRules};
pub use backup::BackupManager;
pub use mod_metadata::{ModMetadata, load_metadata, save_metadata};

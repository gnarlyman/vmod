pub mod mod_data;
pub mod mod_state;
pub mod vfs;
pub mod dfmod_entry;
pub mod dfmod_parser;
pub mod mods_json_manager;
pub mod tree_item;
pub mod conflict_detector;

pub use mod_data::{ModEntry, ModList};
pub use mod_state::ModState;
pub use vfs::VirtualFileSystem;
pub use dfmod_entry::DfmodEntry;
pub use dfmod_parser::{parse_dfmod, parse_dfmod_basic, extract_dfmod_assets, extract_dfmod_assets_cached, DfmodInfo, DfmodCacheKey};
pub use mods_json_manager::{ModsJsonEntry, load_mods_json, save_mods_json, generate_mods_json};
pub use tree_item::TreeItem;
pub use conflict_detector::{ConflictInfo, ModConflictSummary, ConflictScanProgress, detect_conflicts, detect_all_conflicts, enumerate_mod_files, enumerate_mod_files_with_dfmod, get_children_at_path};

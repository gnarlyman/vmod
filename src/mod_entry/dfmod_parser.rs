use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

/// Cache key for dfmod parsing results - tracks file identity by path, size, and mtime
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct DfmodCacheKey {
    pub path: PathBuf,
    pub size: u64,
    pub mtime: SystemTime,
}

impl DfmodCacheKey {
    /// Create a cache key from a file path by reading its metadata
    pub fn from_path(path: &Path) -> Option<Self> {
        let metadata = std::fs::metadata(path).ok()?;
        Some(Self {
            path: path.to_path_buf(),
            size: metadata.len(),
            mtime: metadata.modified().ok()?,
        })
    }
}

// ============================================================================
// Persistent Disk Cache
// ============================================================================

/// Entry in the persistent disk cache - stores file identity and extracted asset paths
#[derive(Debug, Clone, Serialize, Deserialize)]
struct DiskCacheEntry {
    size: u64,
    mtime_secs: i64,
    mtime_nanos: u32,
    asset_paths: Vec<String>,
}

/// Persistent cache stored on disk at ~/.cache/vmod/dfmod_cache.json
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct PersistentCache {
    version: u32,
    entries: HashMap<String, DiskCacheEntry>,
}

const CACHE_VERSION: u32 = 1;

/// Global persistent cache - loaded once on first use, saved periodically
static PERSISTENT_CACHE: std::sync::OnceLock<Mutex<PersistentCache>> = std::sync::OnceLock::new();

/// Get the path to the cache file
fn get_cache_path() -> Option<PathBuf> {
    dirs::cache_dir().map(|d| d.join("vmod").join("dfmod_cache.json"))
}

/// Load persistent cache from disk
fn load_persistent_cache() -> PersistentCache {
    let Some(cache_path) = get_cache_path() else {
        return PersistentCache::default();
    };

    if !cache_path.exists() {
        return PersistentCache {
            version: CACHE_VERSION,
            entries: HashMap::new(),
        };
    }

    match std::fs::read_to_string(&cache_path) {
        Ok(data) => match serde_json::from_str::<PersistentCache>(&data) {
            Ok(cache) if cache.version == CACHE_VERSION => cache,
            _ => PersistentCache {
                version: CACHE_VERSION,
                entries: HashMap::new(),
            },
        },
        Err(_) => PersistentCache {
            version: CACHE_VERSION,
            entries: HashMap::new(),
        },
    }
}

/// Save persistent cache to disk
pub fn save_persistent_cache() {
    let Some(cache_path) = get_cache_path() else {
        return;
    };

    let cache = PERSISTENT_CACHE.get_or_init(|| Mutex::new(load_persistent_cache()));
    let Ok(cache_guard) = cache.lock() else {
        return;
    };

    // Create cache directory if needed
    if let Some(parent) = cache_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    if let Ok(data) = serde_json::to_string(&*cache_guard) {
        let _ = std::fs::write(cache_path, data);
    }
}

/// Convert SystemTime to (secs, nanos) for serialization
fn systemtime_to_parts(time: SystemTime) -> (i64, u32) {
    match time.duration_since(std::time::UNIX_EPOCH) {
        Ok(duration) => (duration.as_secs() as i64, duration.subsec_nanos()),
        Err(e) => {
            let duration = e.duration();
            (-(duration.as_secs() as i64), duration.subsec_nanos())
        }
    }
}

/// Convert (secs, nanos) back to SystemTime
fn parts_to_systemtime(secs: i64, nanos: u32) -> SystemTime {
    if secs >= 0 {
        std::time::UNIX_EPOCH + std::time::Duration::new(secs as u64, nanos)
    } else {
        std::time::UNIX_EPOCH - std::time::Duration::new((-secs) as u64, nanos)
    }
}

/// Extract dfmod assets with persistent disk cache support
/// First checks the disk cache, then in-memory cache, then extracts fresh
pub fn extract_dfmod_assets_cached(
    dfmod_path: &Path,
    memory_cache: &mut HashMap<DfmodCacheKey, Vec<String>>,
) -> Vec<String> {
    let Some(key) = DfmodCacheKey::from_path(dfmod_path) else {
        return extract_dfmod_assets(dfmod_path);
    };

    // Check in-memory cache first (fastest)
    if let Some(cached) = memory_cache.get(&key) {
        return cached.clone();
    }

    // Check persistent disk cache
    let cache = PERSISTENT_CACHE.get_or_init(|| Mutex::new(load_persistent_cache()));
    if let Ok(cache_guard) = cache.lock() {
        let path_key = dfmod_path.to_string_lossy().to_string();
        if let Some(entry) = cache_guard.entries.get(&path_key) {
            let cached_mtime = parts_to_systemtime(entry.mtime_secs, entry.mtime_nanos);
            if entry.size == key.size && cached_mtime == key.mtime {
                // Cache hit - store in memory cache and return
                let assets = entry.asset_paths.clone();
                memory_cache.insert(key, assets.clone());
                return assets;
            }
        }
    }

    // Cache miss - extract fresh
    let assets = extract_dfmod_assets(dfmod_path);

    // Store in memory cache
    memory_cache.insert(key.clone(), assets.clone());

    // Store in persistent disk cache
    if let Ok(mut cache_guard) = cache.lock() {
        let (mtime_secs, mtime_nanos) = systemtime_to_parts(key.mtime);
        let path_key = dfmod_path.to_string_lossy().to_string();
        cache_guard.entries.insert(
            path_key,
            DiskCacheEntry {
                size: key.size,
                mtime_secs,
                mtime_nanos,
                asset_paths: assets.clone(),
            },
        );
    }

    assets
}

#[derive(Debug, Clone)]
pub struct DfmodInfo {
    pub title: String,
    pub file_name: String,
}

/// Capitalize first letter of each word in a string
/// E.g., "aquatic sprites" → "Aquatic Sprites"
/// E.g., "archaeologists" → "Archaeologists"
fn capitalize_words(s: &str) -> String {
    s.split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Extract asset paths from a dfmod file (Unity asset bundle)
/// Returns a list of asset paths contained in the bundle
pub fn extract_dfmod_assets(dfmod_path: &Path) -> Vec<String> {
    // Use catch_unwind to handle panics from unsupported Unity formats
    let path = dfmod_path.to_path_buf();
    let result = std::panic::catch_unwind(move || {
        extract_dfmod_assets_inner(&path)
    });

    match result {
        Ok(paths) => paths,
        Err(_) => Vec::new(),
    }
}

/// Inner function for asset extraction (may panic on unsupported formats)
fn extract_dfmod_assets_inner(dfmod_path: &Path) -> Vec<String> {
    use io_unity::unity_asset_view::UnityAssetViewer;
    use io_unity::unityfs::UnityFS;
    use std::collections::HashSet;

    let file = match File::open(dfmod_path) {
        Ok(f) => f,
        Err(_) => return Vec::new(),
    };

    let reader = BufReader::new(file);

    // Use UnityAssetViewer to extract container paths directly
    // This is faster than scanning binary data because it reads the container metadata
    let mut viewer = UnityAssetViewer::default();

    // add_bundle_file handles loading the UnityFS and all its CAB files
    if viewer
        .add_bundle_file(Box::new(reader), dfmod_path.parent().map(|p| p.to_string_lossy().to_string()))
        .is_err()
    {
        return Vec::new();
    }

    // Extract container paths - these are the actual asset paths
    let mut all_paths: Vec<String> = viewer
        .container_maps
        .keys()
        .filter(|p| is_asset_path(p))
        .cloned()
        .collect();

    // If no container paths found, fall back to binary scanning
    if all_paths.is_empty() {
        // Reload the file for the fallback method
        let file = match File::open(dfmod_path) {
            Ok(f) => f,
            Err(_) => return Vec::new(),
        };
        let reader = BufReader::new(file);

        let fs = match UnityFS::read(Box::new(reader), None) {
            Ok(f) => f,
            Err(_) => return Vec::new(),
        };

        for cab_path in fs.get_cab_path() {
            if let Ok(data) = fs.get_file_data_by_path(&cab_path) {
                let paths = extract_asset_paths_from_data(&data);
                all_paths.extend(paths);
            }
        }
    }

    // Deduplicate while preserving order
    let mut seen = HashSet::new();
    all_paths.retain(|p| seen.insert(p.clone()));

    all_paths
}

/// Extract asset paths from serialized file data
/// Asset paths are stored with a length prefix followed by the path string
fn extract_asset_paths_from_data(data: &[u8]) -> Vec<String> {
    let mut paths = Vec::new();
    let mut i = 0;
    while i < data.len().saturating_sub(4) {
        let len = u32::from_le_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]]) as usize;
        if len >= 4 && len < 512 && i + 4 + len <= data.len() {
            let potential_str = &data[i + 4..i + 4 + len];
            if let Ok(s) = std::str::from_utf8(potential_str) {
                if is_asset_path(s) {
                    paths.push(s.to_string());
                    i += 4 + len;
                    continue;
                }
            }
        }
        i += 1;
    }
    paths
}

/// Check if a string looks like an asset path
fn is_asset_path(s: &str) -> bool {
    // Must be printable ASCII
    if !s.chars().all(|c| c.is_ascii() && !c.is_ascii_control()) {
        return false;
    }

    let lower = s.to_lowercase();

    // Common asset path patterns
    if s.contains("assets/") || s.contains("Assets/") {
        return true;
    }

    // File extensions commonly in Unity assets
    let extensions = [
        ".cs", ".dll", ".txt", ".json", ".xml", ".png", ".jpg", ".wav", ".mp3", ".ogg", ".prefab",
        ".asset", ".mat", ".shader",
    ];
    for ext in &extensions {
        if lower.ends_with(ext) {
            return true;
        }
    }

    // Daggerfall-specific patterns
    if lower.contains("daggerfall")
        || lower.contains("dfunity")
        || lower.contains("questpack")
        || lower.contains("dfmod")
    {
        return true;
    }

    false
}

/// Scan mod folder for .dfmod files
/// Returns DfmodInfo for each .dfmod file found
///
/// FileName: The .dfmod filename without extension (e.g., "archaeologists")
/// Title: Capitalized version of FileName (e.g., "Archaeologists")
pub fn parse_dfmod_basic(mod_folder: &Path) -> Result<Vec<DfmodInfo>, String> {
    use std::fs;

    let mods_subfolder = mod_folder.join("Mods");

    if !mods_subfolder.exists() {
        return Ok(Vec::new());
    }

    let mut results = Vec::new();

    // Enumerate all .dfmod files and create an entry for each
    for entry in
        fs::read_dir(&mods_subfolder).map_err(|e| format!("Failed to read Mods folder: {}", e))?
    {
        let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
        let path = entry.path();

        if path.is_file() && path.extension().map_or(false, |ext| ext == "dfmod") {
            // Get the filename without extension
            let file_name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string();

            // Capitalize for title
            let title = capitalize_words(&file_name);

            results.push(DfmodInfo {
                file_name,
                title,
            });
        }
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_capitalize_words() {
        assert_eq!(capitalize_words("archaeologists"), "Archaeologists");
        assert_eq!(capitalize_words("aquatic sprites"), "Aquatic Sprites");
        assert_eq!(capitalize_words("ambienttext"), "Ambienttext");
        assert_eq!(capitalize_words("my awesome mod"), "My Awesome Mod");
    }

    #[test]
    fn test_parse_dfmod_no_mods_folder() {
        let temp_dir = TempDir::new().unwrap();
        let mod_path = temp_dir.path().join("test_mod");
        fs::create_dir(&mod_path).unwrap();

        let result = parse_dfmod_basic(&mod_path).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_dfmod_with_dfmod_file() {
        let temp_dir = TempDir::new().unwrap();
        let mod_path = temp_dir.path().join("TestMod-1.0");
        fs::create_dir(&mod_path).unwrap();

        let mods_subfolder = mod_path.join("Mods");
        fs::create_dir(&mods_subfolder).unwrap();

        // Create a .dfmod file (filename is what matters, not folder name)
        fs::write(mods_subfolder.join("my awesome mod.dfmod"), "binary data").unwrap();

        let result = parse_dfmod_basic(&mod_path).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].file_name, "my awesome mod");
        assert_eq!(result[0].title, "My Awesome Mod");
    }

    #[test]
    fn test_parse_dfmod_multiple_files() {
        let temp_dir = TempDir::new().unwrap();
        let mod_path = temp_dir.path().join("test_mod");
        fs::create_dir(&mod_path).unwrap();

        let mods_subfolder = mod_path.join("Mods");
        fs::create_dir(&mods_subfolder).unwrap();

        // Create multiple .dfmod files
        fs::write(mods_subfolder.join("archaeologists.dfmod"), "binary").unwrap();
        fs::write(mods_subfolder.join("aquatic sprites.dfmod"), "binary").unwrap();

        let result = parse_dfmod_basic(&mod_path).unwrap();
        assert_eq!(result.len(), 2);

        // Results should contain both files
        let file_names: Vec<_> = result.iter().map(|r| r.file_name.as_str()).collect();
        assert!(file_names.contains(&"archaeologists"));
        assert!(file_names.contains(&"aquatic sprites"));
    }

    #[test]
    fn test_parse_dfmod_no_dfmod_files() {
        let temp_dir = TempDir::new().unwrap();
        let mod_path = temp_dir.path().join("test_mod");
        fs::create_dir(&mod_path).unwrap();

        let mods_subfolder = mod_path.join("Mods");
        fs::create_dir(&mods_subfolder).unwrap();

        // Create non-.dfmod file
        fs::write(mods_subfolder.join("readme.txt"), "text").unwrap();

        let result = parse_dfmod_basic(&mod_path).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_is_asset_path() {
        // Should match asset paths
        assert!(is_asset_path("assets/game/mods/test.png"));
        assert!(is_asset_path("Assets/Scripts/MyMod.cs"));
        assert!(is_asset_path("test.dfmod.json"));
        assert!(is_asset_path("mymod.dll"));

        // Should not match
        assert!(!is_asset_path("just a random string"));
        assert!(!is_asset_path("12345"));
        assert!(!is_asset_path("")); // Empty string
    }

    /// Test with a real dfmod file if available
    /// This is marked as ignored to avoid failing in CI where dfmod files aren't available
    #[test]
    #[ignore]
    fn test_real_dfmod_extraction() {
        let test_paths = [
            "/var/home/bazzite/.config/vmod/profiles/test/mods/ArchaeologistsGuild-2.6.1-14-2-6-1-1712582888/Mods/archaeologists.dfmod",
            "/var/home/bazzite/.config/vmod/profiles/test/mods/Ambient Text 1.7-303-1-7-1743021606/Mods/ambienttext.dfmod",
        ];

        for path_str in &test_paths {
            let path = std::path::Path::new(path_str);
            if path.exists() {
                println!("\nTesting {}", path.display());

                let start = std::time::Instant::now();
                let paths = extract_dfmod_assets(path);
                let elapsed = start.elapsed();

                println!("  Found {} asset paths in {:?}", paths.len(), elapsed);
                for (i, p) in paths.iter().take(5).enumerate() {
                    println!("    {}: {}", i, p);
                }
                if paths.len() > 5 {
                    println!("    ... and {} more", paths.len() - 5);
                }

                // Basic assertion - should find at least some paths in a real dfmod
                assert!(!paths.is_empty(), "Should find asset paths in {}", path_str);
            }
        }
    }

    /// Benchmark comparing old binary scan approach vs new container approach
    /// This is marked as ignored for normal testing
    #[test]
    #[ignore]
    fn benchmark_extraction_methods() {
        use io_unity::unity_asset_view::UnityAssetViewer;
        use io_unity::unityfs::UnityFS;
        use std::collections::HashSet;

        let test_path = "/var/home/bazzite/.config/vmod/profiles/test/mods/ArchaeologistsGuild-2.6.1-14-2-6-1-1712582888/Mods/archaeologists.dfmod";
        let path = std::path::Path::new(test_path);

        if !path.exists() {
            println!("Test file not found, skipping benchmark");
            return;
        }

        // Warm up
        let _ = extract_dfmod_assets(path);

        // Test new approach (UnityAssetViewer container lookup)
        let iterations = 10;
        let mut viewer_times = Vec::new();
        let mut viewer_count = 0;

        for _ in 0..iterations {
            let file = File::open(path).unwrap();
            let reader = BufReader::new(file);

            let start = std::time::Instant::now();
            let mut viewer = UnityAssetViewer::default();
            if viewer
                .add_bundle_file(
                    Box::new(reader),
                    path.parent().map(|p| p.to_string_lossy().to_string()),
                )
                .is_ok()
            {
                let paths: Vec<_> = viewer
                    .container_maps
                    .keys()
                    .filter(|p| is_asset_path(p))
                    .collect();
                viewer_count = paths.len();
            }
            viewer_times.push(start.elapsed());
        }

        // Test old approach (binary scanning)
        let mut scan_times = Vec::new();
        let mut scan_count = 0;

        for _ in 0..iterations {
            let file = File::open(path).unwrap();
            let reader = BufReader::new(file);

            let start = std::time::Instant::now();
            let fs = UnityFS::read(Box::new(reader), None).unwrap();

            let mut all_paths = Vec::new();
            for cab_path in fs.get_cab_path() {
                if let Ok(data) = fs.get_file_data_by_path(&cab_path) {
                    let paths = extract_asset_paths_from_data(&data);
                    all_paths.extend(paths);
                }
            }
            let mut seen = HashSet::new();
            all_paths.retain(|p| seen.insert(p.clone()));
            scan_count = all_paths.len();
            scan_times.push(start.elapsed());
        }

        let viewer_avg = viewer_times.iter().sum::<std::time::Duration>() / iterations as u32;
        let scan_avg = scan_times.iter().sum::<std::time::Duration>() / iterations as u32;

        println!("\n=== Benchmark Results ({} iterations) ===", iterations);
        println!("Container approach: {:?} avg ({} paths)", viewer_avg, viewer_count);
        println!("Binary scan approach: {:?} avg ({} paths)", scan_avg, scan_count);
        println!(
            "Speedup: {:.2}x",
            scan_avg.as_secs_f64() / viewer_avg.as_secs_f64()
        );
    }
}

// Example to test dfmod parsing functionality
// Run with: cargo run --example parse_dfmod [path_to_dfmod]

use std::env;
use std::path::Path;
use vmod::mod_entry::extract_dfmod_assets;

fn main() {
    let args: Vec<String> = env::args().collect();

    let dfmod_path = if args.len() > 1 {
        &args[1]
    } else {
        // Default test file
        "/home/bazzite/.config/vmod/profiles/Daggerfall Overhaul/mods/Ambient Text 1.7-303-1-7-1743021606/Mods/ambienttext.dfmod"
    };

    println!("Parsing dfmod: {}", dfmod_path);
    println!("---");

    let assets = extract_dfmod_assets(Path::new(dfmod_path));

    println!("\n=== Extracted Asset Paths ===");
    for asset in &assets {
        println!("  {}", asset);
    }
    println!("\nTotal: {} assets", assets.len());
}

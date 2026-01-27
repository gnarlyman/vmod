# VMOD - Mod Organizer for Daggerfall Unity

A GTK4-based mod manager for Daggerfall Unity, written in Rust.

## Features

- **Modern GTK4 Interface** - Native Linux support with Wayland compatibility
- **Profile Management** - Multiple mod configurations with auto-detection of game folders
- **Mod List Display** - Drag-and-drop reordering with virtual filesystem deployment
- **Conflict Detection** - Visual file tree showing conflicts between mods
- **DFMods Support** - View and manage .dfmod asset bundles with cached extraction
- **Nexus Mods Integration** - Download mods directly via NXM protocol links
- **Downloads Manager** - Install, delete, and manage downloaded archives

## Installation

### Build from Source

**System Dependencies:**

Fedora/RHEL:
```bash
sudo dnf install gtk4-devel glib2-devel
```

Ubuntu/Debian:
```bash
sudo apt install libgtk-4-dev libglib2.0-dev
```

Arch:
```bash
sudo pacman -S gtk4
```

**Build:**
```bash
# Compile GSettings schema (required for development)
glib-compile-schemas resources/

# Build release binary
cargo build --release

# Run
./target/release/vmod
```

## Usage

1. **Create a Profile** - Select your Daggerfall Unity game folder
2. **Add Mods** - Place mod folders in the profile's mods directory or download via Nexus
3. **Reorder** - Drag mods to set load order (lower = higher priority)
4. **Check Conflicts** - Use the conflict panel to review file overwrites
5. **Apply Changes** - Deploy to the game's virtual filesystem

### Nexus Mods Integration

VMOD registers as an NXM protocol handler. Click "Download with Manager" on Nexus Mods to automatically download and queue mods for installation.

## Project Structure

```
vmod/
├── src/
│   ├── main.rs              # Entry point
│   ├── application.rs       # App lifecycle, NXM handling
│   ├── window/              # Main window
│   ├── mod_list/            # Mod list view and UI
│   ├── conflict_panel/      # Conflict detection UI
│   ├── preferences/         # Settings dialog
│   ├── nexus_api.rs         # Nexus Mods API client
│   ├── dfmod_cache.rs       # DFMod asset caching
│   └── tree_filter.rs       # Reusable tree search
└── resources/
    ├── window.ui            # GTK templates
    └── *.gschema.xml        # Settings schema
```

## License

MIT

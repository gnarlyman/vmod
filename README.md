# VMOD - Mod Organizer for Daggerfall Unity

A GTK4-based mod manager for Daggerfall Unity, written in Rust.

## Features

- **Modern GTK4 Interface** - Native Linux support with Wayland compatibility
- **Profile Management** - Multiple mod configurations with auto-detection of game folders
- **Mod List Display** - Drag-and-drop reordering with virtual filesystem deployment
- **Mod Sections** - Organize mods into collapsible categories with drag-and-drop
- **Auto-Sorting** - Rule-based load order sorting with transitive inference
- **Conflict Detection** - Visual file tree showing conflicts between mods
- **DFMods Support** - View and manage .dfmod asset bundles with cached extraction
- **Nexus Mods Integration** - Download mods directly via NXM protocol links
- **Downloads Manager** - Download queue with progress tracking, resume support, and archive extraction
- **Backup & Restore** - Save and restore mod configurations per profile
- **Game Launcher** - Launch Daggerfall Unity directly with log viewing

## Installation

### Pre-built Binaries

Download the latest release from [GitHub Releases](https://github.com/gnarlyman/vmod/releases).

Extract and run:
```bash
tar xzf vmod-linux-x86_64.tar.gz
./vmod
```

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

## Desktop Integration

### Install Desktop Entry & NXM Handler

Run the installation script to add VMOD to your application menu and register it as the NXM protocol handler:

```bash
./scripts/install-nxm-handler.sh
```

This will:
- Install `vmod.desktop` to `~/.local/share/applications/`
- Register VMOD to handle `nxm://` links from Nexus Mods
- Update the desktop database

### Manual Installation

If you prefer manual setup or installed from a release:

1. Copy the desktop file:
   ```bash
   cp resources/vmod.desktop ~/.local/share/applications/
   ```

2. Edit the `Exec=` line to point to your vmod binary:
   ```bash
   sed -i "s|Exec=vmod|Exec=/path/to/vmod|" ~/.local/share/applications/vmod.desktop
   ```

3. Register as NXM handler:
   ```bash
   xdg-mime default vmod.desktop x-scheme-handler/nxm
   ```

4. Update desktop database:
   ```bash
   update-desktop-database ~/.local/share/applications
   ```

### Testing NXM Links

Test that the handler works:
```bash
xdg-open 'nxm://daggerfallunity/mods/1/files/1'
```

VMOD should launch and begin downloading the mod.

## Usage

1. **Create a Profile** - Select your Daggerfall Unity game folder
2. **Add Mods** - Place mod folders in the profile's mods directory or download via Nexus
3. **Reorder** - Drag mods to set load order (lower = higher priority)
4. **Check Conflicts** - Use the conflict panel to review file overwrites
5. **Apply Changes** - Deploy to the game's virtual filesystem

### Organizing with Sections

Right-click mods to move them into sections. Create custom sections to group mods by type (textures, gameplay, UI, etc.).

### Auto-Sorting

VMOD can automatically sort mods based on predefined rules. Place a `sorting_rules.json` file in `~/.config/vmod/` with load order constraints:

```json
{
  "rules": [
    { "first": "mod_a", "then": "mod_b" },
    { "first": "mod_b", "then": "mod_c" }
  ]
}
```

Each rule specifies that `first` should load before `then`. The sorting system uses transitive inference - if you have rules A→B→C and mod B is missing, A will still be placed before C.

**Generating Rules from a Mod List**

Use the included Python script to generate sorting rules from a text file containing your desired mod order:

```bash
python scripts/generate_sorting_rules.py ~/Documents/modorder.txt sorting_rules.json
```

The script parses a bulleted list of mod names and generates consecutive pair rules. This is useful for converting community-recommended load orders into VMOD sorting rules.

### Backups

Create named backups before experimenting with load order. Restore anytime from the backup manager.

### Launching the Game

Use the Running Panel to launch Daggerfall Unity and view game logs directly in VMOD.

The Running Panel provides:
- **Executable Path** - Auto-detected from your profile's game folder (looks for `DaggerfallUnity.x86_64` or `DaggerfallUnity`), or browse to select manually
- **Environment Variables** - Custom env vars passed to the game process (default: `TERM=xterm` for proper terminal output handling)
- **Tabbed Log Viewing** - View game output, Player.log, and VMOD logs in separate tabs
- **Process Control** - Run/Stop buttons with status indicator

Configuration persisted to `~/.config/vmod/running_config.json`.

### Nexus Mods Integration

VMOD registers as an NXM protocol handler. Click "Download with Manager" on Nexus Mods to automatically download and queue mods for installation.

## File Locations

VMOD uses XDG-compliant directories for all data storage.

### Configuration (`~/.config/vmod/`)

| File/Directory | Description |
|----------------|-------------|
| `profiles.json` | List of game profiles |
| `profiles/<name>/` | Per-profile directory |
| `profiles/<name>/mods/` | Mods for this profile |
| `profiles/<name>/mod_state.json` | Enabled/disabled state and load order |
| `profiles/<name>/sections.json` | Custom section organization |
| `profiles/<name>/backups/` | Profile backups |
| `sorting_rules.json` | Auto-sorting load order rules (see [Auto-Sorting](#auto-sorting)) |
| `nexus.json` | Nexus Mods API credentials |
| `running_config.json` | Game launcher settings |

### Cache (`~/.cache/vmod/`)

| File/Directory | Description |
|----------------|-------------|
| `dfmod_cache.json` | Parsed .dfmod asset cache |
| `downloads/` | Downloaded mod archives from Nexus |
| `vmod.log` | Application log file |

### Game Directories

| Directory | Description |
|-----------|-------------|
| Game folder | Selected when creating a profile (contains `DaggerfallUnity.x86_64`) |
| `~/.config/unity3d/Daggerfall Workshop/Daggerfall Unity/` | Unity player settings and save data |

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

# VMOD - Mod Organizer for Daggerfall Unity

A GTK4-based mod manager for Daggerfall Unity, written in Rust.

## Features

### Phase 1 (Complete)
- Modern GTK4 interface with Wayland support
- Menu bar with File and Edit menus
- Keyboard shortcuts (Ctrl+Q to quit, Ctrl+, for preferences)
- Window state persistence (size and maximized state)

### Phase 2 (Complete)
- Profile management system
- Profile creation with game folder selection
- Auto-detection of launcher executable (DaggerfallUnity.x86_64)
- Auto-initialization of Mods.json path
- Profile dropdown selector with dynamic updates
- Profile selection persistence across sessions
- Full test coverage (27 tests)

## Build Requirements

### System Dependencies

**Fedora/RHEL:**
```bash
sudo dnf install gtk4-devel glib2-devel
```

**Ubuntu/Debian:**
```bash
sudo apt install libgtk-4-dev libglib2.0-dev
```

**Arch:**
```bash
sudo pacman -S gtk4
```

### Rust

Install Rust via [rustup](https://rustup.rs/):
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

## Building

```bash
# Compile GSettings schema (required for development)
glib-compile-schemas resources/

# Build the project
cargo build

# Run the application
cargo run
```

## Project Structure

```
vmod/
├── Cargo.toml              # Project manifest
├── build.rs                # GResource compilation
├── resources/              # GTK resources
│   ├── resources.gresource.xml   # Resource bundle
│   ├── window.ui           # Main window template
│   ├── menu.ui             # Menu definition
│   └── org.vmod.VMOD.gschema.xml  # Settings schema
└── src/
    ├── main.rs             # Entry point
    ├── application.rs      # App lifecycle & actions
    ├── config.rs           # Constants
    ├── window/             # Main window
    │   ├── mod.rs
    │   └── imp.rs
    └── preferences/        # Preferences dialog
        ├── mod.rs
        └── imp.rs
```

## Development Roadmap

- **Phase 1** (Complete): GTK4 foundation with menu bar
- **Phase 2** (Complete): Profile management with auto-detection
- **Phase 3**: Mod list display and virtual filesystem
- **Phase 4**: Plugin order management (Mods.json)
- **Phase 5**: Download manager with Nexus integration

## License

TBD

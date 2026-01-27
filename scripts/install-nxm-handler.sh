#!/bin/bash
# Install VMOD as the NXM protocol handler for Nexus Mods

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

# Determine the vmod binary path
if [ -f "$PROJECT_DIR/target/release/vmod" ]; then
    VMOD_BIN="$PROJECT_DIR/target/release/vmod"
elif [ -f "$PROJECT_DIR/target/debug/vmod" ]; then
    VMOD_BIN="$PROJECT_DIR/target/debug/vmod"
else
    echo "Error: vmod binary not found. Build with 'cargo build' first."
    exit 1
fi

VMOD_BIN="$(realpath "$VMOD_BIN")"

# Create applications directory if needed
APPS_DIR="$HOME/.local/share/applications"
mkdir -p "$APPS_DIR"

# Generate desktop file with correct path
cat > "$APPS_DIR/vmod.desktop" << EOF
[Desktop Entry]
Type=Application
Name=VMOD
Comment=Mod manager for Daggerfall Unity
Exec=$VMOD_BIN %u
Icon=vmod
Terminal=false
Categories=Game;Utility;
MimeType=x-scheme-handler/nxm;
StartupNotify=true
NoDisplay=false
EOF

echo "Installed desktop file to: $APPS_DIR/vmod.desktop"
echo "Binary path: $VMOD_BIN"

# Register as NXM handler
xdg-mime default vmod.desktop x-scheme-handler/nxm

echo "Registered vmod as NXM protocol handler"

# Update desktop database
if command -v update-desktop-database &> /dev/null; then
    update-desktop-database "$APPS_DIR" 2>/dev/null || true
fi

echo ""
echo "Done! VMOD is now registered to handle nxm:// links."
echo "Test with: xdg-open 'nxm://daggerfallunity/mods/1/files/1'"

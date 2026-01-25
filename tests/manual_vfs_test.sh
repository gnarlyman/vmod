#!/bin/bash
# Manual VFS testing script
# Run this to verify symlinks are working with your actual mods

set -e

PROFILE_NAME="${1:-test}"
GAME_PATH="${2:-$HOME/.local/share/Steam/steamapps/common/Daggerfall Unity}"

PROFILE_MODS="$HOME/.config/vmod/profiles/$PROFILE_NAME/mods"
STREAMING_ASSETS="$GAME_PATH/DaggerfallUnity_Data/StreamingAssets"

echo "=== VMOD VFS Test ==="
echo "Profile: $PROFILE_NAME"
echo "Profile mods: $PROFILE_MODS"
echo "Game StreamingAssets: $STREAMING_ASSETS"
echo ""

# Check if paths exist
if [ ! -d "$PROFILE_MODS" ]; then
    echo "❌ Profile mods folder doesn't exist: $PROFILE_MODS"
    exit 1
fi

if [ ! -d "$STREAMING_ASSETS" ]; then
    echo "❌ Game StreamingAssets folder doesn't exist: $STREAMING_ASSETS"
    exit 1
fi

echo "✅ Paths exist"
echo ""

# List mods in profile
echo "=== Mods in profile ==="
mod_count=0
for mod_dir in "$PROFILE_MODS"/*; do
    if [ -d "$mod_dir" ]; then
        mod_name=$(basename "$mod_dir")
        echo "  - $mod_name"

        # Show what folders this mod has
        for folder in Mods Textures Sound Music Books Docs Text QuestPacks Fonts; do
            if [ -d "$mod_dir/$folder" ]; then
                file_count=$(find "$mod_dir/$folder" -type f 2>/dev/null | wc -l)
                echo "    └─ $folder/ ($file_count files)"
            fi
        done
        mod_count=$((mod_count + 1))
    fi
done
echo "Total mods: $mod_count"
echo ""

# Check for symlinks in StreamingAssets
echo "=== Symlinks in StreamingAssets ==="
symlink_count=0
for folder in Mods Textures Sound Music Books Docs Text QuestPacks Fonts; do
    folder_path="$STREAMING_ASSETS/$folder"
    if [ -d "$folder_path" ]; then
        links=$(find "$folder_path" -type l 2>/dev/null | wc -l)
        if [ $links -gt 0 ]; then
            echo "  $folder/: $links symlinks"
            symlink_count=$((symlink_count + links))
        fi
    fi
done
echo "Total symlinks: $symlink_count"
echo ""

# Show example symlinks
echo "=== Example Symlinks (first 10) ==="
symlink_examples=0
for folder in Mods Textures Sound Music Books Docs Text; do
    folder_path="$STREAMING_ASSETS/$folder"
    if [ -d "$folder_path" ]; then
        while IFS= read -r -d '' link; do
            target=$(readlink "$link")
            rel_link="${link#$STREAMING_ASSETS/}"
            echo "  $rel_link"
            echo "    → $target"
            symlink_examples=$((symlink_examples + 1))
            if [ $symlink_examples -ge 10 ]; then
                break 2
            fi
        done < <(find "$folder_path" -type l -print0 2>/dev/null)
    fi
done
echo ""

# Test for conflicts (multiple mods with same file)
echo "=== Checking for file conflicts ==="
declare -A file_sources
conflict_found=0

for mod_dir in "$PROFILE_MODS"/*; do
    if [ -d "$mod_dir" ]; then
        mod_name=$(basename "$mod_dir")
        for folder in Mods Textures Sound Music Books Docs Text; do
            if [ -d "$mod_dir/$folder" ]; then
                while IFS= read -r -d '' file; do
                    rel_path="${file#$mod_dir/$folder/}"
                    key="$folder/$rel_path"

                    if [ -n "${file_sources[$key]}" ]; then
                        if [ $conflict_found -eq 0 ]; then
                            echo "  Conflicts detected (later mod should win):"
                            conflict_found=1
                        fi
                        echo "    $key"
                        echo "      Previously: ${file_sources[$key]}"
                        echo "      Now: $mod_name (should override)"
                    fi
                    file_sources[$key]="$mod_name"
                done < <(find "$mod_dir/$folder" -type f -print0 2>/dev/null)
            fi
        done
    fi
done

if [ $conflict_found -eq 0 ]; then
    echo "  ✅ No conflicts detected"
fi
echo ""

# Verify a symlink points to correct source
echo "=== Symlink Verification ==="
if [ $symlink_count -gt 0 ]; then
    # Find first symlink
    first_link=$(find "$STREAMING_ASSETS" -type l 2>/dev/null | head -1)
    if [ -n "$first_link" ]; then
        target=$(readlink "$first_link")
        echo "  Testing: $(basename "$first_link")"
        echo "  Target: $target"

        if [ -e "$target" ]; then
            echo "  ✅ Symlink target exists"

            # Check if target is in profile mods
            if [[ "$target" == *"$PROFILE_MODS"* ]]; then
                echo "  ✅ Points to profile mods folder"
            else
                echo "  ⚠️  Points outside profile mods folder"
            fi
        else
            echo "  ❌ Symlink target doesn't exist (broken symlink)"
        fi
    fi
else
    echo "  No symlinks to verify"
fi
echo ""

# Summary
echo "=== Summary ==="
echo "Mods in profile: $mod_count"
echo "Symlinks created: $symlink_count"
if [ $symlink_count -gt 0 ] && [ $mod_count -gt 0 ]; then
    echo "✅ VFS appears to be working!"
    echo ""
    echo "Next steps:"
    echo "  1. Enable/disable mods in VMOD UI"
    echo "  2. Run this script again to see symlink count change"
    echo "  3. Check mod priority by enabling conflicting mods"
else
    echo "⚠️  No symlinks found. Enable some mods in VMOD first!"
fi

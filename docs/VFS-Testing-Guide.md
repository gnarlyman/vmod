# VFS Testing Guide

This guide explains how to verify the Virtual File System is working correctly.

## Automated Tests (Already Passing ✅)

Run the integration tests:
```bash
distrobox enter rust-dev -- cargo test --test vfs_integration_tests
```

**All 6 tests passing confirms:**
- ✅ Load order priority (later mods override earlier)
- ✅ Nested directories work correctly
- ✅ All DFU folder types supported
- ✅ Symlink cleanup works
- ✅ Multi-mod priority handling
- ✅ Actual symlinks created (not copies)

---

## Manual Testing with Your Mods

### 1. Run the Test Script

```bash
./tests/manual_vfs_test.sh
```

Or specify a profile and game path:
```bash
./tests/manual_vfs_test.sh "My Profile" "/path/to/Daggerfall Unity"
```

**What it checks:**
- Lists all mods in your profile
- Shows what folders each mod contains
- Counts total symlinks in StreamingAssets
- Shows example symlinks and their targets
- Detects file conflicts between mods
- Verifies symlinks point to correct sources

### 2. Interactive Testing

**Step-by-step verification:**

1. **Start VMOD**
   ```bash
   distrobox enter rust-dev -- cargo run
   ```

2. **Select your profile**
   - Choose "test" or your profile name

3. **Check initial state** (in another terminal)
   ```bash
   ls -la ~/.local/share/Steam/steamapps/common/Daggerfall\ Unity/DaggerfallUnity_Data/StreamingAssets/Mods/
   ```
   Should be empty or have no symlinks yet.

4. **Enable one mod**
   - Check the box next to a mod in VMOD

5. **Verify symlinks created**
   ```bash
   ./tests/manual_vfs_test.sh
   ```
   Should show symlinks in StreamingAssets pointing to your profile's mods.

6. **Enable a second mod**
   - If it has files that conflict with the first mod, check which wins

7. **Verify load order**
   ```bash
   ls -la ~/.local/share/Steam/steamapps/common/Daggerfall\ Unity/DaggerfallUnity_Data/StreamingAssets/Textures/
   ```
   Symlinks should point to the mod with **higher order number**.

8. **Disable a mod**
   - Uncheck the box
   - Run test script again
   - Symlinks should be gone and replaced with lower-priority mod's files

### 3. Testing Specific Scenarios

**Test nested files:**
```bash
# Find a mod with nested structure like Textures/UI/something.png
# Enable it, then check:
find ~/.local/share/Steam/steamapps/common/Daggerfall\ Unity/DaggerfallUnity_Data/StreamingAssets/Textures -type l
```
All nested files should have symlinks.

**Test priority with conflicting mods:**
1. Find two mods that have the same file (e.g., both have `Textures/sky.png`)
2. Enable the first mod (order 0)
   ```bash
   readlink ~/.local/.../StreamingAssets/Textures/sky.png
   # Should point to first mod
   ```
3. Enable the second mod (order 1)
   ```bash
   readlink ~/.local/.../StreamingAssets/Textures/sky.png
   # Should now point to second mod (higher priority)
   ```

**Test all folder types:**
Enable mods that use different folders:
- Mods/ (.dfmod files)
- Textures/ (images)
- Sound/ (audio)
- Music/ (music files)
- Books/ (book content)
- Text/ (text files)
- QuestPacks/ (quests)

Verify each creates symlinks in the corresponding StreamingAssets subfolder.

---

## Expected Behavior

### ✅ Correct Behavior

**When mod enabled:**
```bash
$ readlink StreamingAssets/Textures/forest.png
/home/user/.config/vmod/profiles/test/mods/TexturePack/Textures/forest.png
```

**When higher-priority mod enabled:**
```bash
# Before (mod order 0):
$ readlink StreamingAssets/Textures/forest.png
.../profiles/test/mods/TexturePackA/Textures/forest.png

# After enabling mod order 1:
$ readlink StreamingAssets/Textures/forest.png
.../profiles/test/mods/TexturePackB/Textures/forest.png
```

**When mod disabled:**
```bash
# Symlink removed:
$ ls StreamingAssets/Textures/forest.png
ls: cannot access 'forest.png': No such file or directory

# OR if another mod has it, points to that:
$ readlink StreamingAssets/Textures/forest.png
.../profiles/test/mods/TexturePackA/Textures/forest.png
```

### ❌ Incorrect Behavior (Report if you see this)

**Broken symlinks:**
```bash
$ readlink StreamingAssets/Textures/forest.png
/some/nonexistent/path/forest.png
```

**Files copied instead of symlinked:**
```bash
$ file StreamingAssets/Textures/forest.png
forest.png: PNG image data  # Should say "symbolic link"
```

**Wrong priority:**
```bash
# Lower priority mod winning over higher priority
# (order 0 mod's file used when order 2 mod also has it)
```

**Files not symlinked:**
```bash
# Mod has Textures/sky.png but StreamingAssets/Textures/sky.png doesn't exist
```

---

## Debugging Commands

**Count symlinks per folder:**
```bash
for folder in Mods Textures Sound Music Books; do
  count=$(find ~/.local/.../StreamingAssets/$folder -type l 2>/dev/null | wc -l)
  echo "$folder: $count symlinks"
done
```

**Find all symlinks:**
```bash
find ~/.local/share/Steam/steamapps/common/Daggerfall\ Unity/DaggerfallUnity_Data/StreamingAssets -type l
```

**Show where symlinks point:**
```bash
find ~/.local/.../StreamingAssets -type l -exec sh -c 'echo "{}"; readlink "{}"' \; | head -20
```

**Check for broken symlinks:**
```bash
find ~/.local/.../StreamingAssets -type l ! -exec test -e {} \; -print
```
(Should return nothing if all symlinks valid)

**Compare mod order to symlink targets:**
```bash
# List mods in order
ls -v ~/.config/vmod/profiles/test/mods/

# For a specific file, see which mod it points to
readlink ~/.local/.../StreamingAssets/Textures/some_file.png | grep -o 'mods/[^/]*'
```

---

## Troubleshooting

**No symlinks created:**
- Check VMOD output for errors
- Verify profile mods folder exists
- Verify game path is correct
- Check mods have recognized folders (Mods/, Textures/, etc.)

**Wrong file priority:**
- Check mod order numbers in VMOD
- Lower order = lower priority
- Higher order = higher priority (wins conflicts)

**Symlinks not updating:**
- Try disabling all mods, then re-enabling
- Check VMOD console for VFS rebuild messages

**Game doesn't see mods:**
- Verify symlinks point to correct StreamingAssets
- Check game's Data folder path in profile
- Ensure StreamingAssets path is correct for your OS

---

## Test Summary

If you've completed these tests successfully:

- [ ] Ran automated tests (6/6 passing)
- [ ] Ran manual test script
- [ ] Enabled a mod and verified symlinks
- [ ] Enabled conflicting mods and verified priority
- [ ] Disabled a mod and verified cleanup
- [ ] Tested nested directories
- [ ] Tested multiple folder types

**You can be confident the VFS is working correctly!** 🎉

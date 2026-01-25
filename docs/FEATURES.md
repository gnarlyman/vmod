# VMOD Features

## Profile Management

### Profile Selection Persistence

The application now remembers which profile is selected across sessions.

**How it works:**
1. **On Startup**: The application loads the profile list and automatically selects the last active profile in the dropdown
2. **On Selection Change**: When you select a different profile from the dropdown, the application:
   - Updates the active profile in memory
   - Saves the selection to disk (`~/.config/vmod/profiles.json`)
   - Persists across application restarts

**User Experience:**
- Create multiple profiles for different game installations
- Switch between profiles using the dropdown
- Close and reopen the application - your selected profile is remembered
- No need to manually save or select your profile each time

**Technical Details:**
- Selection changes are captured via `connect_selected_item_notify` signal
- Active profile index is saved immediately when changed
- Profile data persists to `~/.config/vmod/profiles.json`
- First profile added automatically becomes the active profile

### Profile Validation

Profiles are validated to ensure they point to valid Daggerfall Unity installations:
- Checks for `DaggerfallUnity` or `DaggerfallUnity.x86_64` executable
- Prevents creation of invalid profiles
- Shows error messages for missing game installations

### Dynamic Profile Updates

The profile dropdown updates in real-time when you create a new profile:
- No need to restart the application
- New profiles appear immediately in the dropdown
- Newly created profile is automatically selected

## Testing

All profile management features are thoroughly tested:
- **14 unit tests** covering core functionality
- **6 integration tests** for workflows
- Test coverage includes profile persistence, selection, and validation

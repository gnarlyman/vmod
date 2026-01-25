# VMOD

VMOD is a mod organizer that allows users to easily manage their mods and modpacks. It provides a user-friendly interface for installing, updating, and removing mods.

## Features

- Easy installation and removal of mods
- Ability to create and manage modpacks
- User-friendly interface for browsing and searching mods

## UI
- Menu bar
  - File
    - Exit
  - Edit
    - Preferences
- Row
  - Profile row
  - Create/Select a profile
  - Profiles consist of:
    - The selected game (Daggerfall Unity), determines where we look for game's settings files, plugin order.
      - user specifies game's folder
      - validate by confirming the games exe is present. 
    - Launcher (which executables the user has specified for the game)
    - Plugin order (derived from Game's specific plugin order file, Mods.json in the case of Daggerfall Unity)
      - user specifies location of the Mods.json file
    - Changing the profile will reset all below contents.
    - Profile is required to be selected/created before any other settings can be changed.
- Row
  - Divided into two columns
  - Left column is the "mod list" displays a list of mods, table with checkboxes
    - header: Title, Version, order number (based on folder position in the list)
    - filter text input to the right of the header, filters the list of mods.
    - this is the mods folder which contains many installed mod folders containing mod assets, including textures, models, and scripts, and one or more plugin files if applicable
    - a checkbox will cause the mod files to be visible to the executable selected in the launcher section
  - Right column divided into two rows
      - Top row is the "launcher" where you configure the game launcher/executable
      - Bottom row is the "plugin order" which displays the order of the mod plugins
          - Drag and drop to reorder plugins
          - table with checkboxes displaying the mod plugins
              - Checkbox to enable/disable the mod plugin
              - Name, Version, Order number
- Row
  - Downloads
    - Displays download in progress and mods that have already been downloaded
    - Can install downloaded mods, will extract and place in the mods folder, displays in UI as unchecked by default.
## folder structure
 - project root
  - VMOD executable
  - profile folder (user named)
    - profile settings files (user specified settings from ui)
    - mods
      - mod1 (installed)
        - textures
        - models
        - scripts
        - plugin1.xyz (could be .dll, .dfmod, .esp)
        - plugin2.xyz
      - mod2 (installed)
      - mod3 (installed)
    - downloads
      - mods downloaded from internet, usually archives
  
Features:
 - Nexus integration, download mods directly from the Nexus website using the Download with Mod Manager button in the nexus page. Requires link integration.
 - Displays download status
 - Can install mods by decompressing (if needed) the files into the mods folder under its own mod folder named after the mod's name.
  - Custom mod handling, when expected folders are not present, the mod will not be installed until the user specifies the correct folder structure. An error should appear if the folder structure is incorrect. Mod folders should have at least one of textures, models, scripts, or plugin files.
 - Mods can be removed by selecting the mod and clicking the remove button (right-click menu also)
 - Virtual filesystem. Enabled folders are made visible to the executable selected in the launcher section in a way that the executable expects. In the case of Daggerfall Unity, [Game Folder]/DaggerfallUnity_Data/StreamingAssets/Mods/. So when we run the game's executable, all the contents of the enabled mods are made visible to the executable in this folder.
 - Linux only support for now. Wayland.
 - Folder structure is auto-created by the exe on first run. 
 
 
 Mods.json example:
 [
     {
         "FileName": "UIBackdrop",
         "Title": "UI Backdrop",
         "Enabled": true,
         "LoadPriority": 0
     },
     {
         "FileName": "advanced dialogue",
         "Title": "Advanced Dialogue",
         "Enabled": true,
         "LoadPriority": 1
     },
     {
         "FileName": "advanced wilderness encounters",
         "Title": "Advanced Wilderness Encounters",
         "Enabled": true,
         "LoadPriority": 2
     },
     {
         "FileName": "adventure finds you",
         "Title": "Adventure Finds You",
         "Enabled": true,
         "LoadPriority": 3
     }
]

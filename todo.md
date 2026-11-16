# TODO
## Application Improvements
- [ ] Keybinds, and control scheme docs
- [ ] Move settings back to proc macros
- [ ] Unresized cover cache, and cover size option with different caches for different sizes
- [ ] Add more refined database access limitations
- [ ] Profiles
- [ ] To top button on games view
- [ ] Options for native games, gamescope, proton, etc.
- [ ] Move database access to daemon
- [ ] Remove usage of settings from as many places as possible
- [ ] Comment main crate

## QoL
- [ ] Quick tag add
- [ ] Hidden tags
- [ ] Minimize to tray

## Ui Modularity
- [ ] Process info in terminal window
- [ ] Panes for terminal window (perhaps unify terminal and main window)
- [ ] Game info in terminal window (with setting)
- [ ] Thumbnail size preferences

## Scripting/IPC
- [ ] Do not require GUI application for install
- [ ] Add cli and json api for querying and modifying game configs

## Fixes
- [ ] Stdout of prefix initializiation not shown until game runs

## Future Goals/Possibilities
- [ ] Use separate library for context menu, allowing it to be shown outside application

## Passive
- [ ] More context menus
- [ ] Icons where appropriate
- [ ] Decouple from lutris
- [ ] Split up spel-katalog more

# Finished
## Application Improvements
- [x] Add dedicated crate for information gathering (thumbnails, lutris, additional)
- [x] Detangle batch running from other application state (possibly remove tracker)
- [x] Move dedicated batch run to subcommand
- [x] Dedicated lua api docs widget
- [x] Split settings to dedicated window, info should be kept as-is
- [x] Console Window
- [x] In info and batch view, give editors dark backgrounds at all times
- [x] Merge process and batch view with info view, with some kind of switching mechanism
- [x] Remove current lua scripting 
- [x] Use xdg base dirs for some settings
- [x] Http for ipc
- [x] Use iced grid for game view
- [x] Use iced sensor for game cards
- [x] Profiling crate with feature flag to enable
- [x] Deamon for spawning game processes
- [x] Logging in terminal window

## QoL
- [x] Log filtering
- [x] Process info for daemon run games
- [x] Disable games (shadow lutris but do not show)
- [x] Multiple tag filter layers (ui, backend already finished)
- [x] Show module path in log view
- [x] Split of process view to crate
- [x] Simplified/collapsed process view tree
- [x] Do not log no such directory as an error in process open (as it is expected after the process is closed)
- [x] Aquire old running process ids on startup.

## Fixes
- [x] Dummy exe of installer not "moved"
- [x] Install drive of installer not canonicalized

## Future Goals/Possibilities
- [x] Run exe files in prefix without lutris
- [x] Add categories
- [x] Bubblewrap instead of/in addition to firejail

## QoL
- [x] Log filtering
- [x] Process info for daemon run games
- [x] Disable games (shadow lutris but do not show)
- [x] Multiple tag filter layers (ui, backend already finished)
- [x] Show module path in log view
- [x] Split of process view to crate
- [x] Simplified/collapsed process view tree
- [x] Do not log no such directory as an error in process open (as it is expected after the process is closed)
- [x] Aquire old running process ids on startup.

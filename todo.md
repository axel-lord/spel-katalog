# TODO List For Application Improvements
- [x] Add dedicated crate for information gathering (thumbnails, lutris, additional)
- [x] Detangle batch running from other application state (possibly remove tracker)
- [x] Move dedicated batch run to subcommand
- [x] Dedicated lua api docs widget
- [ ] ~~Expand dialog functionality~~
- [ ] ~~Expand lua api, remove cover, get lutris cover~~
- [ ] Keybinds, and control scheme docs
- [x] Split settings to dedicated window, info should be kept as-is
- [x] Console Window
- [ ] Move settings back to proc macros
- [ ] ~~In info view rework buttons~~
- [x] In info and batch view, give editors dark backgrounds at all times
- [x] Merge process and batch view with info view, with some kind of switching mechanism
- [ ] Unresized cover cache, and cover size option with different caches for different sizes
- [ ] Add more refined database access limitations
- [x] Remove current lua scripting 
- [x] Use xdg base dirs for some settings
- [ ] Profiles
- [x] Http for ipc
- [ ] External scripting using ipc
- [x] Use iced grid for game view
- [x] Use iced sensor for game cards
- [x] Profiling crate with feature flag to enable
- [x] Deamon for spawning game processes
- [x] Logging in terminal window
- [ ] To top button on games view
- [ ] Options for native games, gamescope, proton, etc.
- [ ] Move database access to daemon

# QoL
- [ ] Log filtering
- [ ] Process info in terminal window
- [x] Process info for daemon run games
- [x] Disable games (shadow lutris but do not show)
- [ ] Quick tag add
- [ ] Hidden tags
- [ ] Use cache for game filters
- [ ] Multiple tag filter layers (ui, backend already finished)
- [ ] Game info in terminal window (with setting)
- [ ] Panes for terminal window (perhaps unify terminal and main window)
- [x] Show module path in log view
- [x] Split of process view to crate
- [x] Simplified/collapsed process view tree
- [x] Do not log no such directory as an error in process open (as it is expected after the process is closed)
- [ ] Aquire old running process ids on startup.

# Fixes
- [x] Dummy exe of installer not "moved"
- [x] Install drive of installer not canonicalized

# Future Goals/Possibilities
- [x] Run exe files in prefix without lutris
- [x] Add categories
- [x] Bubblewrap instead of/in addition to firejail

# Passive
- [ ] More context menus
- [ ] Icons where appropriate
- [ ] Decouple from lutris
- [ ] Split up spel-katalog more

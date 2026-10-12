### Fixed

- **purlis checks for git before it runs it, so a Mac without git no longer shows a surprise
  system dialog.** On a Mac without Apple's command line developer tools, `/usr/bin/git` is a
  stub that opens the system's install dialog. purlis now asks `xcode-select -p` first (and
  looks for git on `PATH` elsewhere), and runs no git while it is missing. The window can show
  its own guided screen instead: Install opens Apple's installer, Check again looks again, and
  the action you started carries on once git is there. On Linux the screen names your
  distribution's package manager line (#1813).

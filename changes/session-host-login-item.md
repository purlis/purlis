### Added

- **purlis keeps running in the background while something needs it.** Once you have a trigger
  or a paired device, Quit leaves your chats running: the window closes, and the menu bar says
  why purlis is still running, with *Open purlis* and *Stop purlis and end every chat*. purlis
  starts again in the background when you log in. Neither triggers nor paired devices are in
  purlis yet, so for now nothing changes, and Quit ends every chat as before. On macOS a launchd agent runs the session host, and on Linux a systemd
  user unit, each starting it again if it fails (FD-17, #655).

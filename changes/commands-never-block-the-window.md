### Changed

- **A project switch no longer waits on the disk or on git.** The reads a switch makes of every
  pane and notice on screen (the sandbox's state, a chat's usage gauge, its persona grants, the
  network record) are answered off the window's thread, and so is every command that reads or
  writes a file, runs a program or starts a chat. The window's tabs are written down behind it.
  Where the window reads and changes the same thing, the answers still come in the order asked
  (#1949).

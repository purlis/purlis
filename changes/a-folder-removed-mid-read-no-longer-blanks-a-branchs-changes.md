### Fixed

- **A folder removed while the explorer reads a branch no longer blanks its changes.** The read
  of files git does not track starts again, and the folder is simply gone from it. On
  Linux, a folder swapped for a link just before it is watched is no longer watched through the
  link (#1130).

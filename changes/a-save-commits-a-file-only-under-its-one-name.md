### Changed

- **A save commits a file only under the one name it has on disk.** A save of the project or of
  a workspace's repo refuses a file with a second name (a hard link), as purlis's file readers
  already do, and a file that changed or went away after it was staged. It names each one, takes
  it out of the index again, and makes no commit. On macOS, the sandbox lets a chat make a hard
  link only to a file in the folders it may write, so git, cargo and npm keep making their own
  links as before.

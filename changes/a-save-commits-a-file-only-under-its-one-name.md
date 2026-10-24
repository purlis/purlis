### Changed

- **A save commits a file only under the one name it has on disk.** A save of the project or of
  a workspace's repo refuses a file with a second name (a hard link), as purlis's file readers
  already do, and a file that changed or went away after it was staged. It names each one, takes
  it out of the index again, and makes no commit. A file with a second name is refused before
  anything is staged, a refused file's contents are removed from the repository's own store
  where nothing else uses them, and a save of a skill's files stages each file from the one
  open file it checked. On macOS, the sandbox lets a chat make a hard
  link only to a file in the folders it may write, so git, cargo and npm keep making their own
  links as before.
  A commit the app makes for a sandboxed chat in its branch folder, and the files
  `purlis worktree update --continue` stages once a conflict is resolved, hold to the same
  rule; a resolved file refused there is marked as conflicted again. So does Continue on a
  project's save conflict in the window.

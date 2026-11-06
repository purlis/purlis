### Fixed

- **`purlis worktree prune` from a chat now answers on a large workspace.** On about 290 branch
  folders it gave up after two minutes with "os error 35" and printed no list, though the app
  went on and finished the prune about 20 minutes later. A chat now waits for as long as the app
  is still working through folders, and the app reads several folders at once, so the same
  prune takes a fraction of the time. If a chat does stop waiting, it says plainly what was
  removed: nothing for a dry run, or the folders of the workspace removed since it asked
  (#5439).

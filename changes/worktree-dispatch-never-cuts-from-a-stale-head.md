### Fixed

- A task dispatched to a worktree is never started from the commit the clone happens to be on. When the clone's remote does not say which branch is its default, the dispatch is refused with the reason. Asking the remote is now bounded to seconds and tried once more. A task started by a chat that has a forge route gets one too, or it is not started (#7870).

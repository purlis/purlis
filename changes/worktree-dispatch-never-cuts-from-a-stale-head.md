### Fixed

- A task dispatched to a worktree is never started from the commit the clone happens to be on. It starts from the remote's default branch, fetched just before. If the fetch fails, it starts from the default as the clone last fetched it, and says how long ago that was. It is refused, with the reason, only when the clone has fetched no default at all. Asking the remote is bounded to seconds and tried once more after a short wait. A task started by a chat whose forge route covers the task's repo gets a route to that repo too, or it is not started (#7870).

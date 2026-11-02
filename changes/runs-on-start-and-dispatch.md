### Added

- **Choose where one chat runs as you start it.** Start chat has a Runs on row that says where
  the new chat runs and why (its workspace, its project, or this machine), and takes a pool or
  this machine for that start only, without changing a setting. When the pool is gone, stopped
  or not reached, nothing starts: it says why and offers Try again, Pool setup, or Run on this
  machine this once (#4536).
- **`purlis dispatch --pool <name>` and `--this-machine`** name where a task runs, before its
  workspace's and its project's own. When a chat names a place other than its workspace's, the
  task waits for your Approve, and the ask and any grant Notice say where it would run (#4536).

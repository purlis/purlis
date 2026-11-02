### Added

- **Choose where one chat runs as you start it.** Start chat has a Runs on row that says where
  the new chat runs and why (its workspace, its project, or this machine), and takes a pool or
  this machine for that start only, without changing a setting. When the pool is gone, stopped
  or not reached, nothing starts: it says why and offers Try again, Pool setup, or Run on this
  machine this once (#4536).
- **`purlis dispatch --pool <name>` and `--this-machine`** start a task where you name, before
  its workspace's and its project's own. A chat on a pool's runner still waits for your Approve
  for any dispatch, and the held ask says where the task would run (#4536).

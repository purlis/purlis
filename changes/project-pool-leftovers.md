### Fixed

- **A project's pool asks only about workspaces that ran there.** When a chat ends, or the window
  comes back into focus, purlis fetches work back from a project's pool only for workspaces that have run a chat there. A
  project is marked as not held on a pool only while a workspace follows that pool. If a workspace
  folder is removed or renamed outside purlis, a pool chosen for it becomes This machine when the
  project opens, so a new folder with that name never runs on a pool nobody chose for it (#4591).

### Added

- **Start chat says when a workspace's chat is its first on a pool.** Before anything is sent, the
  Runs on row says that the workspace's code will be copied to that pool's runner, and that the
  chat's work comes back here when it ends (#4591).

### Fixed

- **A project's pool asks only about workspaces that ran there.** When a chat ends, or the window
  comes back into focus, purlis fetches work back only for workspaces that have run a chat on the
  pool. A project is marked as not held on a pool only while a workspace follows it. Runs on kept
  for a workspace folder that was removed or renamed outside purlis is forgotten when the project
  opens, so a new workspace with that name follows the project (#4591).

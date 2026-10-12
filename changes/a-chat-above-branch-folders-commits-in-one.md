### Changed

- **A sandboxed chat above its branch folders can commit in one.** `purlis worktree commit`
  from a workspace or the project's top takes paths that start with the branch folder
  (`.worktrees/<repo>/<piece>/…`). Claude Code no longer asks before the command when it runs
  alone, a chat is held to 30 brokered commits in ten minutes, a message carrying purlis's own
  provenance trailers is refused, and a commit purlis's scan refuses is a needs-you item
  (#1476).
- **A folder purlis will not read through its git link says why.** A checkout below a workspace
  that keeps its git directory elsewhere is named as such, a folder only named `workspaces` is no
  longer held to a project's rule, and purlis's git reads find your global excludes under
  `XDG_CONFIG_HOME` (#1476).

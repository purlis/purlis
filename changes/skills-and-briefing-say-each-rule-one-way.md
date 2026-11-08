### Changed

- **The built-in skills and a chat's briefing say each rule one way.** A website login is the
  `browser_login` tool; a chat commits in a branch folder with `purlis worktree commit`, in a
  terminal too; a helper does read-only work; a chat does what it can itself and dispatches when
  the work needs another persona; a sandboxed chat's change is saved by the app and a terminal
  chat runs `purlis save`; `--force` only on your word; a chat never asks for a secret's value;
  it asks you through `ask_operator`; and a task writes its session record before it reports.
  A check fails the build when a shipped skill, the briefing or a purlis tool says the old side
  again (#5722).

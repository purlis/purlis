### Changed

- **Branch-folder work from a harness subagent goes to a purlis task.** A subagent's
  `purlis worktree add`, `commit` and `push` are refused at the hook with one line: this work
  is a purlis task, so ask your chat to dispatch it. The chat's own calls, and a subagent's
  reads, searches and reviews, run as before, in Claude Code and in Codex. A chat's briefing
  now says which work is a task's (anything that writes, commits, needs its own branch folder
  or runs long) and which a subagent's (#4773).

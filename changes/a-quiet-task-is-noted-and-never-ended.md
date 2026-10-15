### Added

- **purlis tells you when a task goes quiet.** While a task works, purlis watches what its
  harness reports: a turn, a tool starting or coming back. After 15 minutes with nothing, or one
  command running for 30, it says so on the task's tab and to the chat that asked, naming the
  command's program where it can ("running one command for 30 minutes: `cargo`"). The note
  ends nothing, and comes down when the task is heard from again. Both times are settings in
  Settings › Dispatch: _Quiet note after_ and _One command note after_ (`quiet-note-minutes`,
  `one-command-note-minutes`), per project, workspace or persona (#1981).

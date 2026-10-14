### Changed

- **Reading a task works as a tool and asks nobody.** Claude Code no longer asks you before a
  chat runs `purlis dispatch read` on one of its own tasks, and the same read is there as the
  `dispatch_read` tool for harnesses that prefer tools to commands. The `dispatch_report` tool
  now says that a turn ending without a report is asked for one, and that purlis then sends the
  task's last message in its place (#2084).

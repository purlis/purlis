### Fixed

- **A long-running task's last note survives an app restart.** After a restart,
  `purlis dispatch list` said "no note yet" for a long-running task until it sent another note.
  It now reads the task's last note from its dispatch record: when it was sent and, where the
  record kept them, its words (#3956).

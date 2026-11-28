### Fixed

- **Updates › Mine shows a chat's finished turn without waiting on another project's.** Each ask
  for finished turns waited while any other ask read the event log, and two projects open at
  once made each other read it again. Each log file is now read once and kept by chat, an ask
  waits only for a file it needs itself, and a project's kept part is let go of when it closes
  (#7902).

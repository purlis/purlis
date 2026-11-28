### Added

- **The slow log times the core's answers.** Each answer to a window command that takes 200 ms
  or longer is written to `slow.jsonl` with its command, its class, the time it waited for a
  thread and the time its work took. Once an hour, and as the app quits, a summary gives each
  command's median and 99th percentile across every answer, quick ones included. The diagnostic
  log also says when a git or session-host deadline expires, and when the main thread's watch
  ran late on a busy machine (FS-M1a, #7893).

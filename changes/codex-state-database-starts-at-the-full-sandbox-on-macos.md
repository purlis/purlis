### Fixed

- **Codex starts at the full sandbox on macOS.** Codex could not create or migrate its state
  database (`state_5.sqlite`) in its profile folder, because the sandbox refused the file locks
  SQLite takes. Codex may now lock its own database files there, and nothing else.

### Changed

- **Handback reports are only ever written by the app.** The folder in which a dispatched task's
  report waits for the chat that asked for it is now held as purlis's own for every harness.
  The chat's hook reads the reports, and the app takes them off disk. Each report carries a seal
  that only the app makes; a report without a valid seal is handed only as quoted text, never in
  purlis's own words.

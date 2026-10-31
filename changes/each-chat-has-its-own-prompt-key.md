### Changed

- **Each chat's prompt filter is keyed by a key of its own.** The filter a chat's hooks send to
  confirm a skill Use skill… typed, or a curation action's prompt, is now keyed by a random key
  purlis makes for that chat as it starts (`PURLIS_PROMPT_KEY`), on this machine, on a runner and
  for a chat taken back after the window restarts. A chat that was already running before the
  update gets its key at its next start (#3369).

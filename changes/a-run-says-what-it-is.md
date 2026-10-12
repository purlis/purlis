### Added

- **Each run in the event log says what it is.** A `run.started` now carries the run's persona,
  harness, profile, sandbox backend and conversation, with the device it runs on, and says
  `unknown` for whatever purlis does not know rather than leaving it out (ADR 0066, #868).
- **A resumed chat knows the chat it resumed.** A session record now also writes `handed-from`
  and `resumed-from`, and **Resume** starts the new chat as having resumed the chat the record
  is of, by its `chat-id` (ADR 0066, #871).

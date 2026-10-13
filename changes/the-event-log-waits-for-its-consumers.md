### Changed

- **The event log keeps what a store written from it has not taken yet.** A sealed segment of
  the event log older than 30 days is now deleted only once every consumer the host opened the
  log with has taken its last event, so an audit writer or a telemetry store that was down for
  longer than that finds everything it had not taken. There is a hard limit: a segment older
  than 90 days is deleted whoever has not taken it, the app's log says which consumer lost
  which events, and that consumer is told what it missed when it reads on. Nothing is a
  consumer yet, so today's log is kept exactly as before (#940).

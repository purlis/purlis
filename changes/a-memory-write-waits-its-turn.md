### Fixed

- **Several chats remembering at once are no longer refused as busy for nothing.** A write to a
  memory store waited for its lock by asking again every 20 ms, so a writer that let go and asked
  again at once could keep taking it, and the other was refused after 5 seconds. A writer now
  waits in turn and gets the lock when it is let go (#5559).

### Fixed

- **A chat waiting on its own background work shows as waiting, not as needing you.** When a
  Claude Code chat's turn ends while its helpers, monitors or background shells are still running,
  or while it has a wake-up scheduled, its row, tab menu and persona mark now say `waiting on 2
  monitors` or `waiting on a wake-up at 14:05`. It raises no hand and sends no notification. The
  chat that dispatched it reads it as `waiting on its own work`, never idle. A background shell
  holds the wait for an hour, and a recurring schedule holds it only when its next run is within
  the hour. After that, or as soon as the chat asks you something, it needs you again (#3878).

### Fixed

- **Quitting or restarting to update warns about a chat in a background turn.** A chat working in a
  background turn over its wait on you used to look idle to the quit warning and the update
  dialog, so neither said that quitting or restarting would interrupt it. Both now name it as
  mid-turn (#5782).
- **Start fresh warns about a chat waiting on its own work.** Its question now says which of the
  chat's background shells, monitors, helpers and wake-ups will end, as the quit warning does.
  It also warns about a chat in a background turn (#5782).

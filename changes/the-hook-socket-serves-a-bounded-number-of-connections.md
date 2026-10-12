### Security

- **One process can no longer tie up the hook channel by opening connection after
  connection.** The app served every connection to its hook socket on a new thread, with no
  limit. It now serves them from a bounded pool: at most 32 at once from one process, and 128
  from all processes together. A connection past either limit is answered at once and closed.
  The hook spools its line for the next drain, and a `purlis` command that asked something says
  the app is busy. Hooks from many chats firing at once are still all served (#991).

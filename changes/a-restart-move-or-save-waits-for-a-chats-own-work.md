### Fixed

- **A restart, a move or a save waits for a chat's own work.** A chat waiting on its own
  monitors or background shells is no longer at a pause between turns: the restart a sandbox
  Allow owes, a move between runners and a repo save wait until that work has woken it and its
  turn ends, and Restart chat on its tab says it restarts when the turn ends. Restart now still
  restarts it at once (#3887).

### Changed

- **A shell follows a chat's rules.** New shell takes the focused pane, and what that pane showed
  keeps running in the sidebar. Closing or replacing a shell's pane leaves it running, so a dev
  server survives. End… on a shell asks only Cancel or End. When a shell's program exits, its row
  goes and its pane is left empty (#4761).
- **A terminal editor opened from a file, and a forge login, open beside what you were reading.**
  Each opens as a split beside the focused pane, with the keyboard, and goes when its program
  exits, the keyboard back where it was. Its pane cannot be dragged, closed or swapped, has no
  sidebar row, and is never kept for the next launch (#4761).

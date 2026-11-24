### Fixed

- **A merge that waits for you no longer reads as a timeout.** When a chat's `gh pr merge` or
  `glab mr merge` was still waiting on the app when the guard's time ran out, the chat was told
  the machine was busy and to retry or split the command. It is now told the merge is held for
  your press of Merge in Asks, and to carry on. The Bash guard has five seconds in all (it had
  two), so a loaded machine meets the limit less often, and the Read, Edit and dispatch guards
  keep two. A real timeout no longer advises splitting a single command.
- **`purlis report` is never refused by the guard.** A chat can report the guard's own fault:
  `purlis report …` is let through without asking the app, and a line that only starts with it,
  such as `purlis report; gh pr merge 1`, goes through the whole guard.

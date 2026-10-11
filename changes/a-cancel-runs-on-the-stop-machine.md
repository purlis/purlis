### Changed

- **A task's cancel is bounded the way your Stop is.** When a chat cancels a task it dispatched,
  the task now gets the one short turn a stop gives: a line that starts no turn within a minute,
  or a turn still going after 90 seconds, ends it with a report written in its place, still as
  `cancelled`. A task showing you a prompt is still never cancelled under it: purlis waits for
  your answer. From the cancel until its program has ended, the task cannot dispatch or hand
  off; the tasks it started still can (#1459).

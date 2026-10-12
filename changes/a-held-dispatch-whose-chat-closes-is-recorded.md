### Fixed

- **A dispatch waiting on your answer is recorded when the chat that asked closes.** Before, it
  was dropped without a trace. Its dispatch record now ends failed, as a task that did not
  start, and says its chat closed before you answered. Past tasks lists it (#1464).

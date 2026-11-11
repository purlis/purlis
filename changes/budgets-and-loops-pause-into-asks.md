### Changed

- **A task that reaches its budget is paused, and you're asked.** When a task reaches
  **Minutes per task** or its session's **Tokens per session**, it is held still instead of being
  ended. It's listed in Asks, counted on the ✋, notified, and shown on its own tab, with
  **Continue**, **Raise the budget** and **Stop**. Stop asks it for its report and then ends it,
  as reaching the limit did before. Raise the budget opens the limits in Settings › Project ›
  Dispatch, and the task goes on once it's within the new limit.

### Added

- **A chat that seems to be looping is paused, and you're asked.** If the same command fails
  6 times in a row (twice the **Stuck? when a command fails** setting), or you refuse 6 of a
  chat's permission prompts in a row, the chat is held still and asks **It's fine, continue**
  or **Stop**.

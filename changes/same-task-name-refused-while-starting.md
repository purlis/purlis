### Fixed

- **Two dispatches under one task name at once start one task.** The refusal of a task name a
  chat already has open is decided under the lock that reserves the task's slot, so a task that
  was let through and is still starting counts as open (#3795).

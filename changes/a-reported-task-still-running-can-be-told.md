### Fixed

- **A task that reported while it still runs can be told and cancelled.** When purlis made a
  task's report from its last message, or the task reported `blocked`, its program kept running
  but `dispatch tell` and `dispatch cancel` were refused as if it had finished. Such a task is
  now listed as "reported, still running" in `dispatch list` and in the open-tasks roster, and
  `tell`, `cancel`, `note`, `ask` and `answer` all reach it. A cancel that gets no new report
  keeps the earlier one and ends the task's program. Once the program has ended, the "has
  finished" refusal applies as before (#3899).

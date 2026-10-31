### Fixed

- **A chat's open tasks at every turn include the ones a restart orphaned that only their
  dispatch records know.** The roster purlis adds to each turn named a task orphaned by a
  restart only where a launch had tried to start it again. A task with no chat open, coming back
  or waiting to start was named only when the task list was asked for. The roster now names it
  too, without a chat number, since the one it had may belong to another chat. purlis reads the
  dispatch records for this once after a launch and again only after one changes, not at every
  turn (#3932).

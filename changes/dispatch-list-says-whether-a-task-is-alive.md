### Added

- **`purlis dispatch list` says whether each task is alive.** Each task in the list, the
  `dispatch_list` tool and the open-task roster at each turn now says whether its program is
  running, when it last ran a command or sent a note, and whether it kept running through a
  purlis restart or was started again on its conversation. A task purlis can no longer reach
  after a restart is marked *orphaned by restart*. When a chat dispatches a second task under
  the name of an open one, the refusal says the open task is alive (#3900).

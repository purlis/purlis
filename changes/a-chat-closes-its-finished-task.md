### Added

- **A chat closes its own finished task.** `purlis dispatch close <chat>` closes a task the chat
  dispatched once it has reported: its finished row goes, and a task kept open after its report
  is ended. Its report stays in the Dispatches tab. A task still working, in a turn, or one you
  typed in is not closed this way (#1981).

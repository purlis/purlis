### Fixed

- **A permission ask's deadline counts from when its hook started.** The harness gives up on
  purlis's permission hook a minute after it starts it, and the ask's deadline used to count
  from when the app raised it. Time the hook spent before that, starting up or waiting to be
  heard, was not counted, so an answer could land after the harness had stopped listening. The
  hook now says how long it has run, and the deadline counts from its start; a hook with too
  little time left raises nothing, and the harness asks in its own pane (#1063).

### Changed

- A chat's `run.started` now says whether anyone is at the chat (`attendance`: `attended`, `unattended`, or `unknown` where purlis holds no fact, which a reader must treat as unattended), and a run that follows a lowered fact (a `/clear` or a sub-agent) says `unattended`. A chat's briefing says so in a line of its own on every harness, so a person and the event log can both see it.

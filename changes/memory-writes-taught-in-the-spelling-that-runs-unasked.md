### Fixed

- **What purlis tells a chat about memory no longer leads it to a permission prompt.** The
  skills, the briefing, the hooks' reminders, the first task and the docs now name the
  `memory_add` and `persona_remember` tools first. Where they give the command, it names the
  chat's own persona or workspace (`purlis persona remember <persona> …`,
  `purlis workspace remember -w <workspace> …`), and where the text knows that name it is
  written in. Those are the forms a Claude Code chat runs without asking. A check in CI now
  fails when a shipped text gives the command without the name (#3880).

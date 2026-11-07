### Changed

- **A chat starts faster on a project with many personas.** Freezing every persona's tools for
  the new chat now happens while its briefing is put together, not before it, and is still
  finished, and written to disk, before the chat's first turn. On the 100-persona test project a
  chat start went from 68 ms to 47 ms on a Mac (#5486).

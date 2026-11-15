### Changed

- **Allow for all and keyboard triage stay in the scope you picked.** Identical requests are one
  row only among the asks Chat, Workspace or Project lists, and Allow for all answers only those.
  ⌘⇧I lands on the first ask listed, and answering one moves to the next listed ask, round to the
  first, never into another scope or project (ADR 0121, amended 2026-11-12, #6346).

### Fixed

- **Answering an ask in triage moves the keyboard on wherever the ask stood.** Answering any ask
  but the first one used to leave the keyboard nowhere (#6346).

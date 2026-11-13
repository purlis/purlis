### Changed

- **Allow for all and keyboard triage act only on what Asks lists.** Identical requests are
  grouped only among the asks listed under the chip you picked. So under Chat, an identical ask
  from a chat outside it stays out of the group, and Allow for all never answers it. ⌘⇧I lands
  on the first listed ask and walks only the listed asks. It never goes on into another scope or
  another project, and Go to project is no longer a triage step (ADR 0121, amended 2026-11-12,
  #6346).

### Fixed

- **Answering the last ask in keyboard triage moves on to the first.** Before, the keyboard
  dropped to the page when the answered ask was the last one listed (#6346).

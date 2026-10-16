# ADR 0066, amended by #2250 (2026-10-16): a start put aside at once is no run

Decided by the implementer under delegation; the operator can veto.

[ADR 0067 as amended 2026-10-16](../0067/2026-10-16-a-project-s-claude-code-settings-are-screened-again.md)
starts a sandboxed Claude Code chat once more, at its start, when the first screen of its
project's settings after its program starts does not pass. That chat has one run:

- **Its `run.started` is held until the start goes on.** For a start that screens its project's
  settings, the host tells the event log who the chat is and which run it begins before the
  program runs, as for any start, and the log writes the `run.started` once the screen passes.
- **A line of a held run that comes first is written after its `run.started`**, so a run's first
  event is still its `run.started`.
- **A start put aside writes no `run.started`.** The start in its place begins its own run, and
  that run is the chat's.
- **This machine's sandbox count counts the start that goes on**, once, after the screen.

The text above is left as it was written, and this section is the amendment.

## Decisions

- **D-2250-1: held, not taken back.** The event log is append-only, so a `run.started` once
  written cannot be withdrawn, and the local count has no decrement. Holding both until the
  screen keeps each written once.
- **D-2250-2: the run is told to the log before the program runs, and its line written after the
  screen.** A hook line from the program then never mints a second run, and still lands after
  the `run.started` of the run it belongs to.

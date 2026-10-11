### Fixed

- **A helper's permission prompt stays on its chat until that helper moves on.** When a
  background helper asked and the main agent kept working, the main agent's own tools coming
  back took the chat out of "waiting on you" while the helper's prompt was still open. Now only
  the chat's own prompt is answered by its own tools; a helper's prompt goes when that helper's
  own tools show it was answered in the pane, when it is answered in the window, when that
  helper ends, or when the turn ends (#1644).

- **Only the chat's own harness run can say its prompt was answered in the pane.** A tool line
  from another harness started inside the chat, or from a job it left running, no longer takes
  the chat (or a helper of it) out of "waiting on you" (#1601).

### Fixed

- **A helper's permission prompt stays on its chat until that helper moves on.** When a
  background helper asked and the main agent kept working, the main agent's own tools coming
  back took the chat out of "waiting on you" while the helper's prompt was still open. Now only
  the chat's own prompt is answered by its own tools; a helper's prompt goes when it is answered
  in the window, when that helper ends, or when the turn ends (#1644).

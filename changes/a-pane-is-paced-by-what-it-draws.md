### Changed

- **A chat's pane that cannot keep up with a flood of output is sent a fresh screen, not a
  backlog.** The session host now counts a pane's output as delivered only once the pane has
  drawn it, so typing into a chat that is printing a lot stays quick: a pane that falls behind
  starts over from the chat's screen as it is now, rather than working through everything it
  missed (#4733).

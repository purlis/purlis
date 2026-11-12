### Changed

- **Asks opens on the chat you are in.** Its scope chips are Chat, Workspace and Project, in
  place of This workspace, This project and All projects. Chat lists the focused chat and every
  chat it dispatched, however deep. Workspace is left out where the project has one workspace.
  Each chip shows its count above 0. Asks opens on Chat every time, and a pick holds until Asks
  closes. With no chat focused, Chat is greyed and Asks lists the next scope. An empty scope
  offers the nearest wider one with something waiting. Another project's asks are no longer
  listed here: you answer them in that project's own Asks (ADR 0121, amended 2026-11-12, #6340).

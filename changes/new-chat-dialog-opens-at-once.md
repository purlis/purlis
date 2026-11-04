### Changed

- **The new-chat dialog opens at once.** It shows the personas and the profiles straight away,
  and each profile's billing, approval and sandbox fill in as they are checked, in room kept for
  them, so nothing moves. A chat started before then waits for them. If it turns out to need an
  approval, or the sandbox cannot hold it, nothing starts: the keyboard goes to Cancel, the
  footer says why, and the button holds still for a moment so a second click grants nothing
  (#4736).

### Fixed

- **The Inbox and the Notices update their ages with the rest of the window.** Each one kept a
  timer of its own and redrew once a minute on it, separately from everything else and even
  while the window was hidden. Now an age like "5m" changes in the same redraw as every chat
  row's time, and not at all while the window is hidden. It is right again the moment the
  window comes back.

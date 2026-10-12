### Changed

- **A Smart close you reached without a pass is never silent.** If a chat runs Smart close without
  a typed `/smart-close` behind it and writes no session record within five minutes, the window
  now says so, as it does for a Smart close you start from the tab (#1361).

### Security

- **Each chat has a cap on the session records it asks purlis to write.** A chat may ask for a
  few records in ten minutes and a bounded number while it is open. Past the cap nothing is
  written and the chat is told why. Notices from a Smart close without a pass have their own
  per-chat cap, past which they go to the log only (#1361).

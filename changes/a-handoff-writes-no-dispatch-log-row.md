### Changed

- **A handoff no longer adds a row to the committed dispatch log.** The row said only when a
  handoff happened, whether it stayed in the workspace and whether it made one; the handoff's
  own record, on the machine that made it, already says more and is what `purlis persona
  stats` counts. A handoff now leaves no trace another machine can see, and `persona stats`
  no longer prints a total of the rows older builds committed (#1457).

### Added

- **Settings › Project › Dispatch › Keep task screens turns keeping a task's last screen on or
  off**, on this machine only. Turning it off asks first, then deletes every kept screen and any
  screen a cut-off write left behind. If a screen cannot be deleted, the row says how many stayed
  and counts them (#6306).

### Changed

- **A task that had finished when purlis quit keeps its last screen**, so *Read it* on its card
  works after the next launch. The quit waits at most 3 seconds for these screens, across every
  open project. A screen that has not come in by then is not kept (#6305).
- **A task's number beside its path in the pane head is drawn at the path's size**, not a step
  larger (#6297).

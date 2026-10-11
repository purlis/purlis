### Fixed

- **A dismissed "project gone" line stays dismissed for the whole run.** Dismissing the line
  about a remembered project that has moved or gone used to last only as long as the place it
  was dismissed in: the opener drawn again after the last project closed showed it once more.
  The window and the opener now share one list, and a dismissal lasts until purlis's own list
  of remembered projects no longer says the project is gone (it is back, located or forgotten).
  It is not yet kept past a relaunch (#1291).

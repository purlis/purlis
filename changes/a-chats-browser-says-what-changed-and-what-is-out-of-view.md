### Added

- **A chat's browser says what an action changed, and what is out of view.** Every browser tool
  that acts on a page takes `changes`, and then answers the lines of the page new or gone since
  it was last read, so the agent need not take a whole snapshot after each step; and a snapshot
  or a find marks an element the page lays out beyond what it shows `(out of view)` (#2137).

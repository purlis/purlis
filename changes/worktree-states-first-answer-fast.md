### Fixed

- **A repo with over a hundred branch folders shows each folder's changes as it is read, not after
  all of them.** The explorer's branch list waited for every folder's `git status` (47 s for 133 folders)
  before drawing any state. It now fills in each folder as its status finishes, keeps the answer
  until something in that folder or its git folder moves, and reads only the folders that moved
  when asked again. A folder is never shown clean when it may have changes: a lost watch, an
  event that could not be placed or a flush that times out reads it again (H16).

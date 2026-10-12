### Fixed

- **A folder the system would not watch is named as that folder, not as its branch.** When the
  system refused to watch every open folder of a branch, the explorer said the whole branch was
  waiting to be read. purlis now says which folders the system refused, and the explorer says
  "the system would not watch it" on them, whatever else of the branch is open (#1160).

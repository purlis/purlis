### Changed

- **A merge is made by the method its ask showed.** A merge waiting in Asks now remembers whether
  the repo squashes or makes a merge commit, as the forge said when the ask was read. If the repo's
  method changes before you press Merge, nothing is merged: the ask shows the new method and asks
  once more (ADR 0128 D8.1).

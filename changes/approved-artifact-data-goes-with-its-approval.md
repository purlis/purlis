### Fixed

- **An artifact tab no longer loads its page twice when a change to its folder arrives late.**
  A change that leaves the template the page is already served as it was keeps the page as it
  is, where it was scrolled, with no flicker (#6697).
- **Unpinning an artifact in the window on macOS 14 or later removes what its approved page
  kept**: its local storage, IndexedDB, cookies and cache go with its approval. `purlis artifact
  unpin` leaves them on disk, unreachable, as before. An approved page starts with empty storage
  once after this update (#6697).

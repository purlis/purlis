### Added

- **Bring back a deleted skill whose name purlis holds now, under a new name.** On Deleted, one
  skill or a group brought back together offers each such skill a new name, filled in as an
  install fills one when a name is taken (`<source>-<name>`), and brings it back once you
  confirm it; in a group the rest come back as named, in the same change (#5457).

### Fixed

- **A skill brought back under a new name leaves Deleted.** One whose delete was a commit stayed
  listed after it came back under another name (#5457).
- **The keyboard reaches a taken name's choice.** In an install's review, *Skip it* and *Install
  it under a new name* were out of the Tab order until one was clicked (#5457).

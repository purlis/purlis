### Fixed

- **A grade purlis stopped under is recorded as interrupted even when another program now has
  its process number** (Skill studio preview, #3466). A running grade holds a lock on its own
  record of being under way, and the next start reads that lock, not the process number, to tell
  whether it still runs. This also makes the check work where process numbers can't be asked.

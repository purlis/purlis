### Fixed

- **The save status in the title bar reads only what changed, on a big project too.** git's
  untracked cache was never used for it, and a project purlis had not saved yet had no cache and
  no file monitor token, so each look read the whole tree. On a project with 200,000 files a look
  took 4 to 25 s; it now takes about a third less where git has no file monitor, and less again
  where it does (#5486).

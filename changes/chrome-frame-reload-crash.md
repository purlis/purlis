### Fixed

- **Chrome no longer ends itself on a page whose frame reloads quickly.** purlis no longer holds
  a frame at its start, and it reads the address of a frame from when the frame attaches. Opening
  a page Chrome is slow to report now says the page stayed blank, instead of "no page open"
  (#4472).

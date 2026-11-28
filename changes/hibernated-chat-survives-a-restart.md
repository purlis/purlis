### Added

- **A hibernated chat stays hibernated when purlis restarts.** A chat you hibernated comes back
  hibernated after a quit, a restart or a crash, whether or not it was on screen, with the copy
  of its screen and Resume. purlis does not start its program again until you resume it. The
  chats that were working still come back as they did, and a chat whose background work the
  restart ended is still told what ended. A chat that was resting when its session host ended
  now comes back hibernated too, with Resume and no copy of its screen, rather than being
  started again by itself (SC-4e, #7634, #1391).

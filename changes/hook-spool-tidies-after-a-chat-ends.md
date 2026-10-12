### Fixed

- **An ended chat's hook spool no longer stays behind.** Its empty folder in
  `.charter/app/spool/` and the emptied file an older build kept for it are removed once the
  chat ends. A project opened by a host that has no event log drops the spool keys of chats
  that have ended, unless the chat's spool still holds a line, so those keys no longer pile
  up from one launch to the next (#983).

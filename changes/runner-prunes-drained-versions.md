### Added

- **A machine purlis installed itself on over SSH no longer keeps every old version.** Once an
  old version's chats have all ended and its host has stopped, the next install removes its
  files there. The version running and the one before it always stay, and a version with a chat
  still running is never removed (#6688).

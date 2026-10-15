### Fixed

- **Opening or closing a chat with many chats open no longer recompiles every chat's sandbox**
  (#2157). The check for chats running on an older sandbox, which runs on each open and close,
  compiles chats started alike once, so with fifty chats open it does one compile where it
  did fifty.

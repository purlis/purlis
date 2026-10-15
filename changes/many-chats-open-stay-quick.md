### Fixed

- **Opening or closing a chat with many chats open no longer recompiles every chat's sandbox**
  (#2157). The check for chats running on an older sandbox, which runs on each open and close,
  compiles once per distinct kind of start, so with fifty chats started alike it does one
  compile where it did fifty.

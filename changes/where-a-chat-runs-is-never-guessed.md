### Changed

- **Where a chat runs is never guessed.** While purlis can't read its machine store
  (`machine.json` in its config folder), a chat started in a workspace is refused in words that
  name the file and how to fix it, rather than started on this machine. A store that reads but
  doesn't parse is kept beside it as `machine.json.unread` before it is replaced, and a project
  whose chats run on a pool is no longer forgotten when you open more than 64 others.

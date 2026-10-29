### Changed

- **Chat settings live in the app's data folder.** A Claude Code chat whose settings are too long
  for its command line now gets them from a file in purlis's data folder, outside the project,
  instead of in the project's state folder. Older files in the project are cleaned up and never
  read.
- **The project's state folder is held whole** for Codex and opencode chats on macOS: a chat
  never renames, moves or removes it.

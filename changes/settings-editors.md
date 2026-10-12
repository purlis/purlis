### Changed

- **Settings edits a whole file in a real editor.** Edit as TOML and Edit as JSON now show the
  file in the same editor as the file viewer, with line numbers, colours and find (⌘F or
  Ctrl+F). If the file changes on disk while you edit it, the file as it is now opens beside
  your edit. Take what you want from it, then keep your edit or discard it. Neither is lost
  until you choose (#1842).
- **List fields are the row's width and grow with what you type.** A list such as the
  environment passed to chats, or a forge's repos never listed, sits under its name and shows
  every line (#1842).
- **Dispatch says once where its limits are kept**, in the group's header, not under each
  table.

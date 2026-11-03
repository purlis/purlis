### Changed

- **A chat touching files no longer redraws the whole Explorer.** Each file a chat reads or
  edits redraws only that file's mark and the marks of the folders above it, so an agent working
  fast keeps the window responsive. A project that is not in front does no work for its chats'
  touches; its marks start again when you bring it back. A file tab's tree is not redrawn for a
  chat working elsewhere. A screen reader now hears which chat is working on a file after the
  file's name, as its description, rather than as part of the name (#4730).

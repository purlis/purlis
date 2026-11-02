### Changed

- **A chat touching files no longer redraws the whole Explorer.** Each file a chat reads or
  edits redraws only that file's row and the folders above it, so an agent working fast keeps
  the window responsive. A project that is not in front does no work for its chats' touches;
  its marks start again when you bring it back (#4730).

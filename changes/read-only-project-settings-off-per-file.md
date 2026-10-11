### Changed

- **On a read-only project, Settings turns off only what writes a committed file.** Settings ›
  Project decides row by row from the file each row writes: what is kept in `purlis.local.toml`
  (`charter.local.toml`) alone, such as the default profile, the environment passed to chats,
  your profiles and your hosts, and Edit as TOML on that file, stays on. A row that may move to
  the committed file stays off, and the links out of Settings stay on (#1759).

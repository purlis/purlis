### Fixed

- **A chat taken back after its project's sandbox setting changed is judged by how it was
  started.** Browser logins treat it as sandboxed or not by the session host's record of how its
  run started, and a chat with no such record is treated as sandboxed and offered a restart
  (#2636).

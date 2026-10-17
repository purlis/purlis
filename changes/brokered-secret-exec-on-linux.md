### Added

- **`purlis secret exec` from a sandboxed chat runs on Linux.** A persona's declared tool now
  runs through the app on Linux too: in purlis's own sandbox, held by the chat's own profile,
  with the credential handed only to that command, as on macOS. Before, the app refused it
  there (#2342).

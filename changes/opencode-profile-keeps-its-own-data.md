### Fixed

- **A sandboxed opencode chat on a profile purlis made keeps its data in that profile.** Its
  sessions and logs are written, and its settings and sign-in read, in the profile's own folders,
  inside purlis's sandbox, and the chat stays out of opencode's default folder and every other
  profile.

### Changed

- **opencode's own sign-in on a runner is carried to the runner that replaces it,** as Claude
  Code's and Codex's are, when the runner it came from kept its opencode chats out of that
  folder. Before, opencode signed in again on the new runner.

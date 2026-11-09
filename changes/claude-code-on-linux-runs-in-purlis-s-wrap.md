### Changed

- **purlis runs Claude Code chats on Linux inside its own sandbox, on a profile for sandboxed chats
  only.** Such a chat runs whole inside purlis's Linux wrap, as Codex and opencode chats do and as
  Claude Code chats on a runner already did, and needs no bubblewrap or socat. Its project lists its
  model's hosts at the full sandbox (`model-providers`). A Claude Code chat on a profile for every
  chat keeps Claude Code's own sandbox.

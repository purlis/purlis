### Changed

- **purlis runs every sandboxed Claude Code chat on Linux inside its own sandbox, on a profile for
  sandboxed chats only.** Such a chat runs whole inside purlis's Linux wrap, as Codex and opencode
  chats do and as Claude Code chats on a runner already did, and needs no bubblewrap or socat. Its
  project lists its model's hosts at the full sandbox (`model-providers`). Where you have no such
  profile, the Inbox's *Claude Code needs a sandbox-only profile here* makes one and opens Claude
  Code's sign-in for it; then start the chat again (ADR 0067, amended 2026-11-10).

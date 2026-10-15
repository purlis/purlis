### Security

- **A profile's sign-in is kept from its chats' commands where its harness allows it.** A
  sandboxed Claude Code chat's commands and its Read and Edit tools are denied the sign-in file in
  its profile's folder. Where a harness keeps its sign-in somewhere its commands can still reach
  (Codex and opencode, which run with their commands, and Claude Code on macOS), Settings › You ›
  Profiles says plainly that the commands its chats run can read it (#2037).

### Changed

- **A sandboxed Codex chat runs on a profile for sandboxed chats only.** A sandboxed Codex chat
  with none waits, and the Inbox's *Codex needs a sandbox-only profile here* makes one and opens
  Codex's own sign-in for it. You can also make one in Settings › You › Profiles with *For
  sandboxed chats only* on, or with `purlis profile create --harness codex --sandbox-only`.
  purlis no longer copies your Codex sign-in for a sandboxed chat, and no sandboxed chat writes
  your own Codex home. A project's earlier sandboxed Codex conversations are brought to that
  profile the first time it runs there (#2037).

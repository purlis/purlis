### Security

- **A Claude Code chat's commands cannot read its profile's sign-in file.** A sandboxed Claude
  Code chat is denied the sign-in file in its profile's config folder, to the commands it runs
  and to its Read and Edit tools. Settings › You › Profiles says plainly on each profile whose
  sign-in the commands its chats run can read (#2037).

### Changed

- **A sandboxed Codex chat runs on a profile for sandboxed chats only.** Make one in Settings ›
  You › Profiles with *For sandboxed chats only* switched on, or `purlis profile create
  --harness codex --sandbox-only`, and sign it in through Codex's own sign-in. purlis no longer
  copies your Codex sign-in for a sandboxed chat, your own Codex home is never written by one,
  and a sandboxed Codex chat on another profile says how to make one. A project's earlier
  sandboxed Codex conversations are brought to that profile the first time it runs there (#2037).

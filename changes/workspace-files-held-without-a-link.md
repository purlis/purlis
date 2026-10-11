### Security

- **A link a chat leaves at a workspace's other files no longer carries your writes anywhere
  else.** `workspace.md` (its vision, its sections and its `## Sessions` line),
  `workspace.json`, the piece log and liveness records in `pieces/`, the work link log in `work/`
  and the project's own `sessions/` are now written the way the four stores are: with no link
  anywhere on the way, and through the folder purlis opened. A link at one of them, or at the
  folder above it, is refused and nothing is written, even when it points at another workspace or
  a persona inside the project (#1082). The piece and work link logs are read the same way.

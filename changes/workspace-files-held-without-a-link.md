### Security

- **A link a chat leaves at a workspace's other files no longer carries your writes anywhere
  else.** `workspace.md` (its vision, its sections and its `## Sessions` line),
  `workspace.json`, the piece log and liveness records in `pieces/`, the work link log in `work/`
  and the project's own `sessions/` are now written the way the four stores are: with no link
  anywhere on the way, and through the folder purlis opened. A link at one of them, or at the
  folder above it, is refused and nothing is written, even when it points at another workspace or
  a persona inside the project (#1082). The piece and work link logs are read the same way.
- **The last readers that opened a store's entries by path read them through the store.** The
  briefing's persona memory digest and the change store's record check, landing log and pending
  landings never read another workspace's or a persona's files through a store swapped for a
  link (#1083).

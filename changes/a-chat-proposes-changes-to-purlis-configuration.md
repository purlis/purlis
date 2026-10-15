### Added

- **A chat can propose a change to purlis's own configuration, and you apply it.** When a
  sandboxed chat edits the project's manifest, a persona's `mcp.json` or `persona.md`, or a
  skill's files, the change is shown on its tab and in the Inbox, the file as it is and as the
  change would make it, with **Apply** and **Dismiss**. Apply writes exactly that, as you, only
  while the file is still what the chat read; it rides your next save, and is recorded. The chat
  is told applied, dismissed or stale on its next turn. Claude Code chats propose with their own
  edits; other harnesses run `purlis propose <file> --from <draft>` (#1975).

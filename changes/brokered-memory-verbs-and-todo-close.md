### Changed

- **Tidying memory and closing todos work from a sandboxed chat.** In a chat purlis started,
  `purlis persona forget`, `edit-memory`, `archive-memory`, `unarchive-memory`, `move-memory`
  and `optimize --apply`, and `purlis workspace forget`, `edit`, `archive`, `unarchive`,
  `move-memory`, `optimize --apply` and `todo done|forget`, hand the change to purlis over the
  chat's own connection and print what they always printed. purlis makes it only in the chat's
  own workspace, the persona it runs as and shared memory, credits it to the chat and counts it
  toward the chat's 60 writes a minute. Recording a todo from such a chat now also finishes the
  closes an interrupted `todo promote` left behind, as it does in a terminal (#1408).
- **A sandboxed chat is told, not left to fail, where purlis does not write for it.**
  `purlis persona remember --ephemeral` and `persona forget --ephemeral` (scratch in purlis's
  state folder), a memory command naming a persona other than the one the chat runs as, and a
  curation or move that reaches another workspace are refused with a sentence that names the
  way round (#1408).

### Fixed

- **A memory too long to hand over is refused, never written twice over.** In a chat purlis
  started, a memory or a memory command's text past what one hand-over carries (12 KiB) is
  refused with a sentence saying so, where it used to break the connection and be written by
  the chat itself (#1408).

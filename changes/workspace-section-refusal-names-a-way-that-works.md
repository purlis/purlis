### Fixed

- **`workspace_section`'s refusal also offers to ask you.** When the app cannot take a section
  entry for a chat, the tool says to edit `workspace.md` to add it, or to ask you to, since a
  chat in a clone cannot edit that file. The harness guide no longer tells Codex chats to run
  `purlis workspace vision`, `purlis persona remember` or `purlis persona where` instead of the
  tools: they reach the app like every other harness's (#1598).

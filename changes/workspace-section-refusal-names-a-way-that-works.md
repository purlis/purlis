### Fixed

- **`workspace_section`'s refusal no longer points only at an edit the sandbox may refuse.** When
  the app cannot take a section entry for a chat, the tool says to add it to `workspace.md` where
  the chat's sandbox lets it, or to ask you to, since a chat in a clone is refused that file. The
  harness guide no longer tells Codex chats to run `purlis workspace vision` instead of the tools:
  they reach the app like every other harness's (#1598).

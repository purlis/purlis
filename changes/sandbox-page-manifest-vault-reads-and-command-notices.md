### Changed

- **Settings › Sandbox says when the project's manifest is gone.** A project whose manifest is
  missing or cannot be read refuses every chat, and the page now says so in place of "Off in this
  project", with no Turn the sandbox on, which would write a new manifest in its place (#1411).
- **A vault tab put back at launch asks the core whether opening it reads a secret.** The vault
  list says so for each vault, so a provider the window does not know waits for a press instead of
  being read (#1660).
- **A refused write that a purlis command does for a chat names that command.** A commit in a
  branch folder points to `purlis worktree commit`, and a direct write of a session record, a
  memory, a todo or `workspace.md` points to the command that writes it. These Notices offer no
  grant (#1476, #1408).

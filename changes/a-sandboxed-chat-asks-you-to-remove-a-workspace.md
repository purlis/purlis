### Changed

- **A sandboxed chat asks you to remove a workspace.** `purlis workspace remove` run in a chat
  the app started under the sandbox now refuses before anything is deleted, and says to remove
  the workspace from the window. A chat without the sandbox, and a terminal, remove it as before
  (#1408).

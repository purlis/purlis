### Changed

- **The ask to open a file outside the workspace names the chat it came from.** Under its
  heading it reads "From chat #641 · Fix the login form", so you can tell which chat printed the
  path. A shell's pane names no chat (#6355).
- **A file a branch doesn't offer is no longer a link.** A path to a file git ignores in a clone
  or a branch folder, such as `.env` or `target/out.log`, stays plain text, as the editor
  would not open it anyway. Its hover says purlis will not open it in its editor (#6357).

### Fixed

- **A file at the project's root opens at its line and column.** ⌘-click on `README.md:3:5` in a
  pane opens the file's tab with the cursor there, and opening the link again brings it back
  there (#6356).

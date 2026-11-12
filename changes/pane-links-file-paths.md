### Added

- **File paths in a pane are links.** A path a chat prints, such as `src/main.rs:42:7`,
  `~/notes.md`, `/etc/hosts` or a `file:///` address, links once purlis finds the file. A relative
  path starts from the folder the chat works in. ⌘-click (Ctrl-click off a Mac) opens the file in
  an editor tab at its line and column. purlis never hands it to another app, so a click never
  runs anything. A file outside the chat's workspace asks first (#6216).

### Changed

- **⌘T opens an empty tab and ⌘N starts a new chat** (Ctrl+T and Ctrl+N off a Mac, where a
  chat's terminal keeps them for its shell). ⌘⇧T is still New shell. Until now neither key did
  anything in purlis, and `+` was the only way to an empty tab (#5292).
- **An empty pane offers New shell beside New chat**, and says you can drag a chat in from the
  sidebar. A tab with nothing in it is called Empty tab until it holds something (#5292).

### Added

- **The strip's `+` has a list.** Pressing `+` still opens an empty tab. Rest the pointer on it, or
  press Down on it, to choose what else opens in a new tab: New chat, New shell, a browser,
  Settings and the other views, each with its key. A long list has a filter (#5292).

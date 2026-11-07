### Changed

- **A chat with no workspace yet is told only the ten most recently used.** Where a project has
  more than ten workspaces, the line asking a new chat to confirm one names the ten used most
  recently, then "…and N more: `purlis workspace list`", in place of every name, which on a
  project of a thousand workspaces was about 16 KB in every chat (#5486).

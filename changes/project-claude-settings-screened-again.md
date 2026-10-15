### Changed

- **A project's Claude Code settings are screened again when they change.** While a sandboxed
  Claude Code chat that loads its project's own settings is open, purlis watches those files.
  When they change and no longer hold only what a sandboxed chat keeps, the chat restarts on its
  conversation at once, on your own Claude Code settings and purlis's, and its tab says which keys
  were not applied. A change that still passes changes nothing. Where the files cannot be
  watched, the chat starts on your own settings and purlis's from the first.

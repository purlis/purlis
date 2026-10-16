### Changed

- **A project's Claude Code settings are screened again when they change.** While a sandboxed
  Claude Code chat that loads its project's own settings is open, purlis watches those files.
  When they change and no longer hold only what a sandboxed chat keeps, purlis restarts the chat
  on its conversation at once, even with no window open, on your own Claude Code settings and
  purlis's. Its tab says which keys were not applied. A chat that cannot be started again is
  ended, and its tab says why. A change that still passes changes nothing. Where the files cannot be
  watched, the chat starts on your own settings and purlis's from the first.

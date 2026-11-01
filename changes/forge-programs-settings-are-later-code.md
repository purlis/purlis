### Changed

- **`gh` and `glab` settings and extensions join the names a chat does not write.** `~/.config/gh`,
  `~/.config/glab-cli` and `~/.local/share/gh` are held as git's own config is, at every sandbox
  level, the floor among them. Chats on your own machine still read them, so a chat's `gh` keeps
  working; run `gh auth login` or `gh extension install` in your own terminal.

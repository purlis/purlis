### Changed

- **A grade's chats run with only purlis's plugin on** (#3468). Every other plugin Claude Code
  has installed is turned off for those chats alone, so a grade measures the skill and not your
  plugins, and the grade's record names the plugins that were off. Your own plugin settings are
  not changed. A grade on an opencode profile is refused with the reason: opencode cannot turn
  its plugins off for one chat.

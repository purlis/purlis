### Changed

- **A chat on a runner starts only once its profile's harness settings file can be read.** When
  Claude Code's `settings.json`, Codex's `config.toml` or opencode's `opencode.json` is a link, a
  file with a second name on the disk, or can't be opened, the chat is not started there and its
  tab says which file and what to do: keep the file itself in the settings folder, under that one
  name. A link elsewhere in the settings, such as an agent's file, is still left out and said.

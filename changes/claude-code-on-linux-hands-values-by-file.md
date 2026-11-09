### Changed

- **A Claude Code chat on Linux under Claude Code's own sandbox hands a vault's value to a command
  by file.** `purlis secret exec --env` is refused there with a sentence naming `--file`, which the command
  reads the value from instead. Declare a persona's tool `as = "NAME file"` for such a chat.

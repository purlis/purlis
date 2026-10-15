### Fixed

- **Printing a file that quotes a refusal no longer raises a Block.** `sed -n`, `cat`, `grep` or
  `git show` over a test or a log that quotes a socket, host or folder refusal brought up a
  Notice and told the chat to wait, although nothing was refused. A line that only reads or
  prints now raises nothing from what it printed, and neither does a background command's
  output read back. A failed command's own error, the sandbox's own report and purlis's proxy
  still raise a Block, in Claude Code, Codex and opencode chats alike (#1972).

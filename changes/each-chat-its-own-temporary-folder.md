### Fixed

- **Each sandboxed Claude Code chat has a temporary folder of its own.** Every Claude Code chat
  on a machine used to write its temporary files, its scratchpad and its commands' `$TMPDIR` in
  the one shared `/tmp/claude-<uid>`, so a file one chat wrote could replace another chat's file
  of the same name. Each chat now gets a folder of its own, which its sandbox lets it write and
  no other chat may, and the folder is removed when the chat ends. Codex and opencode chats
  already had one.

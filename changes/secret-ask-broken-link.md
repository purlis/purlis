### Added

- **A chat can ask you to replace a value that doesn't work.** When a key it uses is refused (a
  401, an expired token), the chat asks again with its reason (`purlis secret ask <KEY>
  --replace "<why>"`, or the `secret_ask` tool), and the window shows its words as the chat's.
  Type a new value, or pick an entry you keep: for a Linked entry the ask names the entry it
  reads, a new value goes there, and a pick re-points the link (#5075).
- **A Linked entry whose entry is gone asks you, instead of failing.** When a chat's command, a
  browser login or a profile reads a key that links to an entry you have since deleted or
  renamed, purlis asks you in the window, naming the entry that is gone. Give the key a value,
  or pick another entry to link it to, and the command carries on. The chat is never told which
  entry the link read (#5075).

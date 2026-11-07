### Changed

- **A chat can ask for a key and its program's Allow in one notice.** `secret_ask` and
  `purlis secret ask` take `--program`: for curl you can fill the key and Allow for this chat
  together; for any other program the run asks on the exact command. Chats are now told to prefer
  simply running the command, and a "filled" answer says whether the program was allowed (ADR
  0112, #5244).

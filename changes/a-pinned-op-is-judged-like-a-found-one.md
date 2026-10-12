### Security

- **A pinned provider program is checked like a found one at every read.** The `op` a
  keyring-held token was pinned to is held to the same checks as an `op` purlis looks up, and a
  refusal says where purlis looks for `op` rather than asking for purlis to be started from a
  shell (#1517).

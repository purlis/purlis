### Changed

- **Asks are listed by facts.** An ask that ends by itself (a permission prompt, a held
  connection, a run grant, a secret, a login, a fill or a command's confirmation) comes first
  with its countdown, then one that other chats wait on ("Blocks 2 chats"), then the longest
  wait. One chat's asks stay together, each row names its project and workspace and says in a
  line why it stands there, and a new ask takes its place without moving to the top (ADR 0121,
  #6006).

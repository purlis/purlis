### Fixed

- **A restart of many chats no longer floods the window with sandbox checks, and the doctor's
  preflight no longer slows with every branch folder.** The window's question of which chats
  run on an older sandbox, and of why each runs at its level, is read once per burst: an ask
  made while one is being read waits for the next read, which every ask that came meanwhile
  shares, so ten asks in a row cost two reads and no answer is older than its ask. One read
  walks the project's folders once for all the chats it compiles, where it walked them again
  for each. The doctor's workspace layer row reads a repository's branch folders once, where
  each of its N folders read all N (felt speed round 4, H10).

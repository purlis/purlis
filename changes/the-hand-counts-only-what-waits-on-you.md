### Fixed

- **The ✋ no longer counts a chat that isn't waiting on you.** A dispatched task's turn end waits
  on the chat that dispatched it, so it is no longer listed in Asks as "Waiting on your reply",
  unless the task asks you directly (a permission, a secret, a sandbox block) or that chat has
  closed. A chat whose turn ended while its own background command or helper still runs is no
  longer listed as waiting on your reply because an update about it was already in the Inbox.

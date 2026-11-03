### Changed

- **A chat can't keep asking to move its work somewhere you refused.** When you refuse a chat's
  dispatch that would run somewhere other than where its workspace runs its chats, that chat's
  later dispatches to the same place are refused without asking you again, for as long as it
  runs, and the chat is told why. Other places and other chats still ask (#4862).

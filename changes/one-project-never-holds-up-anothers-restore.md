### Fixed

- **A project whose chats hang as they start no longer holds up every other project's chats at a
  launch.** One project's chats take all the places a launch starts chats in but one, so the
  other projects' chats come back at once instead of one project at a time, minutes apart. A
  launch now starts three or four chats at once, by the machine's cores (#4986).

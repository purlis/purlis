### Fixed

- **Quitting while a launch is still putting chats back keeps every chat** (#3114). A chat whose
  start finished just as purlis quit could drop out of the record of open chats, and the next
  launch did not put it back. It is now kept as a chat still to start, in its place on the strip
  and in front if it was.

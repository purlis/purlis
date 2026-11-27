### Fixed

- After a crash where the record of open chats could not be read, a task that had already
  reported keeps its last screen. purlis keeps it only where it can prove the program left
  running is that task's chat, so it never keeps the screen of one of your own chats (#6737).

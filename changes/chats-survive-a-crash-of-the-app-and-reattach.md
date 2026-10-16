### Added

- **Chats survive a crash of the app.** With chats running in the session host
  (`PURLIS_CHATS_IN_THE_HOST=1`), a crash or a kill of the app no longer ends them: the session
  host keeps every chat, its network and its project's saving for ten minutes. Start purlis
  again within them and every tab comes back on the same chat, with what it wrote meanwhile on
  screen and each chat's state and Needs you as the host kept them. After ten minutes the host
  ends them as a quit would, and the next start offers to reopen them. Quitting still ends every
  chat, and so does an update (#2006, #646, #2194).

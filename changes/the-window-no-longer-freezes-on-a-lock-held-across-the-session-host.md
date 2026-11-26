### Fixed

- **The window no longer freezes for up to 30 s when many chats are busy.** Moving, resizing or
  focusing a window used to wait, on the thread that draws every window, for a lock that
  another thread held while it waited on the session host. That work now happens on a thread of
  its own, and a drag's frames behind a slow one are folded into one look. When the window
  still stalls, the slow log (`slow.jsonl`) now names what held it: the window event, menu click,
  page load or command, and the source line of the lock it waited on.

### Changed

- **The session host keeps the event log and each chat's end.** With chats in the session host
  (the default, #2004), it is the event log's one writer, and a chat that ends while the window
  is gone still has its end recorded. If that chat was a task, its dispatch
  is settled as one that went without its report, and the chat that asked is handed that when the
  window is back (#2003).

### Fixed

- **A hook line the host took late is recorded once.** With chats in the session host, a line a
  chat's hook also spooled because the host was slow to take it is no longer recorded twice in the
  event log (#2003).

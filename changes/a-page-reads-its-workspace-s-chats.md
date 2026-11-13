### Added

- **A pinned page can draw its workspace's chats as a tree.** The new `purlis.chats` query lists
  the workspace's open chats, each with its persona, its state as the window names it, when it
  started and last moved, and the chat it was dispatched or handed off from. With
  `history: true`, it also lists the workspace's ended tasks and handoffs. A page can subscribe
  to it and stays current as the chats move. It answers names, states and times only, never
  what a chat wrote. Its chat tool, `chats`, asks you before a chat lists the others (#6235).

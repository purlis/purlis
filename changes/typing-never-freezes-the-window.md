### Fixed

- **Typing into a chat no longer freezes the window while its session host is slow.** A pane's
  keys, paste, mouse, resizes and its view going on and off screen used to wait for the session
  host's answer before the window could draw again, for up to 30 seconds on a host that was busy
  or coming up. Each chat now has a queue of its own: what you send goes in at once, reaches the
  chat in the order you sent it, and other chats and the window carry on meanwhile. When it
  cannot reach the chat, because its runner is reconnecting or the host refused it, the chat's
  pane says so with the reason (#4734).

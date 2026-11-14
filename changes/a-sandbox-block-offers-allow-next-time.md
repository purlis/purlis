### Added

- **A sandbox block offers Allow next time and Show this rule**, on its row in the chat's Chat
  log and on the turn's finish row in Updates › Mine. Once the chat has stopped waiting on a
  connection the sandbox refused, Allow next time allows that host with no ask open, at one of
  the Allow labels a sandbox ask offers, and only the ones your administrator's policy allows.
  Allow once lets the chat's next attempt at that host through without asking, and is then gone;
  an attempt already waiting when you press it goes through too. Allowed in Settings › Sandbox ›
  Network lists it until then, and Remove takes it back. It is kept in memory only, so restarting
  the chat, its session host or purlis forgets it, and each grant, use and removal is in the
  chat's audit. A wider label stays until you remove it. On a finish row for several chats, each
  chat is named with its own Allow once and Allow only for this chat, and Allow for me on this
  machine and Allow for everyone in this project are one grant for all of them. purlis checks the
  block again before it allows anything, so Allow next time is offered and works only for a host
  the sandbox itself refused for that chat, and a chat can never press it. A label waits for its
  row to settle, as an ask's Allow does, and a row whose host changes under it settles again.
  Show this rule says which rule refused it. A sandbox block the chat's own hook reported, or one
  since resolved, offers Show this rule alone. A chat in an older session host can't have one
  attempt allowed, and is told so (T1-34, #6425).

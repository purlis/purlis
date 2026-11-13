### Fixed

- **A chat whose permission prompt purlis couldn't hold is listed in Asks.** When purlis can't
  hold a chat's permission ask (the ask is too big to show, its hook names another harness than
  the chat's, the hook has too little time left, or the chat already has 32 asks waiting), the
  harness asks in its own pane. On every harness, the chat is now listed in Asks as "Waiting in
  its terminal", with Go to chat, even beside the asks purlis still holds for it. You answer it
  in the chat's pane, and the row goes. A chat stopped at its harness's folder-trust question
  shows the same row where purlis heard its hook give up, and reads Can't tell in All chats
  otherwise. purlis never reads the terminal to tell (#6426).

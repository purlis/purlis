### Changed

- **A burst of chat events is one redraw of the window, however many it carries.** A chat's
  context gauge reads its record again at every move and every few seconds mid-turn, and each
  answer redrew the chat's pane even when it said what the last one did. An unchanged answer
  now redraws nothing, so what busy chats tell the window in one frame is drawn in a single
  commit (#4882).

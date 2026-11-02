### Changed

- **Many busy chats no longer flood the window with redraws.** What chats report about moving,
  what they are doing and which skill they run is now drawn once per frame, so a burst from many
  agents is one redraw instead of one for each event (#4731, part of #4724).

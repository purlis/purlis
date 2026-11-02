### Added

- **A slow moment in the window is written down, so it can be read afterwards.** Every command
  answer that takes 1 s or longer, and every stall of the window of 200 ms or longer, is
  appended to `slow.jsonl` in the app's log folder (on macOS
  `~/Library/Logs/dev.purlis.app/slow.jsonl`), with how many chats were working and, for a
  stall, the tab in front and whether a chat's view was opening. The newest 1,000 are kept, on
  this machine only. The window still says an answer is late only at 20 s (#4725).

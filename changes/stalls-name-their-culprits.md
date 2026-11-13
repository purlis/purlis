### Changed

- **A stall in the slow log says what held the window.** Each `stall` entry in `slow.jsonl` now
  lists the top five of each: the regions whose React commits took longest, the core's events
  and the command answers handled in that frame, and the scripts the webview says made it long
  (felt speed round 4, H9). A stall while the screen was locked, the display asleep or the
  window covered is marked `dark`, so time away is not counted as a frozen window.

### Fixed

- **A plane fetched or saved onto someone else's commits no longer redraws the whole window.**
  Auto-save fetches every five minutes. When the fetch or a save's rebase moved the plane's
  files, the window was told everything moved, and every view of every chat read and drew
  again. It is now told the files that moved (felt speed round 4, H9).

### Changed

- **A chat started by a handoff or a dispatch remembers which chat started it, by that chat's
  id.** It was kept by number alone, which is dealt again once that chat is gone. The id is
  kept across a relaunch; a chat you start yourself names none (#872).

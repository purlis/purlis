### Changed

- **A window listens for each of the core's events once.** However many of its parts wait for
  an event, the window holds one listener for it and hands it to each, where it held one per
  part: 75 listeners in place of 219 with 50 chats open. Giving a pane another chat no longer
  stops and starts its notices' listeners, which took 20 of its 33 asks to the core (#7909).

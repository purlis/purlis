### Added

- **Over the device's hot target, the chat that has rested longest is hibernated for you.** Once
  more chats are hot than the device's RAM class targets, purlis hibernates a rested chat, one
  that nobody has looked at or typed in for 30 minutes, at its turn's end, with nothing of its
  own running and no task open. It goes one at a time, longest-rested first, and stops at the
  target. Under the target a rested chat stays hot however long it rests. A chat on screen in a
  focused window, a runner's or a pool's chat, and a task are never hibernated this way. The
  rest is read on the machine's monotonic clock, which stops while the Mac sleeps (SC-4g,
  #7636).

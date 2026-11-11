### Fixed

- **Chats come back sooner at a launch.** A chat starting at launch no longer holds its place in
  the queue while it waits for its program to come up, so the next chat starts beside it, and
  terminal shells start before agent chats. A shell that took 11 seconds to come back now takes a
  fraction of one. The app's log now gives a start's wait in the queue apart from how long the
  start itself took.

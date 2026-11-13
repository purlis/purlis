### Fixed

- **Closing a chat no longer waits on what the session host is doing to it or to another chat.**
  A close used to wait behind the host's listing of what the chat had started outside its own
  processes, which can take seconds on a busy Mac, and behind a hold of any chat whose program
  was ending. The session host's log now also says when it answered something a second or more
  late, and how long of that the answer spent waiting its turn, and when its thread was held.

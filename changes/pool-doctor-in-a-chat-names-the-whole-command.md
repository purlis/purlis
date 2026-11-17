### Fixed

- **`purlis pool doctor` and `purlis pool setup` refused in a chat give the whole command to
  copy.** purlis's own shell tabs count as chats, and a plain terminal may have no `purlis` on its
  PATH, so the words now say to use a terminal outside purlis and print this `purlis`'s full path.

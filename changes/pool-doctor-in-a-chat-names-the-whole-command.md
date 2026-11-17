### Fixed

- **A `purlis pool` command refused in a chat gives the whole command to copy.** purlis's own
  shell tabs count as chats, and a plain terminal may have no `purlis` on its PATH, so `pool
  doctor`, `pool setup` and the others now say to use a terminal outside purlis and print the
  command as you typed it, with this `purlis`'s full path.

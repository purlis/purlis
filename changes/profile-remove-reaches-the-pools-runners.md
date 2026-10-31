### Changed

- **`purlis profile remove` reaches the pools' runners.** With the window running, each Docker
  pool's runner forgets the profile and its sign-in, as Remove in Settings does, and the command
  names any pool whose runner is too old to forget it. With no window, it says each runner
  forgets it at its next tidy, and no drain carries its sign-in.

### Fixed

- **A pool's Remove no longer deletes a harness's own folder under a running chat.** Forgetting a
  desktop's adopted folder on a runner is refused while a chat with no profile runs there on that
  harness, with a sentence that says to end the chat first.
- **Taking a profile off a pool clears its Ask in every open project**, not only the one you were
  looking at.

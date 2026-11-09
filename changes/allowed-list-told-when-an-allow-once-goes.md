### Fixed

- **The Allowed list drops an Allow once as soon as it goes.** It used to keep the row until it
  next read the list when the Allow once was used by its run, dropped because a later check
  refused that run, or went with its chat when the chat closed (#5800).

### Fixed

- **A message to or from a dispatched task is no longer lost when the app does not answer.**
  `tell`, `note`, `ask` and `answer`, and their tools, are queued and delivered, in order and
  once, when the app next hears from the chat; the sender is told the message is queued, not
  yet delivered, and `purlis dispatch list` shows each message as queued, delivered or
  undeliverable, with why (#3902).

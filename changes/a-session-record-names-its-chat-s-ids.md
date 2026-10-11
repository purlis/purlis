### Changed

- **A session record names its chat by its ids.** Each record written now carries the chat's
  `chat-id`, the `device` that made it and the `run` that wrote it, as the format has said it
  would, or `unknown` for one the app does not know (#871, #1092).

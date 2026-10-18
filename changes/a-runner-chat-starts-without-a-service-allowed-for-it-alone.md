### Fixed

- **A chat on a runner starts even when you allowed it a service on this machine alone.** The
  runner starts it without that service, and the chat's tab says so. When the chat reaches for
  it there, you are asked about the runner's service, and allowing it, once or for this chat,
  lets that chat reach it on that runner (#2510).

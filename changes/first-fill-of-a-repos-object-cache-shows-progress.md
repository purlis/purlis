### Fixed

- **A repo's first read no longer blocks every push with no word of how far it has got.** The
  refusal now says how many objects purlis has read of how many, and about how long is left, and
  the log records it every 30 seconds. A push whose base purlis already holds goes ahead while the
  rest is read, and the first read starts when a chat opens on the repo, not at its first push
  (#7532).

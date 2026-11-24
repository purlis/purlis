### Fixed

- **A repo's first read no longer blocks every push with no word of how far it has got.** The
  refusal now says how many objects purlis has read of how many, and about how long is left (never
  more than the read's own 15 minutes), and the log records it every 30 seconds. Git writes the
  refs a push is checked against only when a read ends, so a first read still has to finish before
  a push to that repo goes ahead; where the cache already holds a push's base (a read again over a
  cache that has refs, or a top-up that ran out of the push's time) the push goes ahead while the
  rest is read. The first read starts when a chat opens on the repo, not at its first push (#7532).

### Fixed

- **A repo's first read no longer blocks every push with no word of how far it has got.** The
  refusal now says which of the three parts of the read purlis is in, how many objects it has read
  of how many, and (in the last part, where a rate means something) about how long is left, never
  more than the read's own 2 hours; the log records the figure every 30 seconds. A push goes ahead
  only once the repo's history has been read whole: until then the read is refused, and a push whose
  base purlis already holds (or a top-up that ran out of the push's time) goes ahead while the rest
  is read. The first read starts when a chat opens on the repo, not at its first push (#7532).

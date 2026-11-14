### Added

- **Doctor checks a Docker pool on another machine end to end.** Check again in Pool setup and
  `purlis pool doctor <pool>` run the same checks and say each finding in the same words: the
  machine's name, its port, your SSH sign-in, its key, Docker's socket and your account's right to
  it, TLS, the engine, whether Docker there confines runners, its platform, the runner image's
  download, its disk, and a Docker context that now points elsewhere. Each finding has one next
  step, with its commands under See what this does. Try it shows the link's measured round trip,
  and Get help and Send to your admin name the machine's route, host and fingerprints, never a
  key or a value (#6272).

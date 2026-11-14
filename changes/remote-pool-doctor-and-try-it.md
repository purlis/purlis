### Added

- **Doctor checks a Docker pool on another machine end to end.** Check again in Pool setup and
  `purlis pool doctor <pool>` run the same checks and say each finding in the same words: the
  machine's name, its port, your SSH sign-in, its key, Docker's socket and your account's right to
  it, TLS, the engine, whether Docker there confines runners, its platform, the runner image's
  download, its disk, and a Docker context that now points elsewhere. Each finding has one next
  step, with its commands under See what this does. Where the machine lacks the runner image,
  Check again downloads it there first. Trust this machine already checks that Docker there
  confines runners and that the runner image is built for its platform, and Connect marks a check
  as passed only once it has run. Try it shows how long a docker command takes on the machine.
  Get help and Send to your admin name the machine's route, host and fingerprints, and never a
  key, a value or a path to a file on your computer (#6272).

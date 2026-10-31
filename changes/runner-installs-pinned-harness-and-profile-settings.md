### Added

- **A chat on a pool's runner runs the harness version its profile pins.** The first chat on a
  pinned profile there downloads that version from the harness's own release, checks it against
  the digest its vendor publishes, and keeps it on the runner's volume. A version that cannot be
  checked is not installed, and the chat says why (#3446).
- **A profile's settings reach the pool's runner that keeps it.** Its instructions, agents,
  commands and other settings files go with each chat's start or move there, so a runner chat
  behaves as one on your machine. No sign-in and no file named like a credential is sent, and
  only to a pool shown to be your own (#3448).

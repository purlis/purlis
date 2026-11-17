### Added

- **`purlis policy show --default` prints the shipped project policy.** purlis is moving its
  rules about what a chat may do into one project policy, written in Cedar. This build carries
  the schema those rules are checked against and a first stand-in of the default policy, and the
  new `purlis policy` command prints that default. Nothing a chat does is decided by it yet.

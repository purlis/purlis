### Fixed

- **A large commit message or pull request body no longer runs the guard out of its time.** The
  guard read the same large text several times in one check, and on a slow machine a 64 KB body
  took longer than the guard's two seconds and was refused as one purlis could not check in
  time. It now reads such a text once per check (#7245).

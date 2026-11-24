### Fixed

- **A chat's task branch cut from an integration branch pushes through the forge route.** A branch
  based on `phase-1-proven` or another branch that is not the default was refused with "git did
  not take its objects in". purlis now reads the pack against the branches the forge lists. A
  change to a CI file on such a branch is still held for you.

### Fixed

- **Git works in a clone at the floor on Linux.** A Claude Code chat at the floor on Linux could
  not commit or check out when the home had no `.gitconfig` yet: git stopped on a config it could
  not read. purlis now makes that file, empty, before such a chat starts. A `.gitconfig` you
  already have is left as it is.

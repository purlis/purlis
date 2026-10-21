### Added

- **The window says when a runner's key changed.** When the runner a project's chats run on
  answers with another key than the one it was paired with, a Notice says so once for the
  project, with Open Pools settings (#2752).

### Fixed

- **Private-registry pulls work from a window opened from the Finder.** purlis runs `docker`
  with Docker's credential helpers found where `docker` itself is (#2752).

### Security

- **A chat cannot write the home folders Docker links `docker` into.** `~/.docker/bin`,
  `~/.orbstack/bin` and `~/.rd/bin` are denied to every chat's writes (#2752).

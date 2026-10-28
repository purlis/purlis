### Added

- **A runner chat on a Docker pool has a working browser.** The runner image now carries what
  the browser engine needs, and a Docker pool makes each runner so the engine can start with its
  own sandbox on: Docker's default seccomp profile, plus the user, process and network
  namespaces the engine's sandbox makes. Each runner also gets a quarter of its memory as shared
  memory. A chat in purlis's own Linux sandbox still makes no namespace. A runner made before this
  version keeps working, but its browsers don't start: remove its pool and add it again (#3298).

### Changed

- **A sandboxed chat makes no namespace of its own on Linux.** On every machine, its own filter
  refuses `clone` and `unshare` with a `CLONE_NEW…` flag, so it reaches no more of the kernel
  than before, even where a browser engine beside it may make one (#3298).

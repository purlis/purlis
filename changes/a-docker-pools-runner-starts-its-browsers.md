### Added

- **A runner chat on a Docker pool has a working browser.** The runner image now carries what
  the browser engine needs, and a Docker pool makes each runner so the engine can start with its
  own sandbox on: Docker's default seccomp profile, plus the user, process and network
  namespaces the engine's sandbox makes. Each runner also gets a quarter of its memory as shared
  memory. Only the browser engine may make namespaces on a runner, and only an image whose
  purlis keeps every other program from making them gets that profile; a team's image gets it
  once it is rebuilt on this version. A runner a chat ran on without the sandbox starts no
  browser and is filled no login until it is made again, and the browsers already open there
  end. A runner made before this version keeps working, but its browsers don't start: remove
  its pool and add it again (#3298).

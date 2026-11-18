### Added

- **The diagnostic log says how long each step of a remote start took.** A chat on a Docker pool
  over SSH, and a browser's first page, write one line per step (`purlis: step
  link_handshake done after 118 ms`, with the pool's name where there is one): the SSH forward,
  each `docker` check of the engine and the runner's container, the link's handshake, the
  runner's start, the profile check, the program's start and the browser engine's download and
  start. A step that ended in an error says `refused after`; each `docker` call inside a step is a line at `debug`. The lines stay on this machine;
  nothing is sent anywhere.

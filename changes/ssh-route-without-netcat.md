### Fixed

- **A chat's ssh connects on Fedora and RHEL.** Its route ran the system's `nc` with OpenBSD
  netcat's proxy flags, which ncat refuses, so ssh never connected there. The route now carries
  ssh through the chat's SOCKS port with purlis's own command line, so it needs no `nc` on any
  system, and where none is found ssh says why (#3536).

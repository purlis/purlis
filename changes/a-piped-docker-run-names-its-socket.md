### Fixed

- **A Docker command run down a pipe gets its Allow.** When a sandboxed chat ran
  `docker run … 2>&1 | tail` and the sandbox refused Docker its socket, the Notice said "The
  sandbox did not say which socket it refused" and offered only *Start without the sandbox*.
  The pipe ended with `tail`'s status, so the command did not count as failed, and a socket was
  named only from a failed command. A line that runs `docker`, `docker-compose`, `podman`,
  `podman-compose` or `nerdctl` now names the socket its output says it was refused, so the
  Notice shows the socket and offers *Allow once*, *Allow for this chat* and *Always allow*.
  What a line that only reads printed (`cat` of a log, `git show`) still names no socket.

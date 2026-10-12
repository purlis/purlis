### Added

- **A blocked local socket or system service can be allowed.** When a sandboxed chat is refused
  Docker's socket, a database's, an SSH agent's, or macOS's certificate check that Go tools such
  as `gh` and `glab` use, its Notice names it, says plainly what allowing it means (a container
  engine's socket is about the same as no sandbox), and offers **Allow once**, **Allow for this
  chat** or **Always allow** for this project on this machine, or **Keep blocked**. Nothing is
  allowed by default. What you allow is listed in Settings with Remove, and kept only on your
  machine, never in the project's committed file. A project can declare the sockets and services
  it expects with `[sandbox].expects`, which changes only what the Notice says. An
  administrator's policy can forbid all of it with `"socket-grants": false` (ADR 0093).

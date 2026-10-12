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

### Changed

- **`[sandbox].certificate-checks` now says a project expects certificate checks, and allows
  nothing by itself.** Projects that set it now get one Allow per machine the first time a Go
  tool checks a certificate: the Notice says the project expects it, and Always allow keeps it
  for that project on that machine (ADR 0093).

### Security

- **What you allow on this machine is kept outside the project.** The sandbox record (folders,
  hosts, sockets and services you allowed, and which project grants you accepted) moves from the
  project's `app/sandbox.json` to `~/.config/purlis/sandbox-grants/`. The older file is taken
  once, only where git says it is this machine's, never one the repository carries; anything in
  one that is not taken is asked about again (ADR 0093).

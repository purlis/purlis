### Added

- **Sandboxed Codex and opencode chats on Linux.** A project that runs its chats sandboxed now
  starts Codex and opencode chats on Linux inside purlis's own sandbox, built from the same
  policy as on macOS, where before they were refused. It needs no extra package, privilege or
  namespace, so it works the same on a Linux desktop, in a Docker container and in a Kubernetes
  pod. It needs Linux 6.12 or later (Landlock version 6). In a project with `mode = "on"`, an
  older kernel refuses the chat and says how to go on; elsewhere the chat starts without the
  floor and its tab says why (#1040).

### Changed

- **Codex and opencode chats on Linux run at the floor**, as on macOS, in a project that has not
  turned the full sandbox on. So a Codex chat there starts only on a profile for sandboxed chats,
  a chat at the floor does not run over ACP, and `purlis secret get` and `purlis secret exec`
  from such a chat are refused until #2342 (#1040).

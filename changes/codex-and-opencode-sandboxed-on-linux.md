### Added

- **Sandboxed Codex and opencode chats on Linux.** A project that runs its chats sandboxed now
  starts Codex and opencode chats on Linux inside purlis's own sandbox, built from the same
  policy as on macOS, where before they were refused. It needs no extra package, privilege or
  namespace, so it works the same on a Linux desktop, in a Docker container and in a Kubernetes
  pod. It needs Linux 6.12 or later (Landlock version 6); on an older kernel the chat is refused
  and says so (#1040).

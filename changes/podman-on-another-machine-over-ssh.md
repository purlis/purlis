### Added

- **A pool on another machine can run its chats on Podman there, rootless or rootful, reached over SSH.** In Pool setup's Connect step, pick the machine from your Podman connections (`podman system connection list`) or type its address: purlis finds Podman's socket there and signs in with the key file the connection names, or your SSH agent. purlis reaches that Podman with this machine's own `podman`, and stops the pool if Podman's store there changes. From a terminal, `purlis pool add … --on ssh://you@host --identity <key file>` does the same (#6269).

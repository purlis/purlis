### Changed

- **On Linux, a lookup refused at the system's name lookup is one Notice.** On a host whose name
  lookups go through a local socket (systemd-resolved, nscd, sssd or avahi), a program's failed
  lookup in a Codex or opencode chat is shown once, as a lookup past the sandbox's proxy, not
  also as a local socket. Its words say what letting a chat reach that lookup would mean (#2428).

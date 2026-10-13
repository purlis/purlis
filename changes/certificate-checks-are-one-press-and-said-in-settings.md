### Changed

- **Certificate checks are on by default in every sandboxed chat.** `gh`, `glab` and other Go
  tools verify a host's certificate through a macOS system service, and a sandboxed chat was
  refused it until each person allowed it on each machine, so those tools failed with
  `x509: OSStatus -26276`, or with an error that hid the cause (`gh auth status` said the token
  was invalid). Now every sandboxed chat has it, on Claude Code, Codex and opencode, with
  nothing to allow:
  - Settings › Sandbox has one switch, **Certificate checks**, on unless the project turns it
    off. The separate "on this machine" row is gone;
  - `[sandbox] certificate-checks = false` turns it off for everyone in a project. `true` says
    what leaving the key out says. **A project that never set the key now has the check**;
    nothing in your files changes;
  - an administrator's `sandbox.socket-grants: false` turns it off on that machine, and
    Settings says who set it;
  - a chat that was already running when you updated keeps the sandbox it started with: the
    Sandbox changed Notice lists it with Restart, and a block in it says the same.

  What this leaves open, by decision: that system service makes its own requests, for revoked
  certificates and for the certificates a certificate names, and those do not go through the
  sandbox's proxy. Turn the switch off in a project where that matters more than those tools.

### Fixed

- **A chat that waits for its turn to end before it restarts says so.** A chat that is
  mid-turn when you allow something says that it restarts once its turn ends, with **Restart
  now**, where it used to show nothing until the turn ended. And a chat blocked on a socket or
  a service you already allowed on this machine offers **Restart this chat**, where it used to
  ask for the same Allow again.

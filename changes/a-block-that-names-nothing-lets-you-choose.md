### Added

- **A sandbox block that names no socket or service lets you choose it.** When the sandbox
  refused a local socket and nothing said which (a build runner that started Docker, a
  background command read back later), the Notice said there was no socket to allow and offered
  only *Start without the sandbox*. It now also offers *Choose the socket…*: a list of the
  sockets purlis finds on this machine (Docker's, Podman's, your SSH agent's, a database's) and
  a box to type a path. The socket you choose is checked as one the sandbox named is, with the
  same warning for a container engine's, and then offers *Allow once*, *Allow for this chat*
  and *Always allow*. The certificate check is chosen the same way, from a list. Only you can
  choose, in the window; nothing a chat printed fills the list or the box.

### Changed

- **A sandbox refusal a chat only read back is a quiet line.** When a chat reads the output of
  a command that ran earlier and it holds a refusal's words, the Notice now says so, asks
  nothing, and stays out of the Inbox. *Show what you can do* opens the choices it had before.

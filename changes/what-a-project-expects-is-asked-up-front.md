### Added

- **A project that expects a local socket or a system service asks you once, when you open
  it.** A project's settings can declare what its chats need (`[sandbox] expects`), and a
  declaration allows nothing by itself. Until now you learned of it when a tool failed in a
  chat. Now one notice lists each item not yet allowed on this machine, with what allowing it
  means, and offers **Allow on this machine** or **Not now**. Not now is remembered until the
  project declares something new, and Settings › Sandbox keeps a row, **This project expects**,
  with the same Allow. A chat that starts before you answer says so in its pane. Allow keeps
  each item as the notice showed it: one that changed on this machine since is shown again
  instead. A project declares at most 32 items.
- **Allow a socket or a service for every project on this machine.** A sandbox block on a local
  socket or a system service now offers **Allow on this machine, for every project** beside
  Allow once, so Docker's socket or your SSH agent is not asked about again in each project.
  It is listed under Settings › Sandbox › Network as "Every project on this machine", in every
  project, and Remove takes it from all of them. Hosts and folders stay per project.

### Changed

- **Chats already running are told when a socket or a service is allowed.** The Sandbox
  changed notice now lists a chat that started before a socket or a service was allowed for
  its project or for every project, with Restart.

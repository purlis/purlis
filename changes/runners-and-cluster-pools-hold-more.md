### Changed

- **A runner's own policy decides whether a chat it runs may ask about a host.** Where a runner's
  administrator policy turns live asks off, or keeps no Allow for one chat, a chat the runner
  composes or the desktop relays to it refuses an unlisted host and asks the window nothing, as it
  already did for the runner's services.
- **A Docker pool's runner is checked against every container setting that could give it more.**
  Its container is refused when it carries extra groups, a device cgroup rule, protection from the
  out-of-memory killer, a cgroup of its own choosing, a `/dev/shm` its pool did not give it,
  unmasked kernel paths, or any setting the check does not know.
- **A cluster pool's credential plugins are kept by their full path.** The pool's own copy of the
  context names each plugin by the path its add found and held, so no later kubectl run looks it up
  on its PATH. A plugin inside a Python virtual environment is held with the whole environment, and
  one whose package is inside a chat's project is held with its package.
- **The cluster operator holds more of what it makes.** Only the operator makes a purlis pool's
  SandboxTemplate or changes its spec; each runner namespace lets its pods out only where a network
  policy lets them (`purlis-deny-egress`); what the operator makes it holds as applied from the
  first, so a field it stops setting is taken out; and a policy binding that narrows what the
  policy governs does not put it in force.

### Fixed

- **A second session host no longer starts beside one whose lock file the system cleaned away.**
  The host and the window also find a host serving by its socket.
- **A GitHub App token is never kept for a chat that ended while its run was being served** on
  this machine.
- **Claude Code chats are held from writing a program held by its own name**, as wrapped chats
  already were.

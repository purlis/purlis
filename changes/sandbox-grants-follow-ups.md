### Added

- **An administrator can forbid "on this machine, for every project".** The machine policy's
  `sandbox.allow-scopes` has a word for that scope, `machine`, beside `chat`, `you` and
  `project`. A policy that lists scopes and leaves it out removes it from every Notice and
  refuses an Allow at it, saying who set the policy; a policy with no `allow-scopes` key
  changes nothing. `once`, the scope of "Allow once", is a word of the list too, and a Notice
  names each scope a policy removed (#1927).

### Changed

- **Removal, grant, settings and credential commands are pressed in purlis's own window.**
  A page shown anywhere else lists what there is and says where the press is made (#1927).

- **What you allowed before a policy fixed the scopes no longer reaches a chat at a scope it
  leaves out.** A folder, a local socket, a system service or a host an Allow kept at such a
  scope, and anything one chat holds at one, is judged against the policy at every start and contributes
  nothing while it stands. It stays on this machine, listed in Settings › Sandbox › Allowed as
  not in force, held by policy, with Remove; `purlis doctor` counts it (#1927).

### Fixed

- **Only the certificate check is told as the certificate check.** A sandbox report naming a
  system service that merely has the certificate check's service name inside its own is sorted
  as a system service and shown by its whole name (#1927).
- **Allow on a blocked local socket keeps the socket its Notice named.** If the path leads
  somewhere else by the time you press, nothing is allowed and the Notice shows the block
  again as it stands (#1927).
- **"Not now" to what a project expects is an answer to what was shown.** If a path the
  project declared comes to lead to another socket, the question is asked again (#1927).

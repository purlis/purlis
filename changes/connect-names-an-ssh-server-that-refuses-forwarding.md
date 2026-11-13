### Fixed

- **Connect says when the other machine's SSH server doesn't allow forwarding.** purlis reaches
  Docker on another machine by forwarding its socket over SSH. A server with
  `AllowTcpForwarding no`, `AllowStreamLocalForwarding no` or `DisableForwarding yes` signs you
  in and refuses the forward. Connect and `purlis pool setup <pool>` said only that Docker didn't
  answer there. They now say the SSH server doesn't allow forwarding and what to ask its admin
  (#6349).

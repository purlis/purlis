### Added

- **A sandboxed chat on a runner is confined there as on this machine.** `purlis serve` starts
  a chat's egress proxy with the same code the app's own session host runs, a runner composes
  every harness chat it runs with the sandbox it compiles for itself, and a chat's proxy, its
  asks and a withdrawn port reach the window through the link between hosts (#2217).

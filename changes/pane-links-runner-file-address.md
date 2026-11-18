### Changed

- **A `file:` address a runner chat prints links as it does on this machine.** On a chat that
  runs on a pool's runner, `file:///etc/hosts` or `FILE:///etc/hosts:3` now opens the runner's
  `/etc/hosts`, at its line, read-only. Before, the address was asked for under its whole name, so
  it never linked. A `file:` address naming another machine still opens nothing (#7081).

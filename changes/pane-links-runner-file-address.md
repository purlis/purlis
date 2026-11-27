### Changed

- **A `FILE:` address a runner chat prints links as it does on this machine.** On a chat that
  runs on a pool's runner, `FILE:///etc/hosts:3` now opens the runner's `/etc/hosts`, at its
  line, read-only. Before, only an address with an uppercase `FILE:` scheme failed to link there;
  a lowercase `file:` one already linked. A `file:` address naming another machine still opens
  nothing (#7081).

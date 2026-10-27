### Added

- **Devices: this machine and every runner, in a tab of their own.** Once this machine keeps a
  pool, Devices in the palette lists this machine and each runner it keeps, each with the chats
  and browsers on it, the version it runs, whether it answers or is suspended, and when it last
  answered. Under them, each pool's use: its runners running and suspended, and the CPUs and
  memory they request. A suspended runner is never woken to be asked (#2052).

### Security

- **Text from Docker and runners is shown safely in the window.** What Docker or a runner says
  about a pool, in Devices, the Pools settings, a project's mark and a pool's removal, is drawn
  on one line, capped, apart from purlis's own words, without characters a reader does not see
  and without the user or password of an address (#2052).

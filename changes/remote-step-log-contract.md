### Added

- **Detailed remote log for one hour.** Settings › You › Diagnostics has a switch that writes
  every step of starting a chat on a pool to this machine's diagnostic log, each Docker call too,
  and turns itself off after an hour. ssh's own output is never written, and nothing is sent
  (#7864).

### Changed

- **The remote start's step lines are a contract** (#7864). Their names are fixed, and a rename
  from now on is a breaking change, said here. New steps: `link_dial`, `code_send`,
  `hold_project`, `browser_first_page` and `view_first_frame`. A `refused` step is now a warning;
  `done`, `cancelled` and `gave_up` stay at info. Each line's elapsed time is the `ms` field
  (was `elapsed_ms`), and a line may also carry `cause`, `attempt` and `start`, the id of the
  Start the person pressed, which the runner's `harness_spawn` line carries too (session
  protocol 1.97). A pool is named only by its alias, never by a host, user or path.

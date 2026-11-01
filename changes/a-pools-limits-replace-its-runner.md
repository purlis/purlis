### Added

- **Change a Docker pool's CPUs, memory, processes, `/tmp` and suspend time on its page.** Each
  change waits for the runner to be replaced, and the page says so. **Replace the runner…** makes a
  new runner held to the pool's limits: new chats start there, and chats running now carry on on the
  old runner until they end. An error from a limit also offers to raise it where a pool's profiles
  are asked about or changed, and `purlis profile sign-in --pool` names the value (#4286, #4397).

### Fixed

- **A pool with its own CPU, memory, process or `/tmp` limits now links to its runner.** purlis read
  a kept runner back with the default limits, so it refused the runner it had just made (#4286).
- **A definite refusal no longer offers a longer wait.** A code send or a cluster add that was
  refused after a long wait says why, and offers more time only where the wait itself ran out
  (#4398).
- **A limit's row shows what was saved** even if reading the limits again fails (#4390).

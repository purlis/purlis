### Added

- **Change a Docker pool's CPUs, memory, processes, `/tmp` and suspend time on its page.** Each change waits for the runner to be replaced, and the page says so. **Replace the runner…** makes a new runner held to the pool's limits: new chats start there, and chats running now carry on on the old runner until they end (#4286).
- **A limit offers its new value in more places.** A profile's row on a pool's page and a chat moving back here now have the form beside the refusal, and `purlis profile sign-in --pool` names the value (#4397).

### Fixed

- **A definite refusal no longer offers a longer wait.** A code send or a cluster add refused after a long wait says why, and offers more time only where the wait itself ran out (#4398).
- **A pool's page now uses the answer wait you set** when it asks a runner about its profiles (#4397).
- **A pool with its own CPU, memory, process or `/tmp` limits now links to its runner.** purlis read a kept runner back with the default limits, so it refused the runner it had just made (#4286).
- **A limit's row shows what was saved** even if reading the limits again fails (#4390).

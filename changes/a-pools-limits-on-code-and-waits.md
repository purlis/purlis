### Changed

- **A pool's code send and waits are now its limits, and running out offers to raise them.** The
  largest code sent to a runner (1 GiB), the longest code send (10 minutes), the longest wait for a
  sign-in (15 minutes), for a cluster runner to be vouched for (5 minutes) and for a runner's answer
  (30 seconds) can each be raised on a pool. Hitting one says so and suggests a value you can edit:
  what is needed plus a margin for the size, double for a wait. Applying it saves the value to the
  pool and tries again. Only the wait changes: every check still runs. What a runner sends back to
  this machine stays held to 1 GiB and 10 minutes, whatever these say (#4282, #4283).

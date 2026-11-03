### Fixed

- **A chat put back at launch no longer waits for ever on a pool's runner.** A start nobody can
  cancel, at launch or a dispatch's, now waits at most the pool's *Longest wait for a runner's
  answer* (30 seconds unless the pool sets it). Past that, the chat says the runner did not
  answer in time, and Retry starts it on the pool's runner of that moment. One launch waited
  4 minutes on a runner older than the desktop (#4988).

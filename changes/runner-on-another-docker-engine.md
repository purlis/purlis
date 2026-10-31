### Fixed

- **A pool's runner is no longer called removed when Docker points at another engine.** purlis now
  keeps which engine made each runner. When another engine answers, the pool's row says so,
  **Make the runner again** is not offered, and the runner's chats come back once Docker points back
  (#4166). This holds for runners made from now on: a runner made before this keeps today's check.
- **A runner made again takes its sign-ins from its own home whatever the clocks say.** purlis
  compares the home's own creation time with the one kept when the runner was made, and a volume made
  again under the same name gives nothing (#4167).

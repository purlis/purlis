### Fixed

- **Pool setup's Sign-ins step measures each profile against what the pool's runner takes.** On a
  runner from before 1.64, which stages less than a pool's limit allows, the step said a profile
  fit when a send would refuse it. It now holds each profile to the lower of the two, where the
  runner can say which version it runs (#4719).

### Added

- **A panic is sent as a Crash report, and you can see exactly what left.** With Crash reports
  allowed, a panic in the app goes to purlis's private report service (Sentry, in the EU) with
  only where it panicked, its message, purlis's version and channel, and your operating system;
  paths and the names of your projects, workspaces and repos are taken out first. Settings › You
  › Reports lists every report sent from this machine for 90 days, and each opens the exact bytes
  that were sent. The report service is listed in the network log (#5689).

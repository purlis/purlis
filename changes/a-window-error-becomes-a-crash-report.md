### Added

- **An error the window did not catch is sent as a Crash report, as a panic is.** With Crash
  reports allowed, an uncaught error or unhandled rejection in purlis's window goes to purlis's
  private report service with only its name, its message, the file and line it was thrown at,
  purlis's version and channel, and your operating system; paths and the names of your projects,
  workspaces and repos are taken out first. It is listed in Settings › You › Reports with the
  exact bytes sent. Without Crash reports allowed, nothing is sent (#5690).

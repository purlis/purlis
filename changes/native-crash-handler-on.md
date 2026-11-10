### Added

- **A native crash is sent as a Crash report on the next launch.** On macOS, a crash that is not
  a Rust panic (a segfault, a stack overflow, an uncaught Objective-C exception) leaves a
  minidump on your Mac. With Crash reports allowed, the next launch sends its crashing thread's
  stack alone, never the minidump, which stays on your Mac and is removed after 14 days.
  macOS's "quit unexpectedly" dialog still appears on such a crash (#5691).

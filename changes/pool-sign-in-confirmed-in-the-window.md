### Changed

- **A pool sign-in started from the command is confirmed in the window.** `purlis profile sign-in
  <profile> --pool <pool>` now asks the running purlis window, on Docker and cluster pools alike,
  and goes on once you press Sign in there. With no window running, the command says so and starts
  nothing.

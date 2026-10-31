### Added

- **Codex and opencode's ChatGPT sign-in finish on a runner by approving in your browser.** Sign in
  on a pool's profile opens the page, and there is nothing to paste: for as long as the sign-in
  runs, purlis listens on this Mac's `localhost:1455` for that one sign-in's redirect and carries it
  to the runner. If another program holds the port, the strip says so and offers a device code
  (`codex login --device-auth` on the runner). Where purlis cannot follow such a sign-in, it restarts in the
  runner's terminal in the strip by that device-code flow, since nothing carries the browser's
  answer to a terminal.

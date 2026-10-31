### Fixed

- **A sign-in on a pool keeps its Ask while the runner cannot tell yet.** Signing a profile in on
  a pool with Open in terminal, its Ask could leave the Inbox while you were still in the browser,
  on a runner that could not tell yet or was still fetching the harness. It now stays until the
  runner says the profile is signed in, or you close that shell (#4009).

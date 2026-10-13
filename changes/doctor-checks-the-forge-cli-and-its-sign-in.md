### Changed

- **`purlis doctor` checks `gh` or `glab` and its sign-in.** For each forge a workspace repo has
  its origin on, the `gh`/`glab` row says where the CLI is, or that it is not found, and the
  `… auth` row asks the CLI whether it is signed in on that host. purlis never reads the token.
  A forge no repo is on yet is said as unused instead of as not checked. The preflight a chat's
  start runs starts no CLI.

### Fixed

- **purlis no longer runs into its own chat sandbox.** `purlis doctor` run inside a chat skips the
  hook spool, vault and credential-file rows and says so, and a hook that runs inside a chat's
  sandbox (Codex, opencode) no longer tries to spool a line or take a waiting report or message
  there, which the sandbox denies and which drafted a Report of a purlis bug each time. When
  the sandbox does block purlis's own command, the Report now names which `purlis` command it was.

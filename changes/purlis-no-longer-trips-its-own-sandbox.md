### Fixed

- **purlis no longer runs into its own chat sandbox.** `purlis doctor` run inside a chat does not
  look at the hook spool, vault and credential files, and shows each of those rows as a warning
  that it was not checked. A hook that runs inside a chat's sandbox (Codex, opencode) no longer
  tries to spool a line or take a waiting report or message there, which the sandbox denies and
  which drafted a Report of a purlis bug each time. A command that only reads this machine's
  device id no longer rewrites the machine store. When the sandbox does block purlis's own
  command, the Report now names which `purlis` command it was.

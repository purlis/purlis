### Added

- **A high-risk ask says its risk and asks once more.** An ask that pushes to a protected branch,
  reaches a host that looks like production, or writes outside the project says so on its row in
  Asks ("Pushes to main, a protected branch"), and its Allow asks again in the row ("Allow a push
  to main?") before it acts. It is never grouped with other chats' asks. A protected branch is a
  repo's default branch or one named in `[risk] protected_branches` in `charter.toml`, and
  `[risk] production_hosts` names the project's production hosts (#6011).

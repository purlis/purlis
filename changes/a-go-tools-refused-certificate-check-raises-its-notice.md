### Fixed

- **A Go tool refused the certificate check now raises its notice.** In a sandboxed chat,
  `glab` or `gh` failing with `x509: OSStatus -26276` raised no notice, because the failure
  was only read from a command that succeeded. It is now read from a command that failed too,
  so the notice comes up with what you can do about it.

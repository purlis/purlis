### Fixed

- **A vault's tab says what the system keychain kept.** When a token you store replaces older
  ones that purlis could not remove from your system keychain, the tab now says how many are
  left there, rather than only logging it (#1528). The one note that a vault's secrets were
  moved under purlis's access rule is now said in the vault's tab too, after the reveal or copy
  that moved them, not only by the next `purlis secret` command (#1180).

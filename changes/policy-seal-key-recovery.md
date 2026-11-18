### Fixed

- **A seal key another program made can be recovered.** Accept says which keychain item to
  delete (`purlis-seal`, in Keychain Access or with `security delete-generic-password`), and the
  next Accept makes a new key without restarting purlis. A key whose write was cut short is
  restored from its spare instead of replaced, so what you accepted still holds.
- **A git failure no longer changes what you accepted.** Where git cannot read the project's
  last commit, the project's `[policy]` runs as `strict` only until git reads it again; a blob
  git cannot read, or a `.git` that does not read, is `strict` too, never "no file" or the
  working tree's file.
- **A commit's logged scan finding goes to the asking chat's own log**, and the log's cap no
  longer reads the whole file for each line.

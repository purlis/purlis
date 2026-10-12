### Changed

- **A kept 1Password token says why it is not used.** After the vault's provider or its file
  of references changed since the token was stored, the vault's tab and every read said only
  that no token was kept. They now say which of the two changed, without naming either, and ask
  for the token again from the vault's tab. A token stored before purlis recorded these two
  settings records them at its next read (#1828).

### Fixed

- **A 1Password token is not stored where purlis finds no `op`.** It used to be kept with no
  program pinned, so the vault looked set up and every read was then refused until the token
  was given again. Storing it now says purlis could not find the 1Password CLI, names every
  directory it searched, and keeps nothing (#1517).

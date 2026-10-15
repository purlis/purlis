### Added

- **The picker a chat starts from shows who pays.** Each profile's row says its model (the
  harness's default for now) and what a chat on it is billed to: your plan, your key, your cloud
  account, or nobody for a local model, with the profile's label, or whatever the harness is
  signed in with where purlis cannot see it. A row whose chat would not start says why. A chat's
  run records its profile's sign-in kind as its model source (#2036).
- **A profile of this machine can name its key or cloud credentials by vault reference.** A key
  profile waits for the key proxy, and a cloud profile for #2151; each one's row and start say
  so (#2036).

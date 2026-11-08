### Added

- **Signed commits on GitLab**, gitlab.com and your company's own server alike. Setup adds the
  key to your GitLab account as a signing key only, so it can't be used to log in, with the
  sign-in purlis already has: no extra permission to give. GitLab can't check a commit purlis
  hasn't pushed, so "Verified on GitLab ✓" shows once a chat has pushed its first signed commit
  there and GitLab says so. A GitLab server too old to verify signed commits says so, and isn't
  offered setup. The card shows a row for each GitHub and GitLab account (#5629).

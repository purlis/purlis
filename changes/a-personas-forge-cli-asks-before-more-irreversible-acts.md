### Security

- **A persona's forge CLI asks before more irreversible acts.** A persona that declares `gh` or
  `glab` no longer runs these without a prompt: a raw `api` call that may write, revoking or
  rotating a credential, transferring ownership, or changing a repository's visibility (#1096).

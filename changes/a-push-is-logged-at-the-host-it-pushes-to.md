### Changed

- **The network log names the host a push goes to.** A push to a remote by its name is listed at
  the URL it pushes to, after the remote's `pushurl` and your `pushInsteadOf` rewrites, and no
  longer at the one it fetches from (#1828).

- **One git process fewer for most git calls.** purlis keeps a repository's config listing, which
  it reads before a git call to turn off the programs the repository names, until a file it was
  read from changes, and checks that before every call.

### Changed

- **A forge sign-in that stops renewing is now said in the window.** When the application pinned for a
  host is no longer the one an account signed in through, or renewals keep failing, a Notice names the
  account and offers Sign in again; before, only a sign-in a forge had ended was said.
- **A token given by variable is read more strictly.** `GH_TOKEN` and `GITHUB_TOKEN` set to different
  tokens are refused instead of the first winning, a variable that is not text is an error, and a CI
  job's token (`CI_JOB_TOKEN`) is taken whatever its length.

### Fixed

- **Renewing a sign-in can no longer replay a spent refresh token or overwrite a newer sign-in.** A
  renewal that cannot take its machine lock is not made, a renewed pair is written only over the refresh
  token it came from, and a pair the keychain refused is held by marker so another process leaves it alone.
- **A chat is given no `CHARTER_FORGE_*` connection variable,** as it is given no `PURLIS_FORGE_*` one.

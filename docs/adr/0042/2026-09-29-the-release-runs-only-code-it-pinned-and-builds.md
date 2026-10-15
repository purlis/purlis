# Amendment, 2026-09-29: the release runs only code it pinned, and builds cold

The environment decides which refs reach the keys. It does not decide which code runs beside
them, so two more rules hold for every job that has a signing secret, the `release`
environment or the power to publish:

- **Every action is pinned by commit.** Each `uses:` in every workflow names a full commit SHA,
  with the version it was resolved from as a comment. A tag or branch is a name its owner can
  move; a SHA is the tree that was reviewed. Dependabot's `github-actions` updates move the SHA
  and the comment together. `dtolnay/rust-toolchain` has no version tags, so it is pinned to a
  commit of its default branch with the toolchain named, and moved by hand.
- **No cache, restored or saved.** A cache holds whatever the last job to save it left, and
  that job had none of these jobs' care. So `plan`, `build` and `publish` restore no Rust
  cache and no npm cache and save none: they build from the checkout and the lockfiles alone.
  A release build is slower for it, which is the trade. `ci`, `stress` and `mutants` hold no
  keys and keep their caches.

Tests in `crates/release-manifest` hold both rules on the files as written: every `uses:` in
`.github/workflows` must be a commented SHA pin, and the release jobs are found by what they
hold, so a new job with a secret is held to the cache rule without being named.

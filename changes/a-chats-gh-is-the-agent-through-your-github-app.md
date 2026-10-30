### Added

- **A chat's `gh` can act as your organisation's own GitHub App** (#2053). Declare it on a
  persona's tools as `gh = { vault = "…", mint = "github-app", app = "<client id>", key = "…" }`,
  with the App's private key in that vault. Each run of `gh` then gets a short-lived installation
  token in `GH_TOKEN`, limited to the GitHub repos of the clones in the chat's workspace that
  purlis cloned or you saved from the window (save a clone you made yourself once). The key
  itself is never handed to anything: `secret exec`, `get` and `cp`, other tools and the Vaults
  tab's reveal all refuse it. A runner chat gets the token its desktop mints. For GitLab, declare
  `glab` with a project or group access token, as any tool is.

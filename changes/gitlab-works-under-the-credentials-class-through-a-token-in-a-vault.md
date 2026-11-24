### Added

- **GitLab works in a chat that holds your own credentials back, through a token in a vault** (#1349). `purlis gitlab access <host> --persona <persona>` (and Settings › GitLab access in the window) keeps a GitLab token in the persona's vault and declares `glab` for it, on the host one of the project's clones has a GitLab remote on, gitlab.com or your own. We suggest a project or group access token in the Developer role with the `api` scope. Allow the new `glab` line, then restart the chat: `glab api` and `glab mr` reach that host with that token, and never read your own `glab` login. The chat never sees the token.

### Changed

- **A chat's start note says which forge host has no token, and how to get one, for each host.** A GitHub mint no longer silences the note for a GitLab remote, a `glab` tool whose host is not the remote's is named, and the GitLab line names the setup before the fallback of turning the credentials class off. A `glab` the chat's persona does not declare now answers with the flow to run and exits 1, instead of failing on a login it was never to read.

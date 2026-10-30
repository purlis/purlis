### Added

- **A forge tool names the host it reaches.** A persona's `gh` or `glab` tool takes `host`
  (`host = "github.example.com"`); without it, `github.com` or `gitlab.com`. The run is told
  that host, and a chat's run that names another (a repository as `HOST/OWNER/REPO`, a URL, a
  `git@host:` remote) never runs with the tool's value (#3869).

### Added

- **A forge tool names the host it reaches.** A persona's `gh` or `glab` tool takes `host`
  (`host = "github.example.com"`); without it, `github.com` or `gitlab.com`. The run is told
  that host, and a chat's run that names another (a repository as `HOST/OWNER/REPO`, a URL, a
  `git@host:` remote) never runs with the tool's value (#3869).

### Changed

- **A `gh` or `glab` tool used with GitHub Enterprise Server or a self-managed GitLab needs
  `host`.** Without it the tool reaches `github.com` or `gitlab.com`, and a run naming your
  own host is refused with a sentence that says so: add `host` to the tool and allow the
  persona's tools again (#3869).

### Fixed

- **The network log says what the forge answered a failed `gh` or `glab` call.** It lists the
  HTTP status the CLI reported, such as 404, where it said only that the call failed. A call
  that succeeded still lists no status, because the CLI does not report one. An update check
  that the server answered with a 404 is now listed as answered, not as a request that got no
  answer (#1022).

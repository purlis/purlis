### Added

- **A Claude Code chat on a runner has its profile's MCP servers** (#3846). User-scope servers now go
  with a profile's settings, under the same rules: no environment, headers or sign-in, and a server
  whose command shows a credential stays behind. A value a server needs goes in a vault. Plugins are
  not sent yet (#4441).

### Added

- **`purlis profile list` shows the sign-in purlis can see each profile has**, or `unknown`. A
  harness declaration says which sign-in kinds its profiles support (subscription, API key,
  cloud, local), the folder variables it uses, whether its credentials can be kept from the
  commands it runs, and how a conversation is exported and resumed; Claude Code, Codex and
  opencode declare all four, and a project's `harnesses/<name>.toml` may too (#2031).

### Security

- **`gh` and `glab` refuse more of their credential commands in a chat.** After an audit of each
  command line's whole command set, `gh secret` and `gh repo set-default`, and `glab variable`,
  `securefile`, `duo`, `mcp`, `opentofu init` and `download`, `cluster agent` token commands,
  `runner-controller token`, `ci --with-variables` and `repo view` as JSON, each under its aliases
  too, never run with a vault's value from a chat; nor does `api` on the forge's own host where
  its route does what a refused command does (keys, secrets, tokens, CI/CD variables), or GraphQL
  that asks for a credential. The audit is `docs/forge-tool-refusals.md` (#3894).

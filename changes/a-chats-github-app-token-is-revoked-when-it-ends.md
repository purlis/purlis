### Changed

- **A chat's GitHub App token is revoked when the chat ends** (#3808). Each installation token
  purlis minted for a chat's `gh` is revoked on GitHub as the chat closes or its program ends,
  and every chat's as purlis quits (it waits up to two seconds for them), instead of living out
  its hour. A revoke GitHub refuses, or one with no network, is logged, and that token still
  expires within the hour.

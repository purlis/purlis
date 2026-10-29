### Added

- **You control which options a declared tool runs with.** A persona's tool takes `args`, a list
  of patterns for whole arguments (`args = ["get", "describe", "-n", "*"]`); a chat's run of the
  tool goes ahead only where every argument matches one, and an option only where a pattern
  spells one. `args` is part of the tool's Allow, so changing it asks again (#3421).

### Security

- **`gh` and `glab` never run their credential or settings commands with a vault's value from a
  chat.** `auth`, `config`, `alias`, `extension`, `ssh-key`, `gpg-key` (and `glab token` and
  `deploy-key`, `gh repo deploy-key`), and `api` against another host, are refused in a chat
  whatever the tool declares; run them in your own terminal (#3421).

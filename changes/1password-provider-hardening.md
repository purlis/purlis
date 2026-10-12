### Changed

- **`op` no longer sees the `OP_*` variables purlis was started with.** purlis runs the 1Password
  CLI with its own environment less every `OP_` variable, and adds only what the vault declares.
  A token, a session (`OP_SESSION_*`), a Connect server or a settings folder (`OP_CONFIG_DIR`)
  your shell exports no longer decides whose identity a vault is read under: a vault with no
  token of its own signs in through the 1Password app. If you relied on one, declare it on the
  vault as a name when you register it (`--env OP_CONFIG_DIR=<VAR>`) (#1542).

### Security

- **A token kept in the keyring is handed only to a 1Password CLI purlis can vouch for.** Every
  read holds the `op` it pinned to more than the pin says: on macOS it must carry 1Password's own
  signature, verified against the file. A token is not stored beside an `op` that does not
  (#1542).

- **`purlis vault add --token-stdin` recognises more of the app's chats, and fewer of your own
  processes as one.** On macOS a process below the app that started a chat is refused, whatever
  project it belongs to, and a process number a crashed app left in its record no longer makes a
  later process count as a chat (#1542).

# Amendment, 2026-10-10: one Keychain question per run, and none from a sandboxed chat (#1638)

**The problem it closes.** With a vault's service-account token kept in the keyring, the
`purlis` command made the Keychain ask once per `op` it ran, and `secret exec` runs `op` once per
value it hands on: three `--env` values were three questions in one run. Several chats running
`secret exec` raised a stream of them, each "Allow" good for one read. And where the app did not
answer a sandboxed chat's `secret exec`, or for `secret get` and `cp`, which are not handed to the
app, the command read the Keychain itself from inside the chat (V90's measurement: such a read
asks the person).

**Decision (delegated, 2026-10-10).**

- **One run reads each kept token once.** The context a command reads through remembers the
  token it read, for that run only. It is never kept across commands, and the app builds a new
  context for every request it serves, so a replaced or deleted token is read afresh by the
  next run.
- **A sandboxed chat never reads the keyring through the command.** In a chat the app started
  sandboxed, a vault whose values come from the keyring (a `keyring` vault, or one whose token is
  kept there) is refused rather than read: by `secret exec` when no app takes the run, by
  `secret get` and `cp`, which the app does not broker, and, as a backstop where the keyring
  itself is read, by every other read (`vault verify`, `secret rename`, a listing that runs
  `op`). The sentence says why and what to run instead: on macOS `secret exec` through the
  app; where the app runs no command for a chat yet (Linux, #1040), a terminal outside the
  chat. A vault the keyring does not hold runs as it did. On Linux, where the Secret Service
  answers without asking, it refuses the same way: there a sandboxed chat's `secret get`, `cp`
  and every other read of such a vault, which used to read it, are now refused.
- **`vault list` never reads a kept token to draw its status** (#1180). A 1Password vault whose
  token is kept in the keyring shows that it is kept there and that `vault verify` reads it,
  as the guide already said; it no longer runs `op`, which made the Keychain ask once per such
  vault in every listing. `persona list` and `vault add` still check the vault live (#1180).
- **A courtesy, not the boundary.** The refusal reads the chat's own environment, which the
  chat can change. What holds the item is still the store's rule (V90a) and the sandbox; this
  keeps purlis from raising a question on the chat's behalf and says what to do.

**Not decided here (V16b, #1180).** A chat started without the sandbox, and a terminal, still
read the Keychain through the command, so each run asks once. Two ways to take the command out
of that path, each the operator's call:

- **The app runs the command**, as it does for a sandboxed chat: for an unsandboxed chat that
  is a command run outside any sandbox on a chat's word, and it holds such chats to their
  persona's vaults for the first time.
- **The app hands the token back** over the socket and the command runs `op` as now: no new
  run path, but a raw secret then reaches a chat's process, which no sandboxed chat may hold
  (ADR 0067 §5 class 1), and any process of the person's that can reach the socket could ask.

A terminal has no chat token and `local-ui` is the window's alone (V16a), so either way it
needs a route of its own.

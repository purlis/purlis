# Amendment, 2026-09-24: a vault's identity token moves into the keyring (#237)

**The problem it closes.** A vault read through an identity variable (`"env":
{"OP_SERVICE_ACCOUNT_TOKEN": "OP_TEAM_TOKEN"}`) found its token in the environment of the
process running `purlis secret`. So the token had to be exported in every chat's shell, where
any agent could `echo` it. One service-account token outweighs every secret it unlocks. The
operator's ruling (#232, decision 4): tokens move into the keyring, and no chat environment
carries an `OP_*` variable.

**Decision.**

- **The move.** A vault's tab in the app offers "Move this token into the Keychain" where the
  vault's identity variable is set in the app's environment. The core reads each variable the
  vault is read through from its own environment and stores it with `keyring::store(ctx)`, the
  one place the store is chosen, so a fenced build writes the stub. The item's service is
  `charter/identity` and its account is the variable's name. The core then writes
  `"identity": "keyring"` into the vault's config in this machine's registry half. Nothing is
  stored or marked unless every variable the vault declares is set: a half-moved identity would
  read one token from the keyring and look for the other in an environment that no longer
  carries it. The move writes an `identity-move` trace event that names the vault and the
  variables, never the token.
- **The lookup.** For a marked vault, `secrets::env_overlay` reads each identity variable from
  the keyring first and falls back to the environment. It falls back when the keyring holds
  nothing and when the keyring cannot be read, and a keyring error reaches the refusal only when
  the environment has nothing either. An unmarked vault never asks the keyring. `purlis secret`
  from a plain terminal reads the moved token the same way, so it keeps working after the
  terminal stops exporting it.
- **The item is keyed by the variable's name, machine-wide,** unlike a vault's own items, which
  get a random service per vault. The variable is itself machine-wide: two planes whose vaults
  read `$OP_TEAM_TOKEN` were handed one token by one shell.
- **Only the local half can mark.** The mark is read from `.charter/vaults.json` alone. The
  committed `vaults.json` arrives by `git pull`, and a mark there would let a commit choose which
  keyring item purlis hands to the `op` it runs next. A unit test pins this
  (`a_committed_registry_cannot_mark_an_identity_as_held_in_the_keyring`).
- **Where an identity is held is said from the mark**, never by reading the keyring:
  `secrets::identity::held` and `secrets::identity_missing`. So drawing the vault tab, the
  Vaults panel and `vault list` never makes the Keychain ask anything.
- **No chat carries an `OP_*` variable.** `Sessions::open` removes every `OP_*` name the app
  inherited from each chat's environment, as it removes a harness's identity, and drops any
  `OP_*` that a harness profile's `env` declares. The prefix is `secrets::identity::KEPT_FROM_CHATS`.
  All of them are removed, not only those a vault declares: an unregistered token is still a
  token, and `op` reads `$OP_SERVICE_ACCOUNT_TOKEN` on its own. The extension executor already
  starts its programs from an empty environment plus eight named variables, none of them `OP_*`.

**What it does not do.**

- A chat's shell runs the operator's startup files, and a `.zshrc` that exports the token puts it
  back. The docs say to delete that line after the move.
- A terminal outside the app keeps whatever it exports.
- The token still reaches `op`'s environment for each call, as it did before.
- Identity variables that are not `OP_*` (`VAULT_TOKEN`) can be moved, but a chat still inherits
  them from the app. *Superseded twice: the #271 amendment below strips every identity name a
  vault declares, and the 2026-09-29 amendment starts every chat from an allowlisted
  environment, so a variable the app inherited reaches a chat only when a keep-list names it.*
- On macOS the item is written by the app, so the first read by the `purlis` command prompts
  once. "Always Allow" adds that program. This is the two-programs consequence above.
- Nothing moves a token back into the environment. Removing the item from the keyring (or the
  mark from `.charter/vaults.json`) returns the vault to reading the environment.

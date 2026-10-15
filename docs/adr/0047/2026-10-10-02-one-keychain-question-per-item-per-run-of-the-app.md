# Amendment, 2026-10-10: one Keychain question per item per run of the app (#1654)

**The problem it closes.** After an update of an ad-hoc signed build, reopening the app with
several chats raised a stream of Keychain questions for one kept token. The amendment above made
each command read a kept token once, but the app built a new context for every request it
served, so each brokered `secret exec` and each vault tab read the item again, and the Vaults
panel, which every project window draws as it opens, ran `op` with the token to draw its status.

**Decision (delegated, 2026-10-10; flagged for the operator).**

- **The app reads each kept token once per run.** Every context the app builds shares the app's
  one memory of the kept tokens it read. It lives in the app's process only. It is never
  written anywhere and never handed to a chat: a brokered `secret exec` hands the token to `op`
  as before, and the chat gets the command's output. The app's next run reads afresh. This
  replaces "the app builds a new context for every request it serves" above, for the memory
  only.
- **It fails closed where purlis can see a change.** The vault's record of its kept token is
  checked again before every use, so a token removed through purlis is never answered from
  memory, a token put in again is a new item, read afresh, and an item purlis deletes is
  dropped from memory with it. **What it does not see:** a token
  deleted or edited directly in Keychain Access while the app runs is still used until the app
  quits. Revoking the token in 1Password stops it either way. That gap is the widening, and the
  operator rules on it: keep it, or re-check that the item still exists before each use.
- **No listing reads a kept token** (#1180): `persona list`, `persona show` and the app's Vaults
  panel draw the same status `vault list` does, from the record. `vault add` still checks the
  vault live, because the person just asked for it. This replaces "`persona list` and
  `vault add` still check the vault live" above. The app's Vaults panel runs no `op` at all to
  draw a 1Password vault's row, since a vault that signs in through the 1Password app reads
  that app's data; the vault's tab reads it.
- **An `op` handed a service-account token is told not to read the 1Password app's settings**
  (`OP_LOAD_DESKTOP_APP_SETTINGS=false`). It read them from the 1Password app's container on
  every run, which made macOS ask whether purlis may "access data from other apps". A vault that
  signs in through the 1Password app reads them as before.

**Follow-up (delegated, 2026-10-10, #1660; flagged for the operator).**

- **The re-check is built.** Before the app reuses a token it remembers, it asks the Keychain
  whether the item is still there. The search returns the item's attributes and never its data,
  the same question `security find-generic-password` asks without `-w`, so the Keychain
  decrypts nothing and has nothing to ask the person about. A token deleted in Keychain Access is
  then forgotten, and the read that follows finds none. An answer other than found or not found
  keeps the remembered token, as before. A token edited in place in Keychain Access keeps its
  item, so the app still uses the old value until it quits; revoking the token in 1Password
  stops it. Only macOS is asked; on other systems a deleted token is still used until the app
  quits. This picks the second option above. Whether the search
  ever asks the person still has to be confirmed on a real Mac after an update. If it does ask,
  revert to the first option.
- **A 1Password Connect token gets the same treatment as a service-account token.** An `op`
  handed `OP_CONNECT_TOKEN` is told not to read the 1Password app's settings, because a Connect
  sign-in never goes through that app either.
- **A vault tab that a launch puts back reads nothing until the person presses *Read*.** This
  applies to every provider except those read from the project's files alone (keyring,
  plain-file, reference). It is the same rule as an extension's put-back view.

The lasting fix for questions after every update is a stable code-signing identity (Developer
ID, #606).

**Amended 2026-10-13 (#1542, delegated; flagged for the operator).**

- **The pinned `op` is held to more than its record says.** The record is this machine's half,
  a file, so a read no longer trusts the Team id it names alone. The pinned program must be named
  `op`, and on macOS a build that ships verifies it with `codesign --verify --strict` against
  1Password's code requirement (a certificate Apple issued to AgileBits' Team id, `2BUA8C4S2C`),
  at every read and before a token is stored. This replaces "not hard-checked" above. Off macOS
  nothing is verified; the Secret Service keeps no per-program rule either (V90c).
- **`op` is run with none of purlis's own `OP_*` variables.** Every `OP_` variable of purlis's
  environment, of any case, is left out of the child's, and the vault's overlay is the only
  source of one. A vault that relied on an ambient session or settings folder declares it in its
  `env` binding. Chosen fail-closed: an exported variable never decides whose identity a vault
  is read under.

# Amendment, 2026-10-09: a token is given to purlis, and a tick extends it (#1527)

**The problem it closes.** A vault read with a service-account token was still set up by hand:
a variable's name in the registry, an export in a shell profile that every chat's shell could
read, and later a move into the keyring. And a token used by several vaults had to be pasted
once per vault, because the first cut of #1526 that marked every alike vault did it silently,
from the merged registry, and was taken out.

**Decision.**

- **A vault may declare a token with no variable.** `"token": "keyring"` in a 1Password
  vault's config says it is read with a service-account token purlis keeps in the keyring. It
  names no secret, no variable and no keyring item, so it may be committed. It reads as one
  binding whose source is a fixed word that is no variable's name and is never looked up in an
  environment, and an `env` binding of the token's variable beside it is not honoured. So
  nothing in either half of the registry can name where such a token is read from.
- **The registration and the record are one step.** *New vault* in the app, *Change how this
  vault signs in* on a vault's tab and `purlis vault add --token-stdin` store the token's item,
  then write this machine's half once, with the vault and its pinned record together. The vault
  is then read back as both halves merge it; unless its settings are exactly the ones the
  person gave and the record is honoured for them, everything is undone. A failure anywhere
  deletes the item and puts both halves back.
- **The record also pins the item** the vault keeps its secrets in, beside the binding, the
  1Password vault, the account and the provider's program. A record made before this amendment
  names no item and is not held to one.
- **A sign-in is tested before anything is registered**, by reading item names only. A failed
  test is said in purlis's own fixed sentences, by kind; the provider's output is never shown.
- **One token may be given to several vaults, by the person's tick.** After a token is given,
  the set-up lists the other vaults bound to the same identity, each with the settings a record
  would pin for it, which half of the registry names it, and a digest of those. A vault the
  committed half names starts unticked. The store takes the ticked names with the digests the
  person was shown, recomputes each, and marks only those that still match, each under its own
  keyring item and its own record; the rest are skipped and said.

**What stays as the 2026-09-24 amendment wrote it.** A committed entry can neither create a
record nor change one. What makes a record for a vault the committed half names is the person's
tick, in the app's own window, on the settings they were shown; a commit that changes those
settings after they were shown unpins the tick's digest, and a commit that changes them after
the record was made unpins the record. A store with no ticks marks the one vault it is for, as
before.

**A chat never supplies a vault's token.** The window's commands are not a chat's to invoke.
`vault add --token-stdin` refuses inside a chat, or a shell the app started, before it reads
anything: by the marks the app sets in the environment of what it starts, and by whether the
command runs below a program the project's record of open chats names, or below the app that
holds a project's hook socket now, so clearing the variables does not pass. That check is a courtesy to an agent, not the boundary. The boundary
is the sandbox's: both halves of the registry and the keyring are not a sandboxed chat's to
write (ADR 0067). A chat that runs with no sandbox can write whatever the person can, as before.

**Not decided here.** A 1Password Connect server (a host and a token) is not supported by the
provider and is not offered. An identity read from an environment variable stays supported for
a machine with no keyring and for CI.

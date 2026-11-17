# A kept screen is masked after a crash by what purlis sealed, not by reading vaults again

**Accepted 2026-11-18**, deciding #6907 (parent #5820) under the steward's standing order to take
the recommended path. It changes how #6309's crash path works; what a kept screen masks, and how
the mask looks, are the person's ruling of 2026-11-15 and do not change.

## Where purlis was

A dispatched task's kept screen is masked with every secret purlis handed its chat (#6309,
`secrets::handedto`). While the window runs, it knows each value as a keyed digest in memory.
After a window crash it knows nothing in memory, so #6309 named each entry on disk
(`app/chat-handed/<chat>.names`, vault and key, never a value) and the next launch **read each
entry again** to find its value. It read only where that asks nobody: a keyring or plain-file
vault. Anything else left the screen **not kept**: a 1Password vault (macOS or `op` may ask the
person), a reference vault, an entry linked into either, a one-time code or a minted GitHub App
token (made once, never read again), and an entry changed since it was handed.

So a task that used a 1Password vault, left behind by a crash, lost its screen.

## Options weighed

1. **Accept it.** Crashes are rare, and failing closed loses nothing secret. But the vaults
   that lose their screens are the common ones for a team, and a one-time code always does.
2. **Ask the person once, at the launch, to read those entries.** An unasked prompt at a
   launch, for a screen they may never open, and it still cannot make a one-time code again.
3. **Keep that task's screen masked in full.** A screen with nothing on it is no screen.
4. **Seal, as each value is handed, what finds it again, so the launch reads no vault.**

## The decision

Option 4. As purlis hands a chat a value, its line on disk records, **for each way the value may
be shown** (as handed, trimmed, and for a value of several lines, whole with CRLF line ends and
each line alone: `handedto::Known::of`'s forms), three things, each under a key derived from the
plane's `fingerprint.key` for one purpose (`fingerprint::seal`):

- its **length** in bytes;
- its **print**: a Rabin–Karp hash modulo 2⁶¹−1, the one the window already rolls along a
  screen, under a **base of its own**, drawn from the plane's key for this chat, this entry and
  this place in the line;
- its **seal**: `HMAC-SHA256` of the chat, the vault, the key and the form.

And the line carries a seal of its own over the chat, the entry and everything above.

A launch after a crash checks each line's seal, rolls each print along the screen's text, and
masks a window only where its print matches and the window seals the same. **No vault is read**,
so no provider is asked, and a one-time code, a minted token or an entry rotated since is found
as well as any other value: the line is of the value the chat was handed, not of the entry's
value now.

### Why a base of its own for each print

Two forms of one value are related: one is the other with a line feed after it. Under a shared
base `b`, `print(v + "\n") = print(v)·b + 11`, which gives `b` away, and with `b` known each
print is one linear equation on the value's bytes, which a lattice can solve for a short value.
Each print under its own base, drawn from the plane's key, says nothing without that key.

### What it costs

- The line holds more: a short value, two forms (about 200 bytes); a PEM key, one per line.
- Without the plane's key a line says nothing of the value. **With it**, a short value can be
  guessed and checked against the seal, exactly as it could against #6309's seal. The key is
  denied every sandboxed chat; a program that can read it can read the vault too.
- A screen is looked at once per form on disk, not once per length: each form counts against
  `handedto::MATCH_STEPS_AT_MOST`.
- `Form` (dotenv-escaped, tunnelled, made once) is kept on the line as a record of how the chat
  was handed the value, and no longer used to make it again.

### What it removes

`handedto::read_again`, `handedto::asks_nobody` and `secrets::reads_a_value_without_asking`: the
crash path never reaches a vault.

## Consequences

- A task left behind by a crash keeps its screen, masked, whatever vault its values came from.
- A line not sealed by purlis as it stands (edited, or another chat's, #7129) leaves the screen
  not kept, as any line purlis cannot account for does.
- `docs/plane-format.md` records the line's new shape under `app/chat-handed/<chat>.names`.

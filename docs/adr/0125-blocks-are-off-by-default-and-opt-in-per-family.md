# Blocks are off by default and opt-in per family

**Accepted 2026-11-16**: the person's rulings Q308, Q308a and Q308b in the who-judges
orchestration (`work/who-judges/decisions.md`), under Q307 ("one project policy governs everything
a chat can do", map #6969). It is the first slice of that policy: the policy later turns these
keys into full rules with the same default, and keeps the names.

## Where purlis was

purlis refused a chat a great deal, and almost none of it could be changed: 819 of the 1,199
decisions the #6970 inventory lists are hard-coded. The Bash guard said so in every refusal
(charter#370, `toolgate::OVERRIDE_NOTE`): *"There is deliberately no config key, environment
variable or switch that lifts a purlis denial: one purlis could read is one a committed file
could flip."* A team that wanted its chats to work as their harness does had no way to say so.

## The decision

### 1. Families, each `off`, `log` or `enforce`

purlis's refusals fall into families: `sandbox`, `network`, `ports`, `processes`, `guards.git`,
`guards.merge`, `guards.release`, `guards.save`, `guards.credential`, `asks`, `dispatch`,
`vaults`, `browser`, `skills` and `secrets` (Q308), and, by Q308b, `guards.forge`,
`guards.shape`, `guards.plane`, `mode`, `tools`, `trust`, `runners`, `consents` and `limits`.
Each runs in one of three modes:

- **`off`**: the check does not run.
- **`log`**: the check runs and allows. Nothing is shown to the chat or the person (no Notice,
  no ask); one quiet line in the chat's enforcement log names the rule, the family, the chat and
  the host, and never a command, a path or a value (`app/enforcement/<chat>.jsonl`).
- **`enforce`**: the refusal as purlis has always made it.

`vaults` takes `off` or `enforce` only. The sandbox is a level, `off`, `floor` or `full`, which
the template sets through the existing `[sandbox]` mode: `[policy]` has no `sandbox` key.

### 2. Templates, and `watch` by default

A project picks a template and overrides single families under `[policy]` in its committed
settings file (`docs/plane-format.md`).

| Template | Families | Sandbox |
|---|---|---|
| `open` | everything off | off |
| `watch` (**the default**) | everything logs; `secrets`, `vaults` and `guards.credential` enforced | floor |
| `hold` | `watch`, and `guards.merge` enforced | floor |
| `team` | `guards.merge`, `guards.release`, `guards.save`, `guards.credential`, `secrets` and `vaults` enforced; the rest log | full |
| `strict` | everything enforced: purlis before this ruling | as `[sandbox]` says |

A value purlis cannot read never loosens anything: an unknown template is `strict`, an unknown
mode is `enforce`, and the window says which.

### 3. A chat can never loosen its own enforcement

- The setting is read from the plane root's committed file only; a branch folder's file is never
  asked, and the local settings file can only tighten.
- It is resolved once, at a chat's start, and handed to the chat's hooks in `$PURLIS_POLICY`,
  which the app sets after anything a profile set and no chat inherits.
- **A loosening waits for the person's press** in the window. What this person accepted is kept
  on this machine **sealed** with a key only the app reads (Q308b): an HMAC under a keyring item
  outside purlis's own prefixes, which the app's one-item writer refuses, so no other program can
  forge it. An acceptance whose seal does not hold counts as none. A tightening needs no press and
  applies at once.
- A hook takes the stricter of what its chat started with, what the plane root's file says now
  and what the person accepted: nothing a chat puts in its own environment loosens anything.
- A chat's change to `[policy]`, by a write or by a commit, is held as its change to `[sandbox]`
  is.

### 4. A lock wins

An administrator's `/etc/purlis/policy.json` still applies on top. Its `enforcement` list names
families locked enforced on the machine, and every existing lock keeps its own family enforced
(`opt-out` the sandbox, `hosts` the network, `socket-grants` the ports, and so on).

### 5. One check at each choke point

A family's refusals pass through a few functions; each asks `enforcement::mode` once, rather than
each of the hundreds of call sites. The Bash guard asks it per arm, so an arm that is off is not
run, and one that logs lets the arms after it still refuse.

### 6. The same setting in every harness

The choke points are purlis's own (its hooks, its scan, its wrap, its chat start), so a family
means the same in Claude Code, Codex and OpenCode. Where a harness gives purlis no hook for a
check, that check is a gap under every mode, as it was before.

### 7. Built in stages

This record's first pull request makes every family a key with its template default, switches
the families whose choke points stand apart from the grant stores (`guards.*`, `secrets`,
`processes`) and the simple half of `dispatch` (a pair nothing covers starts with no grant) and
`asks` (a host is answered Allow), and keeps the rest enforced until they are wired: the rest of
`dispatch`, `vaults` and `asks` under Q308b's precedence (lock > the person's never > policy deny > same persona > policy allow >
sealed grant > policy ask), `network` and `ports` with the live asks and the sandbox's grants,
`browser`, `skills`, the sandbox level, and the families Q308b added. The person's own denies
(dispatch nevers, "Keep blocked" hosts) hold in every mode. Only "a chat can't change its own
policy", the kill switch and a grader's lock bundles stay outside the policy.

## What it reverses

- **charter#370**: a committed file now lifts a guard's refusal. What stays of it is that the
  switch is a person's: a loosening holds only once they press, and `OVERRIDE_NOTE` names the
  setting.
- **R-021** ("the leak guard is never switchable"): it is the `secrets` family, enforced by every
  template but `open`.

## Consequences

- A new project's chats start at the floor, and its guards log rather than refuse, until the team
  picks `hold`, `team` or `strict`. Vault values stay masked and credentials stay out of commits.
- Every existing test that proves a refusal sets `strict` explicitly; M74.1's matrix runs with
  `strict`.
- The enforcement log is best effort: a hook inside purlis's wrap (Codex, OpenCode) cannot write
  it, and a refusal the app judges for a chat it knows only by number is let through under `log`
  with no line.
- A harness hook trusts `$PURLIS_POLICY` as the start resolved it, at the trust the hook's other
  launch variables have; a nested harness a chat starts from its own shell could name its own.
  Q309 moves evaluation into the session host, asked over the hook socket.

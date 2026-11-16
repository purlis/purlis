# Blocks are off by default and opt-in per family

**Accepted 2026-11-16**: the person's ruling Q308, in the who-judges orchestration
(`work/who-judges/decisions.md`), under Q307 ("one project policy governs everything a chat can
do", map #6969). It is the first slice of that policy: the policy later turns these keys into
full rules with the same default.

## Where purlis was

purlis refused a chat a great deal, and almost none of it could be changed: 819 of the 1,199
decisions the #6970 inventory lists are hard-coded. The Bash guard said so in every refusal
(charter#370, `toolgate::OVERRIDE_NOTE`): *"There is deliberately no config key, environment
variable or switch that lifts a purlis denial: one purlis could read is one a committed file
could flip."* A team that wanted its chats to work as their harness does had no way to say so
short of starting each chat without the sandbox, and even then the guards held.

## The decision

### 1. Fifteen families, each `off`, `log` or `enforce`

purlis's refusals fall into fifteen families: `sandbox`, `network`, `ports`, `processes`,
`guards.git`, `guards.merge`, `guards.release`, `guards.save`, `guards.credential`, `asks`,
`dispatch`, `vaults`, `browser`, `skills` and `secrets`. Each runs in one of three modes:

- **`off`**: the check does not run.
- **`log`**: the check runs and allows. Nothing is shown to the chat or the person (no Notice,
  no ask); one quiet line in the chat's enforcement log names the family, the rule and what it
  would have refused (`app/enforcement/<chat>.jsonl`, Clone state).
- **`enforce`**: the refusal as purlis has always made it.

`sandbox` takes `off` or `enforce` only: the OS wrap is applied at a chat's start or not. `off`
starts the chat as a person's opt-out does.

### 2. Four templates, and `watch` by default

A project picks a template and overrides single families under `[enforcement]` in its committed
settings file (`docs/plane-format.md`).

| Template | Families |
|---|---|
| `open` | everything off |
| `watch` (**the default**) | everything logs; `secrets` enforced; the sandbox off |
| `team` | `guards.merge`, `guards.release`, `guards.save`, `secrets` and `sandbox` enforced; the rest log |
| `strict` | everything enforced: purlis before this ruling |

A value purlis cannot read never loosens anything: an unknown template is `strict`, an unknown
mode is `enforce`.

### 3. A lock wins

An administrator's `/etc/purlis/policy.json` still applies on top. Its new `enforcement` object
sets the least a family runs at on the machine, and every existing lock keeps its own family
enforced (`opt-out` the sandbox, `hosts` the network, `socket-grants` the ports, and so on). A
lock is set on purpose, so it is opt-in, not hard-coded.

### 4. One check at each choke point

A family's refusals pass through a few functions. Each asks `enforcement::mode(project, family)`
once, rather than each of the hundreds of call sites. The Bash guard asks it per arm, so an arm
that is off is not run, and one that logs lets the arms after it still refuse.

### 5. The same setting in every harness

The choke points are purlis's own (its hooks, its proxy, its chat start, the app's decisions),
so a family means the same in Claude Code, Codex and OpenCode. Where a harness gives purlis no
hook for a check, that check is a gap under every mode, as it was before.

## What it reverses

- **charter#370**: a committed file now lifts a guard's refusal. What stays of it is that the
  switch is a person's: a chat's change to `[enforcement]` is held as its change to `[sandbox]`
  is, and `OVERRIDE_NOTE` now names the setting.
- **R-021** ("the leak guard is never switchable"): it is the `secrets` family, enforced by every
  template but `open`.

## Consequences

- A new project's chats start without the OS wrap, and its guards log rather than refuse, until
  the team picks `team` or `strict`. Vault values stay masked.
- Every existing test that proves a refusal sets `strict` explicitly; M74.1's matrix runs with
  `strict`.
- The enforcement log is best effort: a hook inside purlis's wrap (Codex, OpenCode) cannot write
  it, as it cannot write a chat's spend file. Under `watch` no chat is wrapped.

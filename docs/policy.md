# Which of purlis's refusals run: `[policy]`

purlis can refuse a chat many things: a branch move in the plane root, an unattended publish, a
merge, a commit with a credential in it, a read of a vault. Which of these run is the project's
choice, in its committed settings file (`purlis.toml`, or `charter.toml` on a project that still
has that name), under `[policy]` (rulings Q308, Q308a and Q308b;
[ADR 0125](adr/0125-blocks-are-off-by-default-and-opt-in-per-family.md)).

```toml
[policy]
template = "team"          # open, watch, hold, team or strict; watch when absent
guards.release = "log"     # one family, apart from the template
```

## Modes

- **`off`**: the check does not run.
- **`log`**: the check runs and allows. Nobody is shown anything; one quiet line in the chat's
  enforcement log (`.purlis/app/enforcement/<chat>.jsonl`) names the rule, the family, the chat
  and the host, never the command, a path or a value.
- **`enforce`**: the refusal, as purlis has always made it.

## Templates

| Template | What runs | Sandbox (a committed `[sandbox] mode` overrides it) |
|---|---|---|
| `open` | nothing | none |
| `watch` (the default) | everything logs; `secrets`, `vaults`, `guards.credential` and `trust` are enforced | the floor |
| `hold` | `watch`, and `guards.merge` enforced | the floor |
| `team` | `guards.merge`, `guards.release`, `guards.save`, `guards.credential`, `secrets`, `vaults` and the families Q308b added enforced; the rest log | the full sandbox |
| `strict` | everything enforced, as before Q308 | as `[sandbox]` says |

The families, and what each covers, are listed in
[`docs/plane-format.md`](plane-format.md#policy--which-of-purliss-refusals-run). In this release
the git, merge, release, save and credential guards, `secrets`, the sandbox level, `network`,
`ports` (off or enforce only), `dispatch` (a pair nothing covers starts with no grant) and `asks`
(a host and a folder are answered Allow; a socket only under `off`) switch, and so do the nine
families ruling Q308b added (`guards.forge`, `guards.shape`, `guards.plane`, `mode`, `tools`,
`trust`, `runners`, `consents` and `limits`; under `limits = "log"` a limit is counted, not
applied, up to an administrator's ceiling where there is one); every other family is a key already
and stays enforced until it is wired. Which choke point switches each, and the family of every
decision purlis makes about a chat, are in
[`docs/policy-inventory-families.md`](policy-inventory-families.md). One key decides a sandbox
rule: `secrets.credential_files = "enforce"` holds your SSH keys and forge and cloud credential
files from chats at the floor too, as `team` and `strict` do; the full sandbox always holds them
(ADR 0067 §5 class 6, Q325). Your own denies hold in every mode, `open` included: a dispatch never,
a host you kept blocked, and your harness's ask and deny rules for a program, behind which a Bash
guard that fails to answer refuses the call. Where the sandbox is on, a Bash guard that fails to
answer refuses the call too, since it judges the sandbox's own rules (Q327).

**The names of purlis's own sockets are held at every floor, whatever `ports` says.** A chat in
the sandbox can neither move nor link `hooks.sock`, `slots.sock`, `purlisd*.sock`,
`purlis-crash-*.sock`, the app's single-instance socket, or the folders holding purlis's server
sockets and pool forwards (`purlis-serve`, `engine-links`), so a socket moved while `ports` is
enforced is not reached after a `ports = "off"` you press. The same rule stops a chat **creating
any file with one of those names anywhere** (a file called `hooks.sock`, `slots.sock` or
`purlisd*.sock`): purlis's own test suite, run inside a chat, cannot make them either.
**Any folder named `purlis-serve` or `engine-links`, wherever it is, becomes unwritable to chats**
(nothing in it can be made, moved or removed), your own among them: name yours otherwise.

That call is decided **when the chat starts**, from the rules its harness will read (the
project's, and yours in the `CLAUDE_CONFIG_DIR` its profile sets). A deny rule you add mid-chat
holds a dead Bash guard closed from the next start, not before, and a settings layer reached
only through a folder the chat `cd`s into later is not counted at the start: under `open` or
`log` that window is accepted, so that `open` stays open (Q327, #7155).

**A profile or harness declaration nobody approved still asks** under every template but `open`
(Q329). Only `open`, or your own pressed `trust = "off"` or `"log"`, lets one start
unapproved, and then its chat's tab says so as it starts.

## Where the window shows it

- **A chat's pane head** says the template and how many families are set apart from it (`team
  +2`); its tooltip says the whole policy. A narrow pane drops it first.
- **Settings › Project › Policy** lists the template, each family set apart and how your chats
  run it now (stricter where a lock, a family not wired yet or a loosening that waits for your
  Accept holds it), the families an administrator locks, and Accept for a loosening. Its
  **Would have blocked** count is every chat's enforcement log, and opens it: the newest lines,
  by rule, family, chat and the `host:port` a rule was about. A line that is not as purlis
  writes it (an unknown rule, a host that is no `host:port`, another chat's) is left out and
  counted apart.
- **Where this machine's acceptance does not hold**, every family is enforced: the pane head says
  `strict`, Settings says why and offers the Accept that seals it again, and nothing says
  `watch` is the default.
- **The move to `watch`** is announced once on each machine, in the first project that names no
  template, and never while the acceptance does not hold.

The window only says the policy; the app resolves it, as below.

## Who can change it

- **Only a committed change counts**: the file is read as it stands at the plane root's `HEAD`.
  Where git cannot read `HEAD` or the file's content in time, the plane's `.git` does not read,
  or the repository has no commit yet, the project runs `strict`, never the working tree's file.
  That is not kept as what you accepted: once git reads it again, the project runs as before.
- **A tightening applies at once**, to every chat, from its next tool call.
- **A loosening waits for your press.** The window names each family the project now runs looser
  than you accepted, with **Accept**, which accepts exactly what it showed you: if the committed
  file, your local file or an administrator's lock changed since, nothing is accepted. Until you
  press it, your chats keep what you accepted before. What you accepted is kept on this machine,
  sealed by purlis's app; a chat can neither press Accept nor write it.
- **Only the app decides.** A hook or a command a chat runs asks the app, and refuses as `strict`
  does where the app does not answer.
- **A chat cannot change `[policy]`.** Its write of the settings file, or of `policy.cedar` at the
  plane root, is refused by a file tool or a shell command, naming the rule `policy-self-change`,
  whatever the modes say; a chat's sandbox holds the same files. Its commit of a change to the
  table or the rules changes nothing until you press Accept, if it loosens. A branch folder's copy
  of the file is never read, and is the chat's to edit and commit. The refusal reads the words of
  the call: a shell can spell a path no reading sees, so the sandbox is what holds a chat that
  tries, and **a chat that runs without a sandbox has no second hold**: only that reading stands
  between it and the file. Where a sandboxed Claude Code chat edits `[policy]` in the settings
  file with a file tool, the edit becomes a proposed change you apply in the window, not a
  refusal; `policy.cedar` is refused. The hold fails closed: where a guard that runs it times
  out, crashes or answers unreadably, the call is refused, under every mode.
- **Your local settings file can only make a family stricter.**
- **An administrator's lock wins.** `/etc/purlis/policy.json`'s `enforcement` list names families
  that are always enforced on that machine, and its other locks keep their own families enforced.
  A lock on `guards.git` also holds `guards.plane`, which took over the plane root's branch and
  reset guards (A3, A3b) from it.
- **A value purlis cannot read never loosens anything**: an unknown template is `strict`, an
  unknown mode is `enforce`, and the window says which.

## What a chat that is already running picks up

**Every hook and permission decision uses the current policy**, so a tightening applies at once and
a loosening applies at the next decision after you press Accept. Only the shell sandbox, compiled
when the chat started, waits for a restart (ADR 0127, amended 2026-11-25).

- **A tightening reaches it at its next decision**, with no restart: every hook and proxy ask reads
  the committed project and your sealed answers afresh, and seals each tightening it sees into the
  accepted floor.
- **A loosening reaches it at its next decision after your press**, not only at its next start. A
  rule you drop, a Never you remove: nothing reaches the chat until you press Accept (the accepted
  copy only moves at the press), and from then on its decisions follow it. The chat is not marked.
- **The sandbox its shell commands run in is compiled at its start**, so a tightening cannot reach
  it. The chat is marked as having an **older shell sandbox** (not as the project updated, which
  is what **Plane updated** says of its instructions), and its pane offers **Restart chat**, which
  compiles the sandbox again, when
  - a Never reaches it,
  - a grant that served its reads or writes is removed,
  - a forbid comes into force, or a permit that let a place through is gone or narrower (a read is
    allowed only where a permit allows it): rules are compared by the places they resolve to, so a
    named set or repo that grows counts, and a rule whose text is unchanged does not hide it,
  - the sealed answers enter their deny state, or your answers for everywhere cannot be read,
  - a workspace is created after it started, or
  - purlis restarted under it.

  The mark's tooltip says which. The window asks again when the plane's `HEAD` moves, when a Never
  or a grant is written or removed, and when you press Accept.

## When a guard is wrong about your case

Set its family to `log` in `[policy]`, commit it, and press Accept in the window; the chat's
next start runs it that way. Or run the command yourself, in your own terminal: these guards
bound what an agent does with your authority, never what you do.

## What an administrator's forbid does, and what is never asked

- **An administrator's forbid is always enforced**, whatever the project's template or family
  mode: every Organisation-level forbid in `/etc/purlis/policy.cedar` and every one `policy.json`
  translates to is `enforce` unless the administrator's own `[policy.rules]` says otherwise. A
  project can't loosen it by a mode, a rule mode or a rule of the same `@id`. (A forbid the
  administrator writes there with `@level("project")` is project-level by design, and a project's
  rule of the same `@id` can replace it.)
- **A request's host is a name or an address alone**, lower case, with no path, no port (the
  port is the request's `context.port`) and no wildcard. A host that is not one, such as
  `localhost`, a single-label name, a loopback, link-local or cloud-metadata address, an IPv6
  address without its brackets, or `evil.com/x.corp.example`, is **unasked**: it is refused in
  every mode, `off` and `log` included, because no rule can be put on it. The members of the sets
  `known-hosts` and `production-hosts` are read the same way, and a member that is not such a
  host (or carries a port, or starts with `*.`) makes the file broken. A set can't be named
  `*.something`: that is the name the policy gives the hosts under a domain.
- **Hazard for administrators: one such member breaks the whole Organisation.** A
  `known-hosts = ["localhost"]` (or any single-label name, a port, a wildcard) in
  `/etc/purlis/policy.toml` makes the Organisation's file broken, and a broken Organisation reads
  as `strict` plus every lock under the full sandbox, not as that one set left out. Check the
  window's Notice after editing it.

## Whose a branch, port or browser is: `context.owner`

On `read`, `write`, `commit`, `push`, `update`, `discard`, `merge`, `reach`, `listen`, `browse`,
a browser's `close` and every forge operation, `context.owner` says whose the thing acted on is,
to the chat that acts: `"self"`, `"task"` (a chat down its own dispatch line), `"asker"` (one up
it), `"other"` or `"none"`. It is always there, so a rule reads it with no `has`. purlis computes
it from records only the app seals, so a chat can't make itself an owner by editing a file. A
record that is missing or was changed reads as `"none"`, and the window says so.

**Write `context.owner != "self"`, never `context.owner == "other"`.** `"none"` is what a rule
sees wherever purlis holds no record, which today is nearly everywhere, and wherever a record was
lost. A forbid on `== "other"` never fires on `"none"`, so it lets through exactly the act whose
owner purlis can't vouch for:

```cedar
@id("guards-git-push-only-its-own")
forbid (principal, action == Action::"push", resource)
  when { context.owner != "self" };
```

`context.made_by` on a forge operation reads the same words, `"none"` until purlis keeps a record
of what a chat made. A rule on `read` or `write` can't name `owner`: the sandbox is a fixed
profile that can't tell whose a path is, so purlis refuses the rule when it loads the policy.

## Known gaps: what a harness cannot enforce

A rule in force (a family whose mode is not `off`) that a harness cannot enforce on this system is a
**known gap**. purlis declares it and the chat still starts: in Settings under *Sandbox › Known
gaps*, in `purlis doctor` (wherever any family is not `off`, which the default `watch` template
already is for `secrets`, `vaults` and `guards.credential`), in the chat's start event and on its
pane, for a chat started from the window as for one a host starts. A family set to `off` declares
none. Nothing is declared as held where it is not: a chat whose sandbox is lifted (the person's
opt-out, or a floor that could not be applied) has a gap for every family the sandbox, the proxy or
an OpenCode shim's protected files would hold, and a declared harness with no adapter has every
family as a gap.

`context.gaps` on `start` holds the keys of the families that are `enforce` and have a gap for the
chat being started. A project that wants such a chat refused writes the rule itself; the shipped
default has none:

```cedar
@id("trust-no-start-on-a-sandbox-gap") @level("project")
forbid (principal, action == Action::"start", resource)
  when { context.gaps.contains("sandbox") };
```

**The rule refuses only where the family `trust` is `enforce`.** Under the default `watch` and
`hold` templates `trust` is `enforce` (Q329), so the rule above refuses. Where the project writes
`trust = "log"` under `[policy]` (or the template is `open`) it refuses nothing: the chat starts,
and its tab says once that the rule would have refused it. Mind what the rule says:
`contains("sandbox")` is true of every start while `trust` is enforced, because Claude Code's reads
are a gap on every system (`claude-code-reads`), and a lifted sandbox is one too; such a rule
refuses every start.

**Not closed yet (slice 37).** What the host reads of a start is Claude Code's mode, and for Codex
the words it knows are a bypass (`--full-auto`, `--dangerously-bypass-approvals-and-sandbox`,
`-a never`): any other Codex start, and every opencode start, has `context.harness_mode` `unknown`,
which `mode-task-above-auto` allows. And `processes-elevation` reads the program a command names,
by the last part of its path: a wrapper (`env sudo`, `command sudo`, `sh -c sudo`) is not read.

An `@ask` forbid on `start` refuses too, until asking at a start is wired (slice 36): the refusal
says the rule asks and that asking at a start is not wired yet. Write a plain forbid.

The refusal names the rule and the gaps. `crates/purlis-core/src/knowngaps.rs` holds the capability
table (each action's enforcement point per harness, system and sandbox, or the gap that stands in its
place) and every gap's entry; `context.machine` is unset on every system (it is sealed until the
person confirms it), so a rule reading it is denied.
